"""Lo que no necesita Qt: el protocolo del aviso, mandar y recibir con sendme, y dónde queda todo.

El aviso es JSON por líneas sobre TCP, en el puerto 1770. Solo lleva nombres, tamaño y el ticket de sendme:
el archivo nunca pasa por ahí. Quien manda espera {"r": "aceptado"}, luego {"r": "avance", "bytes": N} y al
final {"r": "listo"}. Sendme se corre en una carpeta temporal porque deja ahí su base de datos.
"""
from __future__ import annotations

import getpass
import ipaddress
import json
import os
import re
import shutil
import signal
import socket
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Callable

PUERTO = int(os.environ.get("VORTEX_SHARED_PUERTO", "1770"))
VERSION_PROTOCOLO = 1
LOTE = "vortex-shared-lote"   # sendme manda una sola ruta: varios archivos van en una carpeta con este nombre
MAX_LINEA = 64 * 1024
MAX_NOMBRES = 20
TICKET = re.compile(r"blob[a-z0-9]{10,4000}")
USUARIO = re.compile(r"[A-Za-z0-9._-]{1,64}")
SIN_AVANCE_S = float(os.environ.get("VORTEX_SHARED_SIN_AVANCE", "120"))   # el otro se apagó o se cayó la red

MENSAJES = {
    "rechazado": "En la otra compu dijeron que no.",
    "usuario": "Esa compu no tiene ese usuario.",
    "ocupado": "La otra compu está ocupada (otro envío o un juego). Intenta en un rato.",
    "espacio": "A la otra compu no le alcanza el disco.",
    "sin_respuesta": "Nadie aceptó a tiempo.",
    "error": "La otra compu no pudo bajar los archivos.",
}


class ErrorShared(Exception):
    """Un problema que se le puede decir tal cual a la persona."""


# Sendme nunca sobrevive a quien lo lanzó: si la ventana se cae no debe seguir compartiendo tus archivos. Este sh
# corre sendme y se queda leyendo su entrada; al morir Python la tubería se cierra y corta a sendme.
_GUARDIA = ('exec 3<&0; sendme "$@" </dev/null 3<&- & p=$!; (cat <&3 >/dev/null; kill "$p" 2>/dev/null) & g=$!; '
           'exec 3<&-; wait "$p"; c=$?; kill "$g" 2>/dev/null; exit "$c"')


def lanzar_sendme(*args: str, cwd: Path, salida: int = subprocess.DEVNULL) -> subprocess.Popen:
    return subprocess.Popen(["sh", "-c", _GUARDIA, "sendme", *args], cwd=cwd, stdin=subprocess.PIPE,
                            stdout=salida, stderr=subprocess.DEVNULL, text=True, start_new_session=True)


def limpiar_viejos() -> None:
    """Lo que dejó un envío que se cortó de golpe (más de un día)."""
    try:
        for d in carpeta_cache().iterdir():
            if d.is_dir() and d.name.startswith(("envio-", ".vortex-shared-")) and time.time() - d.stat().st_mtime > 86400:
                shutil.rmtree(d, ignore_errors=True)
    except OSError:
        pass


# --- Dónde ---
def carpeta_recibidos() -> Path:
    prueba = os.environ.get("VORTEX_SHARED_CARPETA")
    if prueba:
        return Path(prueba)
    try:
        d = subprocess.run(["xdg-user-dir", "DOWNLOAD"], capture_output=True, text=True, timeout=3).stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        d = ""
    return Path(d or Path.home() / "Descargas") / "Vortexine Shared"


def carpeta_cache() -> Path:
    return Path(os.environ.get("XDG_CACHE_HOME") or Path.home() / ".cache") / "vortex-shared"


def humano(n: int) -> str:
    valor = float(n)
    for unidad in ("B", "KB", "MB", "GB"):
        if valor < 1024:
            return f"{valor:.0f} {unidad}" if unidad == "B" else f"{valor:.1f} {unidad}"
        valor /= 1024
    return f"{valor:.1f} TB"


