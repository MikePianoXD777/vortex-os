"""Qué es un archivo: por sus primeros bytes (la «firma») y, si no alcanza, por su extensión.

`detectar(ruta)` devuelve un `Info` con el tipo («comprimido», «programa», «appimage», «script»,
«deb», «rpm», «flatpakref», «flatpak», «windows», «mod», «jar» o «desconocido»), su tamaño, de dónde
vino (si el navegador lo anotó) y lo que hace falta para mostrarlo. No ejecuta nada del archivo.
"""
from __future__ import annotations

import os
import stat
import urllib.parse
from dataclasses import dataclass, field
from pathlib import Path

from . import comprimidos, rutas
from .comprimidos import Entrada

# Tipos que, al usarlos, corren código del archivo (se avisa si además vienen de internet).
EJECUTAN_CODIGO = {"appimage", "programa", "script", "windows", "jar", "mod"}


@dataclass
class Info:
    ruta: str
    nombre: str
    tamano: int
    tipo: str
    formato: str = ""             # «zip», «tar.gz», «iso»… o «elf», «texto»…
    origen: str = ""              # la página de donde se bajó, si se sabe
    de_internet: bool = False
    entradas: list[Entrada] = field(default_factory=list)
    error: str = ""               # por qué no se pudo leer el contenido (dañado, falta un programa…)
    falta_comando: str = ""       # el comando para instalar lo que falta («sudo pacman -S 7zip»)
    extra: dict = field(default_factory=dict)

    @property
    def ejecuta_codigo(self) -> bool:
        return self.tipo in EJECUTAN_CODIGO


def _cabeza(ruta: Path, n: int = 0x8800) -> bytes:
    try:
        with open(ruta, "rb") as f:
            return f.read(n)
    except OSError:
        return b""


def origen(ruta: Path) -> str:
    """La dirección de donde se bajó (Chrome, Chromium y otros la guardan en un atributo del archivo)."""
    for clave in ("user.xdg.origin.url", "user.xdg.referrer.url"):
        try:
            valor = os.getxattr(ruta, clave).decode("utf-8", "replace").strip()
        except (OSError, AttributeError):
            continue
        if valor:
            return valor
    return ""


def sitio(url: str) -> str:
    """«https://github.com/x/y/releases/…» → «github.com»."""
    try:
        host = urllib.parse.urlsplit(url).hostname or ""
    except ValueError:
        return ""
    return host[4:] if host.startswith("www.") else host


def _en_descargas(ruta: Path) -> bool:
    try:
        Path(os.path.abspath(ruta)).relative_to(os.path.abspath(rutas.descargas()))
        return True
    except ValueError:
        return False


def es_texto(cabeza: bytes) -> bool:
    return b"\0" not in cabeza[:4096]


def formato_comprimido(nombre: str, cabeza: bytes) -> str:
    """«zip», «tar.gz», «7z»… o "" si no parece un comprimido."""
    bajo = nombre.lower()
    for ext in comprimidos.EXT_TAR + comprimidos.EXT_OTROS:
        if bajo.endswith(ext):
            return ext[1:]
    firmas = ((b"PK\x03\x04", "zip"), (b"PK\x05\x06", "zip"), (b"Rar!\x1a\x07", "rar"),
              (b"7z\xbc\xaf\x27\x1c", "7z"))
    for firma, fmt in firmas:
        if cabeza.startswith(firma):
            return fmt
    if len(cabeza) > 0x8006 and cabeza[0x8001:0x8006] == b"CD001":
        return "iso"
    if len(cabeza) > 262 and cabeza[257:262] == b"ustar":
        return "tar"
    for ext, fmt in comprimidos.EXT_SUELTOS.items():
        if bajo.endswith(ext):
            return ext[1:]
    sueltos = ((b"\x1f\x8b", "gz"), (b"\xfd7zXZ\x00", "xz"), (b"\x28\xb5\x2f\xfd", "zst"), (b"BZh", "bz2"))
    for firma, fmt in sueltos:
        if cabeza.startswith(firma):
            return fmt
    return ""


