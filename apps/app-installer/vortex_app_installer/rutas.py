"""Dónde deja las cosas la app. Todo se calcula al usarlo desde HOME y XDG_DATA_HOME (las pruebas
cambian HOME por una carpeta temporal, así que nada se guarda en el equipo de verdad)."""
from __future__ import annotations

import os
import re
from pathlib import Path


def casa() -> Path:
    return Path(os.environ.get("HOME") or os.path.expanduser("~"))


def datos_xdg() -> Path:
    v = os.environ.get("XDG_DATA_HOME")
    return Path(v) if v and os.path.isabs(v) else casa() / ".local" / "share"


def config_xdg() -> Path:
    v = os.environ.get("XDG_CONFIG_HOME")
    return Path(v) if v and os.path.isabs(v) else casa() / ".config"


def appimages() -> Path:
    """Los AppImages instalados: ~/Aplicaciones si ya la usas (Vortexine's en español), si no
    ~/Applications, la de siempre para AppImages (AppImageLauncher, Gear Lever)."""
    espanol = casa() / "Aplicaciones"
    return espanol if espanol.is_dir() else casa() / "Applications"


def opt() -> Path:
    """Los programas que vienen en un .tar.gz, cada uno en su carpeta."""
    return casa() / ".local" / "opt"


def lanzadores() -> Path:
    return datos_xdg() / "applications"


def iconos() -> Path:
    return datos_xdg() / "icons" / "hicolor"


def propios() -> Path:
    return datos_xdg() / "vortex-app-installer"


def registro() -> Path:
    return propios() / "instalado.json"


def desinstaladores() -> Path:
    return propios() / "desinstaladores"


def papelera() -> Path:
    return datos_xdg() / "Trash"


def _carpeta_usuario(clave: str, respaldo: str) -> Path:
    """Una carpeta de user-dirs.dirs (XDG_DOWNLOAD_DIR…), con $HOME ya reemplazado."""
    try:
        texto = (config_xdg() / "user-dirs.dirs").read_text(encoding="utf-8")
    except OSError:
        texto = ""
    m = re.search(r'^%s="([^"]*)"' % re.escape(clave), texto, re.M)
    if m:
        valor = m.group(1).replace("$HOME", str(casa()))
        if os.path.isabs(valor):
            return Path(valor)
    return casa() / respaldo


def descargas() -> Path:
    return _carpeta_usuario("XDG_DOWNLOAD_DIR", "Descargas")


def protegidas() -> set[Path]:
    """Carpetas que deshacer nunca manda a la papelera, aunque el registro las nombre."""
    c = casa()
    fijas = {Path("/"), c, c / ".local", c / ".local" / "share", c / ".config", datos_xdg(), config_xdg(),
             appimages(), opt(), lanzadores(), iconos(), propios(), desinstaladores(), papelera(),
             c / "Descargas", c / "Downloads", c / "Documentos", c / "Documents", c / "Escritorio",
             c / "Desktop", c / "Imágenes", c / "Pictures", c / "Música", c / "Music", c / "Vídeos",
             c / "Videos", c / ".minecraft"}
    for clave, respaldo in (("XDG_DOWNLOAD_DIR", "Descargas"), ("XDG_DOCUMENTS_DIR", "Documentos"),
                            ("XDG_DESKTOP_DIR", "Escritorio"), ("XDG_PICTURES_DIR", "Imágenes"),
                            ("XDG_MUSIC_DIR", "Música"), ("XDG_VIDEOS_DIR", "Vídeos")):
        fijas.add(_carpeta_usuario(clave, respaldo))
    return {Path(os.path.abspath(p)) for p in fijas}
