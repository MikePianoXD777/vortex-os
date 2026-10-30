"""Motor de Vortexine Task Manager (sin Qt): procesos agrupados por app, rendimiento y acciones.

Los procesos se leen con psutil y se agrupan como en el Administrador de tareas de Windows:
  · «Apps»: cada ventana abierta (hyprctl clients) con todos los procesos que cuelgan de ella;
  · «Segundo plano»: tus otros procesos, juntos por nombre;
  · «Del sistema»: los de root y de otros usuarios.
Los hilos del kernel (hijos de kthreadd) no salen. El nombre y el ícono de cada app vienen de su
lanzador (.desktop), buscado por la clase de su ventana o por su ejecutable.

El CPU de cada proceso está en % de toda la máquina (no de un núcleo), igual que en Windows; la memoria
es la privada (RSS menos la compartida) y el disco, lo que leyó y escribió por segundo (solo de tus
procesos: los de otros usuarios no se pueden leer sin root).
"""
from __future__ import annotations

import json
import os
import pwd
import re
import shutil
import signal
import socket
import subprocess
import time
from pathlib import Path

import psutil

CPUS = psutil.cpu_count() or 1
KTHREADD = 2
INTERPRETES = re.compile(r"(python[\d.]*|bash|sh|dash|node|perl|ruby|lua[\d.]*|qjs|gjs)")


# --- Lanzadores (.desktop) ---

