"""Instalar AppImages y programas .tar.gz, y abrir lo que corre código (siempre ya confirmado).

- AppImage: se copia a ~/Applications, se le da permiso de ejecución, se sacan su .desktop y su
  ícono (--appimage-extract) y se pone en el lanzador, con un desinstalador.
- Programa .tar.gz: se descomprime en ~/.local/opt/<app>, se busca su ejecutable de verdad (el
  .desktop que trae puede tener un «@EXEC@» de relleno), y se pone en el lanzador con su ícono y un
  desinstalar.sh.
- Scripts (.sh .run .bin), .exe y .jar: se abren en una terminal o con Wine/Java. Nunca con sudo.

Nada se escribe encima de lo que ya existía: si hace falta, `Conflicto` y la persona decide.
"""
from __future__ import annotations

import os
import re
import shlex
import shutil
import stat
import subprocess
import tempfile
from datetime import datetime
from pathlib import Path

from . import comprimidos, registro, rutas
from .comprimidos import Conflicto
from .textos import tr

TAMANOS = (16, 22, 24, 32, 48, 64, 96, 128, 192, 256, 512)
MARCA = "X-Vortex-App-Installer"   # clave que se pone en los .desktop que crea la app


class ErrorInstalar(Exception):
    """Algo que se le dice a la persona tal cual (ya traducido)."""


# ------------------------------------------------------------------------------------ .desktop

def leer_desktop(texto: str) -> list[tuple[str, list[list[str]]]]:
    """[(grupo, [[clave, valor], …]), …] en el orden del archivo (los comentarios se pierden)."""
    grupos: list[tuple[str, list[list[str]]]] = []
    for linea in texto.splitlines():
        s = linea.strip()
        if not s or s.startswith("#"):
            continue
        if s.startswith("[") and s.endswith("]"):
            grupos.append((s[1:-1], []))
        elif "=" in s and grupos:
            k, v = s.split("=", 1)
            grupos[-1][1].append([k.strip(), v.strip()])
    return grupos


def valor(grupos, clave: str, grupo: str = "Desktop Entry") -> str:
    for nombre, pares in grupos:
        if nombre == grupo:
            for k, v in pares:
                if k == clave:
                    return v
    return ""


def poner(grupos, clave: str, nuevo: str | None, grupo: str = "Desktop Entry") -> None:
    for nombre, pares in grupos:
        if nombre != grupo:
            continue
        for par in pares:
            if par[0] == clave:
                if nuevo is None:
                    pares.remove(par)
                else:
                    par[1] = nuevo
                return
        if nuevo is not None:
            pares.append([clave, nuevo])
        return


def escribir_desktop(grupos) -> str:
    return "\n".join("[%s]\n%s\n" % (g, "\n".join("%s=%s" % (k, v) for k, v in pares)) for g, pares in grupos)


def comillas_exec(ruta: str) -> str:
    """Una ruta para la línea Exec= (la especificación pide comillas dobles y escapar " ` $ \\)."""
    if re.fullmatch(r"[A-Za-z0-9_@%+=:,./-]+", ruta):
        return ruta
    return '"' + re.sub(r'(["`$\\])', r"\\\1", ruta) + '"'


def _partes_exec(exec_: str) -> list[str]:
    try:
        return shlex.split(exec_)
    except ValueError:
        return exec_.split()


def _nuevo_exec(viejo: str, programa: str, prefijo: str = "") -> str:
    """Cambia el programa de la línea Exec por el instalado y deja sus argumentos (%U, %F…)."""
    partes = _partes_exec(viejo)
    resto = [p for p in partes[1:]] if partes else []
    while resto and partes and partes[0] == "env" and "=" in resto[0]:   # «env VAR=1 programa %U»
        resto = resto[1:]
    if partes and partes[0] == "env" and resto:
        resto = resto[1:]
    args = " ".join(p if p.startswith("%") or re.fullmatch(r"[\w@%+=:,./-]+", p) else comillas_exec(p) for p in resto)
    return (prefijo + comillas_exec(programa) + (" " + args if args else "")).strip()