def _tipo(ruta: Path, cabeza: bytes) -> tuple[str, str]:
    bajo = ruta.name.lower()
    elf = cabeza.startswith(b"\x7fELF")
    if bajo.endswith(".appimage") or (elf and cabeza[8:11] in (b"AI\x01", b"AI\x02")):
        return "appimage", "appimage"
    if bajo.endswith(".flatpakref"):
        return "flatpakref", "flatpakref"
    if bajo.endswith(".flatpak"):
        return "flatpak", "flatpak"
    if bajo.endswith(".deb") or cabeza.startswith(b"!<arch>\ndebian"):
        return "deb", "deb"
    if bajo.endswith(".rpm") or cabeza.startswith(b"\xed\xab\xee\xdb"):
        return "rpm", "rpm"
    if bajo.endswith((".exe", ".msi")) or cabeza.startswith(b"MZ") \
            or cabeza.startswith(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1") and bajo.endswith(".msi"):
        return "windows", "msi" if bajo.endswith(".msi") else "exe"
    if bajo.endswith(".jar"):
        return "jar", "jar"
    if bajo.endswith((".sh", ".run", ".bin")) or cabeza.startswith(b"#!") or elf:
        return "script", "elf" if elf else "texto"
    fmt = formato_comprimido(ruta.name, cabeza)
    if fmt:
        return "comprimido", fmt
    return "desconocido", ""


def _parece_programa(entradas: list[Entrada]) -> bool:
    """Un comprimido de programa para Linux: trae un .desktop y algún ejecutable (no muy hondo)."""
    desktop = any(e.tipo == "archivo" and e.nombre.endswith(".desktop") and e.profundidad <= 3 for e in entradas)
    exe = any(e.ejecutable and e.profundidad <= 3 and ".so" not in Path(e.nombre).name
              and not e.nombre.endswith(".desktop") for e in entradas)
    return desktop and exe


def detectar(ruta: Path | str, contrasena: str | None = None) -> Info:
    ruta = Path(ruta)
    try:
        st = ruta.stat()
    except OSError:
        return Info(str(ruta), ruta.name, 0, "falta")
    if stat.S_ISDIR(st.st_mode):
        return Info(str(ruta), ruta.name, 0, "carpeta")
    cabeza = _cabeza(ruta)
    tipo, fmt = _tipo(ruta, cabeza)
    url = origen(ruta)
    info = Info(str(ruta), ruta.name, st.st_size, tipo, fmt, url, bool(url) or _en_descargas(ruta))
    info.extra["ejecutable"] = bool(st.st_mode & 0o111)

    if tipo == "comprimido":
        try:
            info.entradas = comprimidos.listar(ruta, contrasena)
        except comprimidos.NecesitaContrasena:
            info.extra["pide_contrasena"] = True
        except comprimidos.FaltaPrograma as e:
            info.error, info.falta_comando = str(e), e.comando
        except comprimidos.ErrorComprimido as e:
            info.error = str(e)
        if info.entradas and _parece_programa(info.entradas):
            info.tipo = "programa"
    elif tipo == "script":
        from . import instalar
        info.extra.update(instalar.vista_script(ruta, cabeza))
    elif tipo in ("deb", "rpm"):
        from . import paquetes
        info.extra.update(paquetes.leer(ruta, tipo))
    elif tipo == "flatpakref":
        from . import flatpak
        info.extra.update(flatpak.leer_ref(ruta))
    elif tipo == "jar":
        from . import mods
        mod = mods.leer_mod(ruta)
        if mod:
            info.tipo = "mod"
            info.extra["mod"] = mod
    elif tipo == "windows":
        from . import paquetes
        info.extra["nombre_programa"] = paquetes.nombre_de_exe(ruta.name)
    return info
