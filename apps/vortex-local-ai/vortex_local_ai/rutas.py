"""Dónde guarda sus cosas Vortexine's Local AI.

- DATOS: los chats y la configuración (~/.local/share/vortex-local-ai).
- CACHE: el índice de la memoria del vault, que se puede rehacer (~/.cache/vortex-local-ai).
- VAULT_POR_DEFECTO: el vault que se crea si no abres otro (~/Documentos/Vortexine Local AI).

Respetan XDG_DATA_HOME y XDG_CACHE_HOME. Con VORTEX_LOCAL_AI_HOME (para las pruebas) todo cuelga de
esa carpeta: <home>/datos, <home>/cache y <home>/vault, y no se toca nada de tu usuario.

Las rutas se calculan al importar el módulo; si cambias las variables después, llama a recargar().
"""
from __future__ import annotations

import os
from pathlib import Path

APP = "vortex-local-ai"


def _carpeta_documentos() -> Path:
    """La carpeta de documentos del usuario (XDG_DOCUMENTS_DIR; si no, ~/Documentos o ~/Documents)."""
    casa = Path.home()
    xdg = os.environ.get("XDG_DOCUMENTS_DIR")
    if xdg:
        return Path(xdg.replace("$HOME", str(casa))).expanduser()
    archivo = Path(os.environ.get("XDG_CONFIG_HOME") or casa / ".config") / "user-dirs.dirs"
    try:
        for linea in archivo.read_text(encoding="utf-8").splitlines():
            if linea.startswith("XDG_DOCUMENTS_DIR="):
                valor = linea.split("=", 1)[1].strip().strip('"').replace("$HOME", str(casa))
                if valor:
                    return Path(valor)
    except OSError:
        pass
    for nombre in ("Documentos", "Documents"):
        if (casa / nombre).is_dir():
            return casa / nombre
    return casa / "Documentos"


def calcular() -> tuple[Path, Path, Path]:
    """(DATOS, CACHE, VAULT_POR_DEFECTO) según las variables de entorno de ahora."""
    propia = os.environ.get("VORTEX_LOCAL_AI_HOME")
    if propia:
        base = Path(propia).expanduser()
        return base / "datos", base / "cache", base / "vault"
    casa = Path.home()
    datos = Path(os.environ.get("XDG_DATA_HOME") or casa / ".local" / "share") / APP
    cache = Path(os.environ.get("XDG_CACHE_HOME") or casa / ".cache") / APP
    return datos, cache, _carpeta_documentos() / "Vortexine Local AI"


DATOS, CACHE, VAULT_POR_DEFECTO = calcular()


def recargar() -> None:
    """Vuelve a calcular DATOS, CACHE y VAULT_POR_DEFECTO (por si cambió VORTEX_LOCAL_AI_HOME)."""
    global DATOS, CACHE, VAULT_POR_DEFECTO
    DATOS, CACHE, VAULT_POR_DEFECTO = calcular()


def chats() -> Path:
    """Carpeta de los chats guardados (DATOS/chats)."""
    return DATOS / "chats"


def config() -> Path:
    """Archivo de configuración de la app (DATOS/config.json). Lo usa la interfaz."""
    return DATOS / "config.json"