def _preparar_desktop(grupos, nombre: str, programa: str, icono: str | None, id_: str, prefijo: str = "") -> None:
    if not any(g == "Desktop Entry" for g, _ in grupos):
        grupos.insert(0, ("Desktop Entry", []))
    poner(grupos, "Type", "Application")
    if not valor(grupos, "Name"):
        poner(grupos, "Name", nombre)
    poner(grupos, "Exec", _nuevo_exec(valor(grupos, "Exec") or programa, programa, prefijo))
    poner(grupos, "TryExec", programa)
    poner(grupos, "Path", None)
    poner(grupos, "DBusActivatable", None)   # sin servicio D-Bus instalado, el lanzador no la abriría
    poner(grupos, "Terminal", valor(grupos, "Terminal") or "false")
    if icono:
        poner(grupos, "Icon", icono)
    poner(grupos, MARCA, id_)
    for g, pares in grupos:   # las acciones del lanzador (clic derecho) también llaman al programa nuevo
        if g.startswith("Desktop Action "):
            ex = valor(grupos, "Exec", g)
            if ex:
                poner(grupos, "Exec", _nuevo_exec(ex, programa, prefijo), g)


def _desktop_libre(nombre_archivo: str, id_: str) -> Path:
    """La ruta para el .desktop: con su nombre original si está libre (así el ícono de la barra
    coincide con el de la ventana) o si ya era de esta misma instalación; si no, con prefijo."""
    carpeta = rutas.lanzadores()
    base = nombre_archivo if nombre_archivo.endswith(".desktop") else nombre_archivo + ".desktop"
    for candidato in (base, "vortex-app-installer-" + base):
        p = carpeta / candidato
        if not p.exists() or _es_de(p, id_):
            return p
    return comprimidos.nombre_libre(carpeta / ("vortex-app-installer-" + base))


def _es_de(desktop: Path, id_: str) -> bool:
    try:
        return valor(leer_desktop(desktop.read_text(encoding="utf-8", errors="replace")), MARCA) == id_
    except OSError:
        return False


# ------------------------------------------------------------------------------------ íconos

def tamano_png(ruta: Path) -> int:
    try:
        with open(ruta, "rb") as f:
            cabeza = f.read(24)
    except OSError:
        return 0
    if cabeza[:8] != b"\x89PNG\r\n\x1a\n":
        return 0
    return int.from_bytes(cabeza[16:20], "big")


def _tipo_imagen(ruta: Path) -> str:
    try:
        with open(ruta, "rb") as f:
            cabeza = f.read(512)
    except OSError:
        return ""
    if cabeza.startswith(b"\x89PNG"):
        return "png"
    if b"<svg" in cabeza or cabeza.lstrip().startswith(b"<?xml"):
        return "svg"
    if cabeza.startswith(b"/* XPM */"):
        return "xpm"
    return ""


def instalar_icono(origen: Path, nombre: str, id_: str) -> tuple[str, list[Path]]:
    """Copia el ícono al tema hicolor de tu usuario. Devuelve (nombre para Icon=, archivos creados)."""
    tipo = _tipo_imagen(origen)
    if not tipo:
        return "", []
    if tipo == "svg":
        carpeta = rutas.iconos() / "scalable" / "apps"
    else:
        lado = tamano_png(origen) if tipo == "png" else 48
        tam = min(TAMANOS, key=lambda t: abs(t - lado)) if lado else 256
        carpeta = rutas.iconos() / ("%dx%d" % (tam, tam)) / "apps"
    carpeta.mkdir(parents=True, exist_ok=True)
    for candidato in (nombre, "vortex-app-installer-" + nombre):
        destino = carpeta / ("%s.%s" % (candidato, tipo))
        propio = any(str(destino) in (e.get("rutas") or []) for e in registro.cargar() if e["id"] == id_)
        if not os.path.lexists(destino) or propio:
            shutil.copyfile(origen, destino)
            return candidato, [destino]
    destino = comprimidos.nombre_libre(carpeta / ("vortex-app-installer-%s.%s" % (nombre, tipo)))
    shutil.copyfile(origen, destino)
    return destino.stem, [destino]