def _carpetas_aplicaciones() -> list[Path]:
    datos = [os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local/share")]
    datos += (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":")
    datos += [str(Path.home() / ".local/share/flatpak/exports/share"), "/var/lib/flatpak/exports/share"]
    vistas, carpetas = set(), []
    for d in datos:
        p = Path(d) / "applications"
        if d and p.is_dir() and p not in vistas:
            vistas.add(p)
            carpetas.append(p)
    return carpetas


def _idioma_preferido() -> list[str]:
    lang = (os.environ.get("LC_MESSAGES") or os.environ.get("LANG") or "es_MX").split(".")[0]
    return [lang, lang.split("_")[0]]


def leer_desktop(ruta: Path) -> dict | None:
    """Las claves de [Desktop Entry] que se usan aquí (Name en tu idioma, Icon, Exec, StartupWMClass)."""
    try:
        texto = ruta.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    claves: dict[str, str] = {}
    dentro = False
    for linea in texto.splitlines():
        linea = linea.strip()
        if linea.startswith("["):
            dentro = linea == "[Desktop Entry]"
            continue
        if dentro and "=" in linea and not linea.startswith("#"):
            k, v = linea.split("=", 1)
            claves.setdefault(k.strip(), v.strip())
    if claves.get("Type", "Application") != "Application":
        return None
    nombre = next((claves[f"Name[{l}]"] for l in _idioma_preferido() if f"Name[{l}]" in claves), claves.get("Name", ""))
    exe = ""
    for parte in claves.get("Exec", "").split():
        if parte == "env" or "=" in parte:
            continue
        exe = os.path.basename(parte.strip('"'))
        break
    return {"id": ruta.stem, "nombre": nombre, "icono": claves.get("Icon", ""), "exe": exe,
            "clase": claves.get("StartupWMClass", ""), "oculto": claves.get("NoDisplay", "").lower() == "true"}


class Aplicaciones:
    """Índice de lanzadores: por id (archivo), por StartupWMClass y por ejecutable, sin mayúsculas."""

    def __init__(self, carpetas: list[Path] | None = None):
        self.por_id: dict[str, dict] = {}
        self.por_clase: dict[str, dict] = {}
        self.por_exe: dict[str, dict] = {}
        for carpeta in carpetas if carpetas is not None else _carpetas_aplicaciones():
            for ruta in sorted(carpeta.glob("*.desktop")):
                d = leer_desktop(ruta)
                if not d or not d["nombre"]:
                    continue
                self.por_id.setdefault(d["id"].lower(), d)       # la primera carpeta (la tuya) manda
                if d["clase"]:
                    self.por_clase.setdefault(d["clase"].lower(), d)
                exe = d["exe"].lower()
                if exe and exe not in ("flatpak", "sh", "bash", "env"):
                    # El lanzador visible gana a los ocultos (kitty.desktop y no «kitty URL Launcher»).
                    if exe not in self.por_exe or (self.por_exe[exe]["oculto"] and not d["oculto"]):
                        self.por_exe[exe] = d

    def de_clase(self, clase: str) -> dict | None:
        c = clase.lower()
        if not c:
            return None
        if c in self.por_clase:
            return self.por_clase[c]
        if c in self.por_id:
            return self.por_id[c]
        for ident, d in self.por_id.items():     # org.gnome.Nautilus ← «nautilus»; com.spotify.Client…
            if ident.rsplit(".", 1)[-1] == c or ident.endswith("." + c):
                return d
        return self.por_exe.get(c)

    def de_ejecutable(self, nombre: str) -> dict | None:
        return self.por_id.get(nombre.lower()) or self.por_exe.get(nombre.lower())


# --- Procesos ---

def nombre_de(nombre: str, cmdline: list[str]) -> str:
    """El nombre que se muestra: el de comm (o el de cmdline si comm se cortó en 15 letras) y, para
    intérpretes (python, bash, node…), el script o el módulo que corren («python3 -m vortex_player»)."""
    base = nombre
    if cmdline:
        cero = os.path.basename(cmdline[0])
        if cero and len(nombre) >= 15 and cero.startswith(nombre):
            base = cero
    if INTERPRETES.fullmatch(base) and len(cmdline) > 1:
        args = cmdline[1:]
        i = 0
        while i < len(args):
            a = args[i]
            if a == "-m" and i + 1 < len(args):
                return args[i + 1]
            if a in ("-c", "-e", "--eval"):
                return base
            if a.startswith("-"):
                i += 1
                continue
            return os.path.basename(a) or base
    return base


_usuarios: dict[int, str] = {}


def _usuario(uid: int) -> str:
    if uid not in _usuarios:
        try:
            _usuarios[uid] = pwd.getpwuid(uid).pw_name
        except KeyError:
            _usuarios[uid] = str(uid)
    return _usuarios[uid]


class Procesos:
    """Muestrea los procesos. Cada llamada a `muestrear()` da el CPU desde la llamada anterior."""

    def __init__(self):
        self._io: dict[int, tuple[float, int]] = {}

    def muestrear(self) -> list[dict]:
        ahora = time.monotonic()
        filas, io_nuevo = [], {}
        for p in psutil.process_iter():
            try:
                with p.oneshot():
                    ppid = p.ppid()
                    if p.pid == KTHREADD or ppid == KTHREADD:
                        continue
                    cmd = p.cmdline()
                    mem = p.memory_info()
                    uid = p.uids().real
                    fila = {
                        "pid": p.pid, "ppid": ppid, "nombre": nombre_de(p.name(), cmd),
                        "comando": " ".join(cmd), "uid": uid, "usuario": _usuario(uid),
                        "cpu": p.cpu_percent(None) / CPUS,
                        "mem": max(0, mem.rss - getattr(mem, "shared", 0)),
                        "hilos": p.num_threads(), "estado": p.status(), "disco": 0.0,
                    }
                    try:
                        io = p.io_counters()
                        total = io.read_bytes + io.write_bytes
                        io_nuevo[p.pid] = (ahora, total)
                        antes = self._io.get(p.pid)
                        if antes and ahora > antes[0]:
                            fila["disco"] = max(0.0, (total - antes[1]) / (ahora - antes[0]))
                    except (psutil.AccessDenied, OSError):
                        pass
            except (psutil.NoSuchProcess, psutil.ZombieProcess, psutil.AccessDenied):
                continue
            filas.append(fila)
        self._io = io_nuevo
        return filas


def ventanas_hyprland() -> list[dict]:
    """Las ventanas abiertas (pid, clase, título, dirección). Vacío fuera de Hyprland."""
    if not shutil.which("hyprctl") or not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        return []
    try:
        salida = subprocess.run(["hyprctl", "clients", "-j"], capture_output=True, text=True, timeout=3).stdout
        return [{"pid": c.get("pid", 0), "clase": c.get("class") or c.get("initialClass") or "",
                 "titulo": c.get("title", ""), "direccion": c.get("address", "")}
                for c in json.loads(salida or "[]") if c.get("pid", 0) > 0 and c.get("mapped", True)]
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return []


def _suma(procesos: list[dict], clave: str) -> float:
    return sum(p[clave] for p in procesos)


def _grupo(clave: str, nombre: str, icono: str, procesos: list[dict], subtitulo: str = "",
           ventanas: list[str] | None = None) -> dict:
    return {
        "clave": clave, "nombre": nombre, "icono": icono, "subtitulo": subtitulo,
        "procesos": procesos, "pids": [p["pid"] for p in procesos], "ventanas": ventanas or [],
        "cpu": _suma(procesos, "cpu"), "mem": _suma(procesos, "mem"), "disco": _suma(procesos, "disco"),
        "hilos": int(_suma(procesos, "hilos")),
        "suspendido": bool(procesos) and all(p["estado"] == psutil.STATUS_STOPPED for p in procesos),
        "usuario": procesos[0]["usuario"] if procesos else "",
    }


def agrupar(filas: list[dict], ventanas: list[dict], apps: Aplicaciones, uid: int | None = None) -> dict[str, list[dict]]:
    """Procesos → {"apps": […], "fondo": […], "sistema": […]}, cada uno una lista de grupos."""
    uid = os.getuid() if uid is None else uid
    por_pid = {f["pid"]: f for f in filas}
    hijos: dict[int, list[int]] = {}
    for f in filas:
        hijos.setdefault(f["ppid"], []).append(f["pid"])

    usados: set[int] = set()
    grupos_apps: list[dict] = []
    ventanas_por_pid: dict[int, list[dict]] = {}
    for v in ventanas:
        ventanas_por_pid.setdefault(v["pid"], []).append(v)
    # Primero las ventanas cuyo proceso no cuelga de otra ventana (una terminal y lo que abriste en ella).
    for pid, vs in ventanas_por_pid.items():
        if pid not in por_pid or pid in usados:
            continue
        pila, miembros = [pid], []
        while pila:
            actual = pila.pop()
            if actual in usados or actual not in por_pid:
                continue
            usados.add(actual)
            miembros.append(por_pid[actual])
            pila.extend(hijos.get(actual, []))
        lanzador = apps.de_clase(vs[0]["clase"]) or apps.de_ejecutable(por_pid[pid]["nombre"])
        nombre = (lanzador or {}).get("nombre") or vs[0]["clase"] or por_pid[pid]["nombre"]
        icono = (lanzador or {}).get("icono") or vs[0]["clase"]
        titulo = vs[0]["titulo"] if len(vs) == 1 else f"{len(vs)} ventanas"
        grupos_apps.append(_grupo(f"a:{pid}", nombre, icono, miembros,
                                  "" if titulo == nombre else titulo, [v["direccion"] for v in vs]))

    fondo: dict[str, list[dict]] = {}
    sistema: dict[str, list[dict]] = {}
    for f in filas:
        if f["pid"] in usados:
            continue
        destino = fondo if f["uid"] == uid else sistema
        destino.setdefault(f["nombre"], []).append(f)

    def armar(seccion: str, juntos: dict[str, list[dict]]) -> list[dict]:
        grupos = []
        for nombre, procesos in juntos.items():
            lanzador = apps.de_ejecutable(nombre)
            grupos.append(_grupo(f"{seccion[0]}:{nombre}", (lanzador or {}).get("nombre") or nombre,
                                 (lanzador or {}).get("icono", ""), procesos,
                                 nombre if lanzador and lanzador.get("nombre") != nombre else ""))
        return grupos

    return {"apps": grupos_apps, "fondo": armar("fondo", fondo), "sistema": armar("sistema", sistema)}


ORDENES = ("nombre", "cpu", "mem", "disco")


def ordenar(grupos: list[dict], por: str = "nombre") -> list[dict]:
    if por == "nombre":
        return sorted(grupos, key=lambda g: (g["nombre"].lower(), g["clave"]))
    return sorted(grupos, key=lambda g: (-g[por], g["nombre"].lower()))


def coincide(grupo: dict, filtro: str) -> bool:
    f = filtro.strip().lower()
    if not f:
        return True
    if f in grupo["nombre"].lower() or f in grupo["subtitulo"].lower():
        return True
    return any(f in p["nombre"].lower() or f == str(p["pid"]) for p in grupo["procesos"])


# --- Acciones sobre procesos ---

def _con_pkexec(argv: list[str]) -> str | None:
    """Para procesos de root u otros usuarios: lo pide polkit (te pregunta tu contraseña)."""
    if not shutil.which("pkexec"):
        return "Ese proceso es del sistema y aquí no hay pkexec para pedir permiso."
    r = subprocess.run(["pkexec", *argv], capture_output=True, text=True)
    if r.returncode in (126, 127):
        return "No se dio el permiso."
    return None if r.returncode == 0 else (r.stderr.strip() or "No se pudo.")


def senal(pids: list[int], sig: int) -> str | None:
    """Manda la señal a todos (los hijos primero). Si alguno no es tuyo, lo pide con pkexec. None = bien."""
    ajenos = []
    for pid in sorted(pids, reverse=True):
        try:
            os.kill(pid, sig)
        except ProcessLookupError:
            pass
        except PermissionError:
            ajenos.append(pid)
    if ajenos:
        return _con_pkexec(["kill", f"-{signal.Signals(sig).name[3:]}", *map(str, ajenos)])
    return None


def terminar(pids: list[int], forzar: bool = False) -> str | None:
    error = senal(pids, signal.SIGKILL if forzar else signal.SIGTERM)
    if not forzar and not error:
        # Un proceso en pausa (SIGSTOP) no atiende SIGTERM hasta que lo reanudan: se reanuda para que termine.
        senal(pids, signal.SIGCONT)
    return error


def suspender(pids: list[int], si: bool) -> str | None:
    return senal(pids, signal.SIGSTOP if si else signal.SIGCONT)


PRIORIDADES = {"alta": -5, "normal": 0, "baja": 10, "minima": 19}


def prioridad(pids: list[int], nivel: str) -> str | None:
    """Cambia el «nice». Subirla (alta) o tocar procesos ajenos pide permiso con pkexec."""
    valor = PRIORIDADES[nivel]
    pendientes = []
    for pid in pids:
        try:
            os.setpriority(os.PRIO_PROCESS, pid, valor)
        except ProcessLookupError:
            pass
        except PermissionError:
            pendientes.append(pid)
    if pendientes:
        return _con_pkexec(["renice", "-n", str(valor), "-p", *map(str, pendientes)])
    return None


def prioridad_de(pid: int) -> str:
    try:
        n = os.getpriority(os.PRIO_PROCESS, pid)
    except OSError:
        return ""
    return "alta" if n < 0 else "normal" if n < 5 else "baja" if n < 15 else "minima"


def cerrar_ventanas(direcciones: list[str]) -> None:
    """Cierra las ventanas con amabilidad (como la X): la app puede preguntar si guardas."""
    for d in direcciones:
        for orden in (f'hl.dsp.window.close({{ window = "address:{d}" }})', f"closewindow address:{d}"):
            try:
                if subprocess.run(["hyprctl", "dispatch", orden], capture_output=True, timeout=3).returncode == 0:
                    break
            except (OSError, subprocess.TimeoutExpired):
                break


def ubicacion(pid: int) -> str:
    try:
        return psutil.Process(pid).exe()
    except (psutil.Error, OSError):
        return ""


# --- Rendimiento ---

def _leer(ruta: str | Path, defecto: str = "") -> str:
    try:
        return Path(ruta).read_text(encoding="utf-8", errors="replace").strip()
    except OSError:
        return defecto


def nombre_cpu() -> str:
    for linea in _leer("/proc/cpuinfo").splitlines():
        if linea.startswith("model name"):
            n = linea.split(":", 1)[1].strip()
            n = re.sub(r"\((R|TM)\)", "", n)
            n = re.sub(r"\s+(\d+|Dual|Quad|Six|Eight|Twelve|Sixteen)-Core Processor$|\s+(CPU\s+)?@.*$|\s+Processor$", "", n)
            return re.sub(r"\s{2,}", " ", n).strip()
    return "Procesador"


def temperatura_cpu() -> float | None:
    try:
        sensores = psutil.sensors_temperatures()
    except (AttributeError, OSError):
        return None
    for chip, preferidas in (("k10temp", ("Tctl", "Tdie")), ("zenpower", ("Tdie", "Tctl")),
                             ("coretemp", ("Package id 0",)), ("cpu_thermal", ("",)), ("acpitz", ("",))):
        for s in sensores.get(chip, []):
            if not preferidas[0] or s.label in preferidas:
                return s.current
        if sensores.get(chip):
            return sensores[chip][0].current
    return None


def discos_fisicos() -> dict[str, dict]:
    """nvme0n1, sda… (sin particiones, loop, zram ni device-mapper): modelo, tipo y capacidad."""
    discos = {}
    for d in sorted(Path("/sys/block").glob("*")):
        if not (d / "device").exists() or d.name.startswith(("loop", "zram", "ram", "dm-", "sr")):
            continue
        rotatorio = _leer(d / "queue/rotational") == "1"
        tipo = "NVMe" if d.name.startswith("nvme") else "HDD" if rotatorio else "SSD"
        removible = _leer(d / "removable") == "1"
        if int(_leer(d / "size", "0") or 0) == 0:     # lector de tarjetas sin tarjeta
            continue
        discos[d.name] = {
            "id": d.name, "modelo": _leer(d / "device/model") or d.name, "tipo": "USB" if removible else tipo,
            "capacidad": int(_leer(d / "size", "0") or 0) * 512,
        }
    return discos


def uso_de_discos(discos: dict[str, dict]) -> dict[str, tuple[int, int]]:
    """Por disco: (usado, total) de sus particiones montadas (btrfs con varios subvolúmenes cuenta una vez)."""
    uso: dict[str, tuple[int, int]] = {}
    vistos: set[str] = set()
    for part in psutil.disk_partitions(all=False):
        dev = os.path.basename(part.device)
        disco = next((d for d in discos if dev.startswith(d)), None)
        if not disco or part.device in vistos:
            continue
        vistos.add(part.device)
        try:
            u = psutil.disk_usage(part.mountpoint)
        except OSError:
            continue
        usado, total = uso.get(disco, (0, 0))
        uso[disco] = (usado + u.used, total + u.total)
    return uso


def interfaces_red() -> dict[str, dict]:
    """Las tarjetas de red de verdad (con /device): Wi-Fi o Ethernet. Sin lo, docker, veth, virbr…"""
    redes = {}
    for d in sorted(Path("/sys/class/net").glob("*")):
        if not (d / "device").exists():
            continue
        wifi = (d / "wireless").exists() or (d / "phy80211").exists()
        redes[d.name] = {"id": d.name, "tipo": "Wi-Fi" if wifi else "Ethernet"}
    return redes


def _ssid(interfaz: str) -> str:
    if shutil.which("iwgetid"):
        try:
            return subprocess.run(["iwgetid", "-r", interfaz], capture_output=True, text=True, timeout=2).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            return ""
    return ""


def gpus_nvidia() -> list[dict]:
    if not shutil.which("nvidia-smi"):
        return []
    try:
        salida = subprocess.run(
            ["nvidia-smi", "--query-gpu=index,name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw",
             "--format=csv,noheader,nounits"], capture_output=True, text=True, timeout=4).stdout
    except (OSError, subprocess.TimeoutExpired):
        return []
    gpus = []
    for linea in salida.strip().splitlines():
        partes = [x.strip() for x in linea.split(",")]
        if len(partes) < 7:
            continue

        def num(x: str) -> float:
            try:
                return float(x)
            except ValueError:
                return 0.0
        gpus.append({"id": f"nvidia{partes[0]}", "nombre": partes[1].replace("NVIDIA ", ""), "uso": num(partes[2]),
                     "vramUsada": num(partes[3]) * 1024 ** 2, "vramTotal": num(partes[4]) * 1024 ** 2,
                     "temp": num(partes[5]), "watts": num(partes[6])})
    return gpus


_nombres_pci: dict[str, str] = {}


def _nombre_pci(ranura: str) -> str:
    if ranura not in _nombres_pci:
        nombre = "GPU"
        if shutil.which("lspci"):
            try:
                linea = subprocess.run(["lspci", "-s", ranura], capture_output=True, text=True, timeout=3).stdout
                nombre = linea.split(": ", 1)[1].strip() if ": " in linea else nombre
                corchetes = re.findall(r"\[([^\]]+)\]", nombre)
                nombre = corchetes[-1] if corchetes else nombre
            except (OSError, subprocess.TimeoutExpired):
                pass
        _nombres_pci[ranura] = nombre
    return _nombres_pci[ranura]


def gpus_sysfs() -> list[dict]:
    """AMD (gpu_busy_percent) e Intel (sin uso: solo nombre). NVIDIA va por nvidia-smi."""
    gpus = []
    for card in sorted(Path("/sys/class/drm").glob("card[0-9]")):
        dev = card / "device"
        vendor = _leer(dev / "vendor")
        if vendor not in ("0x1002", "0x8086"):
            continue
        uso = _leer(dev / "gpu_busy_percent")
        temp = None
        for h in dev.glob("hwmon/hwmon*/temp1_input"):
            try:
                temp = int(_leer(h, "0")) / 1000
            except ValueError:
                pass
        gpus.append({"id": card.name, "nombre": _nombre_pci(dev.resolve().name), "uso": float(uso) if uso.isdigit() else -1,
                     "vramUsada": float(_leer(dev / "mem_info_vram_used", "0") or 0),
                     "vramTotal": float(_leer(dev / "mem_info_vram_total", "0") or 0),
                     "temp": temp or 0.0, "watts": 0.0})
    return gpus


class Rendimiento:
    """Lo de la página «Rendimiento». Cada `muestrear()` mide desde el anterior (velocidades, % activo)."""

    def __init__(self):
        psutil.cpu_percent(percpu=True)
        self.cpu_nombre = nombre_cpu()
        self.fisicos = psutil.cpu_count(logical=False) or CPUS
        self.discos = discos_fisicos()
        self.redes = interfaces_red()
        self._antes: tuple[float, dict, dict] | None = None
        self._uso_discos: dict[str, tuple[int, int]] = {}
        self._uso_cuando = 0.0
        self._ssid: dict[str, str] = {}
        self._ssid_cuando = 0.0

    def muestrear(self, procesos: int = 0, hilos: int = 0) -> dict:
        ahora = time.monotonic()
        nucleos = psutil.cpu_percent(percpu=True)
        frec = psutil.cpu_freq()
        mem, swap = psutil.virtual_memory(), psutil.swap_memory()
        io = psutil.disk_io_counters(perdisk=True) or {}
        red = psutil.net_io_counters(pernic=True) or {}
        dt = ahora - self._antes[0] if self._antes else 0.0
        io_antes, red_antes = (self._antes[1], self._antes[2]) if self._antes else ({}, {})
        self._antes = (ahora, io, red)

        if ahora - self._uso_cuando > 10:
            self._uso_discos = uso_de_discos(self.discos)
            self._uso_cuando = ahora
        discos = []
        for ident, d in self.discos.items():
            c, a = io.get(ident), io_antes.get(ident)
            lectura = escritura = activo = 0.0
            if c and a and dt > 0:
                lectura = max(0.0, (c.read_bytes - a.read_bytes) / dt)
                escritura = max(0.0, (c.write_bytes - a.write_bytes) / dt)
                if hasattr(c, "busy_time"):
                    activo = min(100.0, max(0.0, (c.busy_time - a.busy_time) / (dt * 10)))
            usado, total = self._uso_discos.get(ident, (0, 0))
            discos.append({**d, "lectura": lectura, "escritura": escritura, "activo": activo,
                           "usado": usado, "total": total or d["capacidad"]})

        if ahora - self._ssid_cuando > 15:
            self._ssid = {i: _ssid(i) for i, r in self.redes.items() if r["tipo"] == "Wi-Fi"}
            self._ssid_cuando = ahora
        estados = psutil.net_if_stats()
        direcciones = psutil.net_if_addrs()
        redes = []
        for ident, r in self.redes.items():
            if not getattr(estados.get(ident), "isup", False):
                continue
            c, a = red.get(ident), red_antes.get(ident)
            envio = recepcion = 0.0
            if c and a and dt > 0:
                envio = max(0.0, (c.bytes_sent - a.bytes_sent) / dt)
                recepcion = max(0.0, (c.bytes_recv - a.bytes_recv) / dt)
            ipv4 = next((x.address for x in direcciones.get(ident, []) if x.family == socket.AF_INET), "")
            redes.append({**r, "envio": envio, "recepcion": recepcion, "ipv4": ipv4,
                          "ssid": self._ssid.get(ident, ""), "velocidad": getattr(estados.get(ident), "speed", 0)})

        return {
            "cpu": {"uso": sum(nucleos) / max(1, len(nucleos)), "nucleos": nucleos, "nombre": self.cpu_nombre,
                    "ghz": (frec.current / 1000) if frec else 0.0, "maxGhz": (frec.max / 1000) if frec and frec.max else 0.0,
                    "fisicos": self.fisicos, "logicos": CPUS, "temp": temperatura_cpu() or 0.0,
                    "procesos": procesos, "hilos": hilos, "encendido": time.time() - psutil.boot_time()},
            "mem": {"total": mem.total, "usada": mem.total - mem.available, "disponible": mem.available,
                    "cache": getattr(mem, "cached", 0) + getattr(mem, "buffers", 0), "uso": mem.percent,
                    "swapTotal": swap.total, "swapUsada": swap.used},
            "discos": discos,
            "redes": redes,
            "gpus": gpus_nvidia() + gpus_sysfs(),
        }
