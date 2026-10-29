"""Dónde guarda cada cosa Vortex Player (todas se pueden cambiar con variables, para las pruebas)."""
from __future__ import annotations

import os
import subprocess
from pathlib import Path


def _xdg(var: str, defecto: str) -> Path:
    return Path(os.environ.get(var) or Path.home() / defecto)


def datos() -> Path:
    """La biblioteca (biblioteca.db)."""
    return Path(os.environ.get("VORTEX_PLAYER_DATOS") or _xdg("XDG_DATA_HOME", ".local/share") / "vortex-player")


def config() -> Path:
    """ajustes.json."""
    return Path(os.environ.get("VORTEX_PLAYER_CONFIG") or _xdg("XDG_CONFIG_HOME", ".config") / "vortex-player")


def cache() -> Path:
    """Portadas sacadas de los archivos."""
    return Path(os.environ.get("VORTEX_PLAYER_CACHE") or _xdg("XDG_CACHE_HOME", ".cache") / "vortex-player")


def musica() -> Path:
    """La carpeta Música de la persona (xdg-user-dir MUSIC)."""
    if os.environ.get("VORTEX_PLAYER_MUSICA"):
        return Path(os.environ["VORTEX_PLAYER_MUSICA"])
    try:
        r = subprocess.run(["xdg-user-dir", "MUSIC"], capture_output=True, text=True, timeout=3)
        p = r.stdout.strip()
        if r.returncode == 0 and p and p != str(Path.home()):
            return Path(p)
    except (OSError, subprocess.TimeoutExpired):
        pass
    return Path.home() / "Música"


def descargas() -> Path:
    """Donde quedan las canciones bajadas: Música/Vortex Player."""
    return musica() / "Vortex Player"