# ------------------------------------------------------------------------------------ desinstalador

def escribir_desinstalador(ruta: Path, nombre: str, borrar: list[Path]) -> None:
    lineas = ["#!/bin/sh",
              "# Desinstalador de %s, creado por Vortex App Installer el %s." % (nombre, datetime.now().strftime("%d/%m/%Y")),
              "# También se puede desinstalar desde la app, en la pestaña «Instalado» (así va a la papelera).",
              "set -u"]
    for p in borrar:
        lineas.append("rm -rf -- %s" % shlex.quote(str(p)))
    lineas += ["command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q %s 2>/dev/null"
               % shlex.quote(str(rutas.lanzadores())),
               "echo %s" % shlex.quote(tr("desinstalado_terminal", nombre=nombre)), ""]
    ruta.parent.mkdir(parents=True, exist_ok=True)
    ruta.write_text("\n".join(lineas), encoding="utf-8")
    ruta.chmod(0o755)


# ------------------------------------------------------------------------------------ AppImage

def hay_fuse2() -> bool:
    return any(os.path.exists(p) for p in ("/usr/lib/libfuse.so.2", "/usr/lib64/libfuse.so.2",
                                           "/lib/x86_64-linux-gnu/libfuse.so.2", "/usr/lib/x86_64-linux-gnu/libfuse.so.2"))


def _entorno_appimage() -> dict:
    env = dict(os.environ)
    if not hay_fuse2():   # sin FUSE 2 los AppImages no montan: así se descomprimen y corren igual
        env["APPIMAGE_EXTRACT_AND_RUN"] = "1"
    return env


def _sacar_de_appimage(appimage: Path, carpeta: Path, patron: str) -> None:
    try:
        subprocess.run([str(appimage), "--appimage-extract", patron], cwd=carpeta, stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120,
                       env={k: v for k, v in os.environ.items() if k != "APPIMAGE_EXTRACT_AND_RUN"},
                       check=False, start_new_session=True)
    except (OSError, subprocess.TimeoutExpired):
        pass


def _metadatos_appimage(appimage: Path, carpeta: Path) -> tuple[Path | None, Path | None]:
    """Saca el .desktop y el ícono del AppImage a `carpeta`. Devuelve (desktop, ícono)."""
    raiz = carpeta / "squashfs-root"
    _sacar_de_appimage(appimage, carpeta, "*.desktop")
    if not raiz.is_dir():   # AppImage tipo 1 (una imagen ISO 9660): lo lee bsdtar sin correr nada
        raiz.mkdir(exist_ok=True)
        bsdtar = shutil.which("bsdtar")
        if bsdtar:
            subprocess.run([bsdtar, "-xf", str(appimage), "-C", str(raiz), "--no-same-owner", "*.desktop",
                            ".DirIcon", "*.png", "*.svg"], stdin=subprocess.DEVNULL, capture_output=True,
                           start_new_session=True, check=False)
    desktops = sorted(raiz.glob("*.desktop")) or sorted(raiz.glob("usr/share/applications/*.desktop"))
    desktop = next((d for d in desktops if d.is_file() and not d.is_symlink()), None)
    if desktop is None:
        desktop = next((d for d in desktops if d.exists()), None)
    icono_nombre = ""
    if desktop is not None:
        try:
            icono_nombre = valor(leer_desktop(desktop.read_text(encoding="utf-8", errors="replace")), "Icon")
        except OSError:
            icono_nombre = ""
    icono_nombre = os.path.basename(icono_nombre)
    patrones = [".DirIcon"]
    if icono_nombre:
        patrones += [icono_nombre + ".svg", icono_nombre + ".png", icono_nombre,
                     "usr/share/icons/hicolor/256x256/apps/%s.png" % icono_nombre,
                     "usr/share/icons/hicolor/scalable/apps/%s.svg" % icono_nombre]
    for patron in patrones:
        _sacar_de_appimage(appimage, carpeta, patron)
    candidatos = []
    if icono_nombre:
        candidatos += [raiz / (icono_nombre + ".svg"), raiz / (icono_nombre + ".png"), raiz / icono_nombre,
                       raiz / "usr/share/icons/hicolor/scalable/apps" / (icono_nombre + ".svg"),
                       raiz / "usr/share/icons/hicolor/256x256/apps" / (icono_nombre + ".png")]
    candidatos.append(raiz / ".DirIcon")
    icono = None
    for c in candidatos:
        real = _dentro_de(c, raiz)
        if real is not None and real.is_file() and _tipo_imagen(real):
            icono = real
            break
    return (_dentro_de(desktop, raiz) if desktop else None), icono


