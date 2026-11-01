"""Lo que cambia entre Linux y Windows (2.5: Vortex Player para Windows).

En Windows, Vortex Player viene empaquetado con PyInstaller y trae sus programas en la carpeta `bin`
junto al .exe (mpv, ffmpeg, ffprobe, yt-dlp y deno, el JavaScript que yt-dlp pide para YouTube). Esa
carpeta va primero en el PATH, así que los `subprocess` y `shutil.which` de siempre los encuentran.
"""
from __future__ import annotations

import getpass
import os
import sys
from pathlib import Path

WINDOWS = sys.platform == "win32"
EMPAQUETADO = bool(getattr(sys, "frozen", False))


def carpeta_bin() -> Path | None:
    """La carpeta `bin` del paquete de Windows (o None fuera de él)."""
    if not EMPAQUETADO:
        return None
    b = Path(sys.executable).resolve().parent / "bin"
    return b if b.is_dir() else None


def preparar() -> None:
    """Al arrancar: los programas del paquete, primero en el PATH."""
    b = carpeta_bin()
    if b is not None:
        os.environ["PATH"] = str(b) + os.pathsep + os.environ.get("PATH", "")


def usuario() -> str:
    """Para nombrar el socket de «una sola ventana» (os.getuid no existe en Windows)."""
    if hasattr(os, "getuid"):
        return str(os.getuid())
    try:
        return getpass.getuser()
    except Exception:
        return "usuario"


def salida_audio() -> str:
    """La salida de mpv: WASAPI en Windows; PipeWire (y sus respaldos) en Linux."""
    return os.environ.get("VORTEX_PLAYER_AO") or ("wasapi" if WINDOWS else "pipewire,pulse,alsa")


def canal_mpv(nombre: str, carpeta: str) -> tuple[str, str]:
    """(lo que se le pasa a mpv en --input-ipc-server, lo que abre QLocalSocket).

    En Linux es un socket de Unix (una ruta); en Windows, una tubería con nombre (\\\\.\\pipe\\nombre),
    que QLocalSocket abre por su nombre solo."""
    if WINDOWS:
        return "\\\\.\\pipe\\" + nombre, nombre
    ruta = str(Path(carpeta) / f"{nombre}.sock")
    return ruta, ruta


def abrir(destino: str) -> None:
    """Abre una carpeta o un enlace con lo que tengas (Explorador / navegador)."""
    if WINDOWS:
        os.startfile(destino)   # noqa: S606 (solo existe en Windows)
        return
    import subprocess
    subprocess.Popen(["xdg-open", destino], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


def a_la_papelera(ruta: str) -> None:
    if WINDOWS:
        from PySide6.QtCore import QFile
        QFile.moveToTrash(ruta)
        return
    import subprocess
    subprocess.run(["gio", "trash", ruta], capture_output=True)