# --- Quién puede avisar ---
def ip_permitida(texto: str) -> bool:
    """Solo la misma red o Tailscale: nadie de internet te puede llenar de avisos."""
    try:
        ip = ipaddress.ip_address(texto.split("%")[0])
    except ValueError:
        return False
    if getattr(ip, "ipv4_mapped", None):
        ip = ip.ipv4_mapped
    if ip.version == 4 and ip in ipaddress.ip_network("100.64.0.0/10"):
        return True
    return ip.is_private or ip.is_loopback or ip.is_link_local


def limpiar_nombre(nombre: str) -> str:
    """Los nombres vienen de la otra compu y se ven en un aviso: sin saltos, controles ni marcado."""
    nombre = re.sub(r"[\x00-\x1f\x7f<>&]", "", str(nombre)).strip()
    return nombre[:80] or "archivo"


def validar_peticion(datos: object) -> dict:
    """La petición ya revisada, o ValueError. El ticket va a un comando: se revisa carácter por carácter."""
    if not isinstance(datos, dict) or datos.get("v") != VERSION_PROTOCOLO:
        raise ValueError("versión")
    ticket, usuario, tam, nombres = datos.get("ticket"), datos.get("usuario"), datos.get("tam"), datos.get("nombres")
    if not isinstance(ticket, str) or not TICKET.fullmatch(ticket):
        raise ValueError("ticket")
    if not isinstance(usuario, str) or not USUARIO.fullmatch(usuario):
        raise ValueError("usuario")
    if not isinstance(tam, int) or isinstance(tam, bool) or tam < 0:
        raise ValueError("tamaño")
    if not isinstance(nombres, list) or not nombres:
        raise ValueError("nombres")
    n = datos.get("n", len(nombres))
    return {"ticket": ticket, "usuario": usuario, "tam": tam,
            "nombres": [limpiar_nombre(x) for x in nombres[:MAX_NOMBRES]],
            "n": n if isinstance(n, int) and not isinstance(n, bool) and n > 0 else len(nombres),
            "de": limpiar_nombre(datos.get("de", "alguien"))}


def texto_peticion(p: dict) -> str:
    primero = p["nombres"][0]
    return f"{primero} ({humano(p['tam'])})" if p["n"] == 1 else f"{primero} y {p['n'] - 1} más ({humano(p['tam'])})"


# --- Qué se manda ---
def tamano(ruta: Path) -> int:
    if not ruta.is_dir():
        return ruta.stat().st_size
    total = 0
    for base, _, archivos in os.walk(ruta):
        for a in archivos:
            try:
                total += os.stat(os.path.join(base, a)).st_size
            except OSError:
                pass
    return total


def _enlazar(origen: str, destino: str) -> None:
    """Enlace duro: no ocupa disco. Si el archivo está en otro disco, se copia."""
    try:
        os.link(origen, destino)
    except OSError:
        shutil.copy2(origen, destino)