def _dentro_de(ruta: Path, raiz: Path) -> Path | None:
    """La ruta real (siguiendo enlaces) si queda dentro de `raiz`; None si se sale o no existe."""
    try:
        real = ruta.resolve(strict=True)
        real.relative_to(raiz.resolve())
        return real
    except (OSError, ValueError, RuntimeError):
        return None


def _copiar(origen: Path, destino: Path, progreso=None, cancelar=None) -> None:
    total = max(1, origen.stat().st_size)
    hecho = 0
    with open(origen, "rb") as f, open(destino, "xb") as out:
        while True:
            if cancelar is not None and cancelar.is_set():
                raise comprimidos.Cancelado()
            bloque = f.read(4 << 20)
            if not bloque:
                break
            out.write(bloque)
            hecho += len(bloque)
            if progreso:
                progreso(0.85 * hecho / total)


def id_appimage(ruta: Path) -> str:
    """Id estable para un AppImage aunque cambie de versión: «BedrockOnLinux-2.2.7-x86_64» → «appimage-bedrockonlinux»."""
    base = re.sub(r"\.appimage$", "", ruta.name, flags=re.I)
    base = re.split(r"[-_ ](?:v?\d)", base, maxsplit=1)[0]
    base = re.sub(r"[-_](x86_64|amd64|aarch64|arm64|i386|linux)$", "", base, flags=re.I)
    return "appimage-" + registro.slug(base)


def instalar_appimage(ruta: Path, reemplazar: bool = False, progreso=None, cancelar=None) -> dict:
    ruta = Path(ruta)
    id_ = id_appimage(ruta)
    previo = registro.buscar(id_)
    destino = rutas.appimages() / ruta.name
    if previo and not reemplazar:
        raise Conflicto(Path(previo.get("abrir") or destino), destino)
    if os.path.lexists(destino) and not (previo and str(destino) in previo.get("rutas", [])):
        if not reemplazar:
            raise Conflicto(destino, comprimidos.nombre_libre(destino))
        destino = comprimidos.nombre_libre(destino)   # no es nuestro: no se toca, va al lado
    if previo:
        registro.deshacer(id_)   # la versión anterior, a la papelera
    destino.parent.mkdir(parents=True, exist_ok=True)
    _copiar(ruta, destino, progreso, cancelar)
    destino.chmod(0o755)
    creados = [destino]
    try:
        with tempfile.TemporaryDirectory(prefix="vortex-appimage-") as tmp:
            desktop_origen, icono_origen = _metadatos_appimage(destino, Path(tmp))
            if progreso:
                progreso(0.95)
            grupos = []
            if desktop_origen is not None:
                grupos = leer_desktop(desktop_origen.read_text(encoding="utf-8", errors="replace"))
            nombre = valor(grupos, "Name") or re.sub(r"\.appimage$", "", ruta.name, flags=re.I)
            base_icono = registro.slug(desktop_origen.stem if desktop_origen else nombre)
            icono = ""
            if icono_origen is not None:
                icono, archivos = instalar_icono(icono_origen, base_icono, id_)
                creados += archivos
            if not grupos:
                grupos = [("Desktop Entry", [["Name", nombre], ["Categories", "Utility;"]])]
            prefijo = "" if hay_fuse2() else "env APPIMAGE_EXTRACT_AND_RUN=1 "
            _preparar_desktop(grupos, nombre, str(destino), icono or "application-x-executable", id_, prefijo)
            desktop = _desktop_libre(desktop_origen.name if desktop_origen else base_icono + ".desktop", id_)
            desktop.parent.mkdir(parents=True, exist_ok=True)
            desktop.write_text(escribir_desktop(grupos), encoding="utf-8")
            creados.append(desktop)
        script = rutas.desinstaladores() / (id_ + ".sh")
        creados.append(script)
        escribir_desinstalador(script, nombre, creados)
    except BaseException:
        for p in creados:   # a medias no se queda nada
            if p.is_file() or p.is_symlink():
                p.unlink(missing_ok=True)
        raise
    registro.refrescar_lanzador()
    entrada = registro.agregar({"id": id_, "tipo": "appimage", "nombre": nombre, "rutas": [str(p) for p in creados],
                                "icono": str(next((p for p in creados if p.suffix in (".png", ".svg")), "")),
                                "archivo": ruta.name, "abrir": str(destino), "desinstalador": str(script)})
    if progreso:
        progreso(1.0)
    return entrada


