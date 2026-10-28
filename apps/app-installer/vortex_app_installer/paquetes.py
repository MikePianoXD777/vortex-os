"""Paquetes de otras distros (.deb de Debian/Ubuntu, .rpm de Fedora) y programas de Windows.

No se instalan: en Arch romperían el sistema. Aquí solo se lee su nombre y su descripción para
buscar el mismo programa en Flathub.
"""
from __future__ import annotations

import re
import shutil
import struct
import subprocess
from pathlib import Path

from .comprimidos import _entorno


def _bsdtar_salida(args: list[str], entrada: bytes | None = None) -> bytes:
    exe = shutil.which("bsdtar")
    if not exe:
        return b""
    try:
        r = subprocess.run([exe, *args], input=entrada, capture_output=True, env=_entorno(), timeout=60,
                           start_new_session=True, check=False, **({} if entrada is not None else
                                                                   {"stdin": subprocess.DEVNULL}))
    except (OSError, subprocess.TimeoutExpired):
        return b""
    return r.stdout if r.returncode == 0 else b""


def leer_deb(ruta: Path) -> dict:
    """{nombre, version, descripcion} del archivo control de un .deb (vacío si no se pudo)."""
    control_tar = _bsdtar_salida(["-xOf", str(ruta), "control.tar*"])
    texto = b""
    for nombre in ("./control", "control"):
        texto = _bsdtar_salida(["-xOf", "-", nombre], control_tar) if control_tar else b""
        if texto:
            break
    campos, clave = {}, None
    for linea in texto.decode("utf-8", "replace").splitlines():
        if linea[:1] in (" ", "\t") and clave:
            continue
        if ":" in linea:
            clave, valor = linea.split(":", 1)
            campos[clave.strip().lower()] = valor.strip()
    return {"nombre": campos.get("package", ""), "version": campos.get("version", ""),
            "descripcion": campos.get("description", "")}


# Etiquetas del encabezado de un RPM
_NOMBRE, _VERSION, _RESUMEN = 1000, 1001, 1004


def _encabezado(datos: bytes, pos: int) -> tuple[dict[int, str], int]:
    if datos[pos:pos + 3] != b"\x8e\xad\xe8":
        raise ValueError("no es un encabezado RPM")
    n, tam = struct.unpack(">II", datos[pos + 8:pos + 16])
    indice = pos + 16
    almacen = indice + n * 16
    etiquetas = {}
    for i in range(n):
        etiqueta, tipo, desplazamiento, _ = struct.unpack(">IIII", datos[indice + i * 16:indice + i * 16 + 16])
        if tipo in (6, 8, 9) and etiqueta in (_NOMBRE, _VERSION, _RESUMEN):
            inicio = almacen + desplazamiento
            fin = datos.index(b"\0", inicio)
            etiquetas[etiqueta] = datos[inicio:fin].decode("utf-8", "replace")
    return etiquetas, almacen + tam


def leer_rpm(ruta: Path) -> dict:
    try:
        with open(ruta, "rb") as f:
            datos = f.read(1 << 20)
        _, fin = _encabezado(datos, 96)               # la firma, justo después de los 96 bytes iniciales
        fin += (8 - fin % 8) % 8                       # alineada a 8
        etiquetas, _ = _encabezado(datos, fin)
    except (OSError, ValueError, struct.error):
        etiquetas = {}
    if not etiquetas.get(_NOMBRE):   # respaldo: «nombre-1.2-3.fc40.x86_64.rpm»
        m = re.match(r"(.+?)-\d[^-]*-[^-]+\.[^.]+\.rpm$", ruta.name)
        etiquetas[_NOMBRE] = m.group(1) if m else ruta.stem
    return {"nombre": etiquetas.get(_NOMBRE, ""), "version": etiquetas.get(_VERSION, ""),
            "descripcion": etiquetas.get(_RESUMEN, "")}


def leer(ruta: Path, tipo: str) -> dict:
    datos = leer_deb(ruta) if tipo == "deb" else leer_rpm(ruta)
    if not datos.get("nombre"):
        datos["nombre"] = re.split(r"[_]", ruta.stem)[0]
    datos["nombre_programa"] = datos["nombre"]
    return datos


def nombre_de_exe(nombre_archivo: str) -> str:
    """«DiscordSetup-1.2.3-x64.exe» → «Discord» (para buscar una versión de Linux en Flathub)."""
    n = re.sub(r"\.(exe|msi)$", "", nombre_archivo, flags=re.I)
    n = re.sub(r"[-_ .]?(v?\d[\d.]*\w*|x64|x86|amd64|win(32|64)?|windows|installer|setup|install|full|"
               r"portable|release|latest)\b", " ", n, flags=re.I)
    n = re.sub(r"(Setup|Installer|Install)$", "", n.strip(" -_."))
    n = re.sub(r"(?<=[a-z])(?=[A-Z])", " ", n)
    return " ".join(n.replace("_", " ").replace("-", " ").split()) or nombre_archivo