class Envio:
    """Un sendme mandando esos archivos. Sirve mientras siga vivo; `terminar` limpia todo."""

    def __init__(self, rutas: list[str | Path]):
        self.rutas = [Path(r).expanduser().resolve() for r in rutas]
        faltan = [r.name for r in self.rutas if not r.exists()]
        if not self.rutas or faltan:
            raise ErrorShared(f"No encuentro {', '.join(faltan)}." if faltan else "Elige algo para mandar.")
        if not shutil.which("sendme"):
            raise ErrorShared("Falta sendme (sudo pacman -S sendme).")
        self.nombres = [r.name for r in self.rutas]
        self.tam = sum(tamano(r) for r in self.rutas)
        self.ticket = ""
        self._proc: subprocess.Popen | None = None
        self._lote: Path | None = None
        carpeta_cache().mkdir(parents=True, exist_ok=True)
        limpiar_viejos()
        self._cwd = Path(tempfile.mkdtemp(prefix="envio-", dir=carpeta_cache()))

    def _preparar_lote(self) -> Path:
        # Junto a los archivos si están en otro disco que la caché: ahí sí se pueden enlazar.
        base = carpeta_cache()
        padre = self.rutas[0].parent
        if padre.stat().st_dev != base.stat().st_dev and os.access(padre, os.W_OK):
            base = padre
        self._lote = Path(tempfile.mkdtemp(prefix=".vortex-shared-", dir=base))
        destino = self._lote / LOTE
        destino.mkdir()
        usados: set[str] = set()
        for ruta in self.rutas:
            nombre, n = ruta.name, 2
            while nombre in usados:   # dos «foto.jpg» de carpetas distintas
                nombre, n = f"{ruta.stem} ({n}){ruta.suffix}", n + 1
            usados.add(nombre)
            if ruta.is_dir():
                shutil.copytree(ruta, destino / nombre, copy_function=_enlazar)
            else:
                _enlazar(str(ruta), str(destino / nombre))
        return destino

    def preparar(self, espera_s: float = 900) -> str:
        """Arranca sendme y devuelve el ticket. Calcular la huella de algo grande tarda un rato."""
        try:
            ruta = self.rutas[0] if len(self.rutas) == 1 else self._preparar_lote()
            self._proc = lanzar_sendme("send", str(ruta), cwd=self._cwd, salida=subprocess.PIPE)
        except OSError as e:
            self.terminar()
            raise ErrorShared(f"No se pudo preparar el envío: {e.strerror or e}") from e
        fin = time.monotonic() + espera_s
        assert self._proc.stdout is not None
        while time.monotonic() < fin:
            linea = self._proc.stdout.readline()
            if not linea:
                break
            encontrado = re.search(r"\bblob[a-z0-9]+", linea)
            if encontrado:
                self.ticket = encontrado.group(0)
                # sendme sigue escribiendo; si nadie lee, se atora cuando se llena la tubería.
                self._vaciar = subprocess.Popen(["cat"], stdin=self._proc.stdout, stdout=subprocess.DEVNULL, start_new_session=True)
                self._proc.stdout.close()
                return self.ticket
        self.terminar()
        raise ErrorShared("No se pudo preparar el envío.")

    def peticion(self, usuario: str) -> dict:
        return {"v": VERSION_PROTOCOLO, "ticket": self.ticket, "usuario": usuario, "tam": self.tam,
                "nombres": self.nombres[:MAX_NOMBRES], "n": len(self.nombres), "de": f"{getpass.getuser()}@{socket.gethostname()}"}

    def terminar(self) -> None:
        if self._proc and self._proc.stdin:
            try:
                self._proc.stdin.close()
            except OSError:
                pass
        if self._proc and self._proc.poll() is None:
            try:
                os.killpg(self._proc.pid, signal.SIGTERM)
                self._proc.wait(timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                try:
                    os.killpg(self._proc.pid, signal.SIGKILL)
                    self._proc.wait(timeout=5)
                except (OSError, subprocess.TimeoutExpired):
                    pass
        vaciar = getattr(self, "_vaciar", None)
        if vaciar:
            try:
                vaciar.wait(timeout=3)
            except subprocess.TimeoutExpired:
                vaciar.kill()
                vaciar.wait()
        shutil.rmtree(self._cwd, ignore_errors=True)
        if self._lote:
            shutil.rmtree(self._lote, ignore_errors=True)


def _destino_de(host: str) -> tuple[str, int]:
    host = host.strip().strip("[]")
    if not host or re.search(r"[\s/]", host):
        raise ErrorShared("Escribe la IP o el nombre de la otra compu.")
    if re.fullmatch(r"([0-9a-fA-F]{2}[:-]){5}[0-9a-fA-F]{2}", host):
        raise ErrorShared("Con una MAC no se puede llegar a otra compu: escribe su IP o su nombre en Tailscale.")
    return host, PUERTO


def mandar(host: str, usuario: str, envio: Envio, al_estado: Callable[[str, int], None] | None = None,
           cancelado: Callable[[], bool] | None = None, espera_s: float = 150) -> None:
    """Avisa a la otra compu y espera a que baje todo. ErrorShared si no se pudo, con el motivo."""
    if not USUARIO.fullmatch(usuario.strip()):
        raise ErrorShared("Escribe el usuario de la otra compu.")
    destino = _destino_de(host)
    al_estado = al_estado or (lambda _e, _b: None)
    try:
        conexion = socket.create_connection(destino, timeout=8)
    except socket.gaierror as e:
        raise ErrorShared(f"No encuentro «{host}». Revisa la IP o el nombre.") from e
    except OSError as e:
        raise ErrorShared(f"«{host}» no contesta. Si está lejos, usen Tailscale o un código.") from e
    with conexion:
        conexion.settimeout(1)
        conexion.sendall((json.dumps(envio.peticion(usuario.strip())) + "\n").encode())
        al_estado("esperando", 0)
        ultimo = time.monotonic()
        limite = espera_s
        buffer = b""
        while True:
            if cancelado and cancelado():
                raise ErrorShared("Cancelado.")
            if time.monotonic() - ultimo > limite:
                raise ErrorShared("Se cortó la conexión con la otra compu.")
            try:
                trozo = conexion.recv(4096)
            except TimeoutError:
                continue
            except OSError as e:
                raise ErrorShared("Se cortó la conexión con la otra compu.") from e
            if not trozo:
                raise ErrorShared("Se cortó la conexión con la otra compu.")
            buffer += trozo
            while b"\n" in buffer:
                linea, buffer = buffer.split(b"\n", 1)
                try:
                    r = json.loads(linea)
                    respuesta = r["r"]
                except (ValueError, KeyError, TypeError):
                    raise ErrorShared("La otra compu contestó algo raro. ¿Tiene la misma versión?") from None
                ultimo = time.monotonic()
                if respuesta == "aceptado":
                    limite = SIN_AVANCE_S + 30
                    al_estado("mandando", 0)
                elif respuesta == "avance":
                    al_estado("mandando", int(r.get("bytes", 0)))
                elif respuesta == "listo":
                    return
                else:
                    raise ErrorShared(MENSAJES.get(str(respuesta), "La otra compu no pudo recibirlos."))


# --- Recibir ---
def _peso(carpeta: Path) -> int:
    total = 0
    for base, _, archivos in os.walk(carpeta):
        for a in archivos:
            try:
                total += os.lstat(os.path.join(base, a)).st_size
            except OSError:
                pass
    return total


def _sin_pisar(destino: Path) -> Path:
    if not destino.exists():
        return destino
    n = 2
    while True:
        otro = destino.with_name(f"{destino.stem} ({n}){destino.suffix}")
        if not otro.exists():
            return otro
        n += 1


def recibir(ticket: str, al_avance: Callable[[int], None] | None = None,
            cancelado: Callable[[], bool] | None = None, tam: int = 0) -> list[Path]:
    """Baja todo a la carpeta de recibidos y devuelve lo que quedó. Nunca pisa algo que ya tengas."""
    ticket = ticket.strip()
    if not TICKET.fullmatch(ticket):
        raise ErrorShared("Ese código no parece bueno: cópialo completo.")
    if not shutil.which("sendme"):
        raise ErrorShared("Falta sendme (sudo pacman -S sendme).")
    final = carpeta_recibidos()
    try:
        final.mkdir(parents=True, exist_ok=True)
        temporal = Path(tempfile.mkdtemp(prefix=".recibiendo-", dir=final))
    except OSError as e:
        raise ErrorShared(f"No se pudo escribir en {final}.") from e
    proc = None
    try:
        proc = lanzar_sendme("receive", ticket, cwd=temporal)
        ultimo_peso, ultimo_cambio = -1, time.monotonic()
        while proc.poll() is None:
            time.sleep(0.5)
            if cancelado and cancelado():
                raise ErrorShared("Cancelado.")
            peso = _peso(temporal)
            if peso != ultimo_peso:
                ultimo_peso, ultimo_cambio = peso, time.monotonic()
                if al_avance:   # al final sendme tiene su copia y la exportada: no pasarse del total
                    al_avance(min(peso, tam) if tam else peso)
            elif time.monotonic() - ultimo_cambio > SIN_AVANCE_S:
                raise ErrorShared("No llega nada: la otra compu se apagó o cerró el envío.")
        if proc.returncode != 0:
            raise ErrorShared("No se pudo bajar: revisa el código y que la otra compu siga mandando.")
        items = [i for i in temporal.iterdir() if not i.name.startswith(".sendme-")]
        if len(items) == 1 and items[0].name == LOTE and items[0].is_dir():
            items = list(items[0].iterdir())
        if not items:
            raise ErrorShared("No llegó nada.")
        quedaron = []
        for item in items:
            destino = _sin_pisar(final / item.name)
            shutil.move(str(item), str(destino))
            quedaron.append(destino)
        return quedaron
    finally:
        if proc:
            if proc.stdin:
                try:
                    proc.stdin.close()
                except OSError:
                    pass
            if proc.poll() is None:
                try:
                    os.killpg(proc.pid, signal.SIGKILL)
                    proc.wait(timeout=5)
                except (OSError, subprocess.TimeoutExpired):
                    pass
        shutil.rmtree(temporal, ignore_errors=True)


# --- Las direcciones ---
def tailscale_estado() -> dict:
    if not shutil.which("tailscale"):
        return {}
    try:
        r = subprocess.run(["tailscale", "status", "--json"], capture_output=True, text=True, timeout=4)
        return json.loads(r.stdout) if r.returncode == 0 else {}
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return {}


def _ipv4(lista: object) -> str:
    return next((x for x in lista if isinstance(x, str) and ":" not in x), "") if isinstance(lista, list) else ""


def mis_direcciones(estado: dict | None = None) -> list[dict]:
    """Lo que le das a otra persona para que te mande algo."""
    estado = tailscale_estado() if estado is None else estado
    direcciones = []
    yo = estado.get("Self") or {}
    ip = _ipv4(yo.get("TailscaleIPs"))
    if ip:
        direcciones.append({"tipo": "Tailscale (de lejos)", "ip": ip})
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(("192.0.2.1", 9))   # no manda nada: solo pregunta qué interfaz usaría
            local = s.getsockname()[0]
        if not local.startswith("127.") and local != ip:
            direcciones.append({"tipo": "Misma red", "ip": local})
    except OSError:
        pass
    return direcciones


def compus_tailscale(estado: dict | None = None) -> list[dict]:
    """Tus otras compus con Linux prendidas en Tailscale, para mandarles con un clic."""
    estado = tailscale_estado() if estado is None else estado
    compus = []
    for peer in (estado.get("Peer") or {}).values():
        if isinstance(peer, dict) and peer.get("Online") and peer.get("OS") == "linux":
            ip = _ipv4(peer.get("TailscaleIPs"))
            if ip:
                compus.append({"nombre": limpiar_nombre(peer.get("HostName") or ip), "ip": ip})
    return sorted(compus, key=lambda c: c["nombre"].lower())


# --- El cortafuegos ---
def _marca_puerto() -> Path:
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config") / "vortex" / "shared-ufw"


def puerto_cerrado() -> bool:
    """¿El cortafuegos está prendido y todavía no se abrió el puerto? Sin eso, recibir no funciona."""
    if _marca_puerto().exists() or not shutil.which("ufw"):
        return False
    try:
        return subprocess.run(["systemctl", "is-active", "--quiet", "ufw"], timeout=5).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def abrir_puerto() -> str:
    """Con ufw prendido, se abre el 1770 una sola vez y solo para Tailscale y redes privadas (pide la
    contraseña). Devuelve qué pasó, para decirlo."""
    if not puerto_cerrado():
        return "listo"
    marca = _marca_puerto()
    p = f"to any port {PUERTO} proto tcp"
    orden = (f"ufw allow in on tailscale0 {p} && ufw allow from 192.168.0.0/16 {p} && "
             f"ufw allow from 10.0.0.0/8 {p} && ufw allow from 172.16.0.0/12 {p}")
    try:
        r = subprocess.run(["pkexec", "sh", "-c", orden], capture_output=True)
    except OSError:
        return "falta"
    if r.returncode != 0:
        return "falta"
    marca.parent.mkdir(parents=True, exist_ok=True)
    marca.touch()
    return "listo"