def abrir_appimage(ruta: Path) -> None:
    """«Solo abrir»: le da permiso de ejecución y lo corre, sin instalar nada."""
    ruta = Path(ruta)
    ruta.chmod(ruta.stat().st_mode | stat.S_IXUSR)
    lanzar([str(ruta)], env=_entorno_appimage(), carpeta=ruta.parent)


# ------------------------------------------------------------------------------------ programas .tar.gz

def _es_programa(p: Path) -> bool:
    try:
        if not p.is_file() or p.is_symlink() or not os.access(p, os.X_OK):
            return False
        with open(p, "rb") as f:
            cabeza = f.read(4)
    except OSError:
        return False
    nombre = p.name.lower()
    if ".so" in nombre or nombre.endswith(".desktop"):
        return False
    return cabeza == b"\x7fELF" or cabeza[:2] == b"#!"


_INSTALADORES = {"instalar.sh", "install.sh", "desinstalar.sh", "uninstall.sh", "installer.sh", "setup.sh"}


def buscar_ejecutable(raiz: Path, grupos, nombre_desktop: str) -> Path | None:
    candidatos = []
    for carpeta, dirs, archivos in os.walk(raiz):
        hondo = len(Path(carpeta).relative_to(raiz).parts)
        if hondo >= 3:
            dirs[:] = []
        dirs[:] = [d for d in dirs if d not in ("_internal", "lib", "lib64", "share", "resources")]
        for a in archivos:
            p = Path(carpeta) / a
            if a.lower() not in _INSTALADORES and _es_programa(p):
                candidatos.append(p)
    if not candidatos:
        return None
    partes = _partes_exec(valor(grupos, "Exec"))
    while partes and (partes[0] == "env" or "=" in partes[0] and not partes[0].startswith("/")):
        partes = partes[1:]
    token = partes[0] if partes else ""
    if token and not token.startswith("/") and "@" not in token:
        directo = raiz / token
        if directo in candidatos:
            return directo
    pistas = [os.path.basename(token), os.path.basename(valor(grupos, "TryExec")), Path(nombre_desktop).stem,
              valor(grupos, "StartupWMClass")]
    pistas = [p.lower() for p in pistas if p and "@" not in p]
    for pista in pistas:
        for c in sorted(candidatos, key=lambda c: len(c.parts)):
            if c.name.lower() == pista:
                return c
    nombre = re.sub(r"[^a-z0-9]", "", valor(grupos, "Name").lower())
    for c in candidatos:
        if nombre and re.sub(r"[^a-z0-9]", "", c.name.lower()) == nombre:
            return c
    poco_hondos = sorted(candidatos, key=lambda c: len(c.parts))
    if len(poco_hondos) == 1 or len(poco_hondos[0].parts) < len(poco_hondos[1].parts):
        return poco_hondos[0]
    return None


