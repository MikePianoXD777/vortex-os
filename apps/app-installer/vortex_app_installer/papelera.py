"""Mandar a la papelera (la de freedesktop, la misma que usa el explorador de archivos).

Deshacer nunca borra para siempre: lo que se desinstala o se quita va a la papelera y se puede
recuperar. Dentro de la carpeta personal se hace aquí mismo (Trash/files + Trash/info); fuera de
ella (otro disco) se le pide a «gio trash», que sabe usar la papelera de cada disco.
"""
from __future__ import annotations

import os
import shutil
import subprocess
import urllib.parse
from datetime import datetime
from pathlib import Path

from . import rutas


def _dentro(ruta: Path, carpeta: Path) -> bool:
    try:
        Path(os.path.abspath(ruta)).relative_to(os.path.abspath(carpeta))
        return True
    except ValueError:
        return False


def mandar(ruta: Path | str) -> bool:
    """Manda `ruta` (archivo, carpeta o enlace) a la papelera. False si no existía."""
    ruta = Path(os.path.abspath(ruta))
    if not ruta.exists() and not ruta.is_symlink():
        return False
    if not _dentro(ruta, rutas.casa()) and shutil.which("gio"):
        r = subprocess.run(["gio", "trash", "--", str(ruta)], stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace", check=False)
        if r.returncode == 0:
            return True
    base = rutas.papelera()
    archivos, info = base / "files", base / "info"
    archivos.mkdir(parents=True, exist_ok=True)
    info.mkdir(parents=True, exist_ok=True)
    nombre, n = ruta.name, 1
    while True:
        try:   # el .trashinfo se reserva primero (O_EXCL): así dos borrados no eligen el mismo nombre
            fd = os.open(info / (nombre + ".trashinfo"), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except FileExistsError:
            fd = None
        if fd is not None and not (archivos / nombre).exists() and not (archivos / nombre).is_symlink():
            break
        if fd is not None:
            os.close(fd)
            os.unlink(info / (nombre + ".trashinfo"))
        n += 1
        nombre = "%s.%d%s" % (ruta.stem, n, ruta.suffix) if ruta.is_file() else "%s.%d" % (ruta.name, n)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write("[Trash Info]\nPath=%s\nDeletionDate=%s\n"
                % (urllib.parse.quote(str(ruta)), datetime.now().strftime("%Y-%m-%dT%H:%M:%S")))
    try:
        shutil.move(str(ruta), str(archivos / nombre))
    except OSError:
        os.unlink(info / (nombre + ".trashinfo"))
        raise
    return True