def _buscar_icono(raiz: Path, icono: str) -> Path | None:
    if not icono:
        return None
    if "/" in icono:
        p = _dentro_de(raiz / icono.lstrip("/"), raiz) if not icono.startswith("/") else None
        return p if p and _tipo_imagen(p) else None
    mejores: list[tuple[int, Path]] = []
    for carpeta, dirs, archivos in os.walk(raiz):
        for a in archivos:
            base, ext = os.path.splitext(a)
            if base == icono and ext.lower() in (".svg", ".png", ".xpm"):
                p = Path(carpeta) / a
                if _dentro_de(p, raiz) is None:
                    continue
                puntos = 100000 if ext.lower() == ".svg" else tamano_png(p) if ext.lower() == ".png" else 1
                mejores.append((puntos, p))
    return max(mejores, key=lambda x: x[0])[1] if mejores else None


def plan_programa(entradas: list[comprimidos.Entrada], archivo: Path) -> tuple[str, str]:
    """(nombre de la carpeta en ~/.local/opt, nombre del .desktop que trae)."""
    desktops = sorted((e for e in entradas if e.tipo == "archivo" and e.nombre.endswith(".desktop")
                       and e.profundidad <= 3), key=lambda e: e.profundidad)
    nombre_desktop = Path(desktops[0].nombre).name if desktops else ""
    carpeta = registro.slug(Path(nombre_desktop).stem if nombre_desktop
                            else comprimidos.nombre_sin_extension(archivo.name))
    return carpeta, nombre_desktop


def instalar_programa(archivo: Path, entradas: list[comprimidos.Entrada], reemplazar: bool = False,
                      contrasena: str | None = None, progreso=None, cancelar=None) -> dict:
    archivo = Path(archivo)
    carpeta, nombre_desktop = plan_programa(entradas, archivo)
    id_ = "programa-" + carpeta
    destino = rutas.opt() / carpeta
    previo = registro.buscar(id_)
    if os.path.lexists(destino) or previo:
        if not reemplazar:
            raise Conflicto(destino, destino)
        if previo:
            registro.deshacer(id_)
        if os.path.lexists(destino):   # una carpeta que no es nuestra: no se toca, va al lado
            destino = comprimidos.nombre_libre(destino)
    rutas.opt().mkdir(parents=True, exist_ok=True)
    comprimidos.extraer(archivo, destino, entradas, contrasena,
                        (lambda f: progreso(0.9 * f)) if progreso else None, cancelar)
    creados: list[Path] = [destino]
    try:
        desktop_origen = None
        if nombre_desktop:
            desktop_origen = next((p for p in sorted(destino.rglob(nombre_desktop), key=lambda p: len(p.parts))
                                   if _dentro_de(p, destino)), None)
        grupos = leer_desktop(desktop_origen.read_text(encoding="utf-8", errors="replace")) if desktop_origen else []
        programa = buscar_ejecutable(destino, grupos, nombre_desktop)
        if programa is None:
            raise ErrorInstalar(tr("programa_sin_ejecutable"))
        nombre = valor(grupos, "Name") or carpeta
        icono_origen = _buscar_icono(destino, valor(grupos, "Icon"))
        icono = valor(grupos, "Icon") if valor(grupos, "Icon") and "/" not in valor(grupos, "Icon") else ""
        if icono_origen is not None:
            icono, archivos = instalar_icono(icono_origen, registro.slug(Path(nombre_desktop).stem or carpeta), id_)
            creados += archivos
        if not grupos:
            grupos = [("Desktop Entry", [["Name", nombre], ["Categories", "Utility;"]])]
        _preparar_desktop(grupos, nombre, str(programa), icono or "application-x-executable", id_)
        desktop = _desktop_libre(nombre_desktop or carpeta + ".desktop", id_)
        desktop.parent.mkdir(parents=True, exist_ok=True)
        desktop.write_text(escribir_desktop(grupos), encoding="utf-8")
        creados.append(desktop)
        script = destino / "desinstalar.sh"
        if script.exists():   # el que trae el comprimido es para su propio instalador: se guarda aparte
            guardado = destino / "desinstalar.sh.original"
            script.rename(comprimidos.nombre_libre(guardado) if os.path.lexists(guardado) else guardado)
        escribir_desinstalador(script, nombre, [creados[0], *creados[1:]])
    except BaseException:
        from . import papelera
        for p in reversed(creados):
            papelera.mandar(p)
        raise
    registro.refrescar_lanzador()
    entrada = registro.agregar({"id": id_, "tipo": "programa", "nombre": nombre, "rutas": [str(p) for p in creados],
                                "icono": str(next((p for p in creados if p.suffix in (".png", ".svg")), "")),
                                "archivo": archivo.name, "abrir": str(programa), "desinstalador": str(script)})
    if progreso:
        progreso(1.0)
    return entrada


# ------------------------------------------------------------------------------------ correr cosas

def lanzar(args: list[str], env: dict | None = None, carpeta: Path | None = None) -> None:
    """Abre un programa aparte (sigue vivo aunque se cierre esta ventana)."""
    try:
        subprocess.Popen(args, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                         env=env, cwd=str(carpeta or rutas.casa()), start_new_session=True)
    except OSError as e:
        raise ErrorInstalar(tr("no_se_pudo_abrir")) from e


def abrir_carpeta(ruta: Path) -> None:
    ruta = Path(ruta)
    carpeta = ruta if ruta.is_dir() else ruta.parent
    for exe in ("xdg-open", "gio"):
        if shutil.which(exe):
            lanzar([exe, str(carpeta)] if exe == "xdg-open" else [exe, "open", str(carpeta)])
            return
    raise ErrorInstalar(tr("no_se_pudo_abrir"))


_ADMIN = {"sudo", "pkexec", "doas", "su", "run0"}


def vista_script(ruta: Path, cabeza: bytes) -> dict:
    """Lo que se muestra de un instalador .sh/.run/.bin antes de correrlo: sus primeras líneas si es texto."""
    elf = cabeza.startswith(b"\x7fELF")
    lineas = []
    if not elf:
        for linea in cabeza[:16384].split(b"\n")[:40]:
            if b"\0" in linea or sum(1 for c in linea if c < 9 or 13 < c < 32) > 2:
                break   # empieza lo binario (un .run trae el programa pegado después del texto)
            lineas.append(linea.decode("utf-8", "replace").rstrip("\r"))
    texto = "\n".join(lineas)
    pide_admin = bool(re.search(r"\b(sudo|pkexec|doas)\b|EUID|id -u|as root|como root", cabeza[:65536].decode("utf-8", "replace")))
    return {"vista": texto, "binario": elf, "pide_admin": pide_admin,
            "comando_admin": "sudo " + " ".join(shlex.quote(p) for p in comando_script(ruta, cabeza, sin_admin=False))}


def comando_script(ruta: Path, cabeza: bytes | None = None, sin_admin: bool = True) -> list[str]:
    """Cómo se corre el instalador: con el intérprete de su primera línea (#!), con sh, o directo si es binario."""
    ruta = Path(ruta)
    if cabeza is None:
        try:
            with open(ruta, "rb") as f:
                cabeza = f.read(512)
        except OSError:
            cabeza = b""
    if cabeza.startswith(b"\x7fELF"):
        return [str(ruta)]
    if cabeza.startswith(b"#!"):
        primera = cabeza[2:].split(b"\n", 1)[0].decode("utf-8", "replace").strip()
        partes = _partes_exec(primera)
        if partes:
            if sin_admin and any(os.path.basename(p) in _ADMIN for p in partes):
                raise ErrorInstalar(tr("script_con_sudo"))
            return [*partes, str(ruta)]
    return ["sh", str(ruta)]


def terminal(args: list[str], titulo: str, carpeta: Path) -> None:
    """Abre `args` en una terminal que se queda abierta al terminar (para leer lo que pasó)."""
    opciones = (("kitty", ["kitty", "--hold", "--title", titulo, "--directory", str(carpeta), "--", *args]),
                ("foot", ["foot", "--hold", "--title", titulo, "--working-directory", str(carpeta), *args]),
                ("alacritty", ["alacritty", "--hold", "--title", titulo, "--working-directory", str(carpeta), "-e", *args]),
                ("konsole", ["konsole", "--hold", "--workdir", str(carpeta), "-e", *args]))
    for exe, cmd in opciones:
        if shutil.which(exe):
            lanzar(cmd, carpeta=carpeta)
            return
    raise ErrorInstalar(tr("falta_terminal"))


def correr_script(ruta: Path) -> None:
    ruta = Path(ruta)
    args = comando_script(ruta)
    if args == [str(ruta)]:   # un binario: necesita permiso de ejecución
        ruta.chmod(ruta.stat().st_mode | stat.S_IXUSR)
    terminal(args, ruta.name, ruta.parent)


def opciones_windows() -> list[dict]:
    """Con qué se puede abrir un programa de Windows en este equipo: Wine, umu (Proton) o Proton de Steam."""
    salida = []
    if shutil.which("wine"):
        salida.append({"id": "wine", "nombre": "Wine"})
    if shutil.which("umu-run"):
        salida.append({"id": "umu", "nombre": "Proton (umu)"})
    proton = _proton()
    if proton:
        salida.append({"id": "proton", "nombre": proton.parent.name, "ruta": str(proton)})
    return salida


def _proton() -> Path | None:
    casa = rutas.casa()
    raices = [casa / ".local/share/Steam", casa / ".steam/steam",
              casa / ".var/app/com.valvesoftware.Steam/.local/share/Steam"]
    encontrados = []
    for r in raices:
        for patron in ("steamapps/common/Proton*/proton", "compatibilitytools.d/*/proton"):
            encontrados += [p for p in r.glob(patron) if p.is_file()]
    return sorted(encontrados, key=lambda p: p.parent.name)[-1] if encontrados else None


def abrir_windows(ruta: Path, opcion: str) -> None:
    ruta = Path(ruta)
    msi = ruta.suffix.lower() == ".msi"
    env = dict(os.environ)
    if opcion == "wine":
        args = ["wine", "msiexec", "/i", str(ruta)] if msi else ["wine", str(ruta)]
    elif opcion == "umu":
        env.setdefault("WINEPREFIX", str(rutas.propios() / "windows"))
        args = ["umu-run", "msiexec", "/i", str(ruta)] if msi else ["umu-run", str(ruta)]
    elif opcion == "proton":
        proton = _proton()
        if proton is None:
            raise ErrorInstalar(tr("falta_wine"))
        prefijo = rutas.propios() / "proton"
        prefijo.mkdir(parents=True, exist_ok=True)
        steam = proton.parents[3] if proton.parent.parent.name == "common" else proton.parents[2]
        env.update(STEAM_COMPAT_DATA_PATH=str(prefijo), STEAM_COMPAT_CLIENT_INSTALL_PATH=str(steam))
        args = [str(proton), "run", *(["msiexec", "/i"] if msi else []), str(ruta)]
    else:
        raise ErrorInstalar(tr("falta_wine"))
    lanzar(args, env=env, carpeta=ruta.parent)


def abrir_java(ruta: Path) -> None:
    if not shutil.which("java"):
        raise ErrorInstalar(tr("falta_java"))
    lanzar(["java", "-jar", str(ruta)], carpeta=Path(ruta).parent)
