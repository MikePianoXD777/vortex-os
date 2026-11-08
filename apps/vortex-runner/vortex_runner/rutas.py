"""Dónde guarda sus cosas Vortexine's Runner.

- DATOS (~/.local/share/vortex-runner-datos): la biblioteca (biblioteca.json), los íconos sacados de los
  .exe y los prefijos de Wine (uno por programa, en prefijos/). El código de la app va aparte, en
  ~/.local/share/vortex-runner: quitar la app no borra tus programas ni sus partidas guardadas.
- LANZADORES (~/.local/share/applications): los accesos que crea «Agregar al lanzador».

Con VORTEX_RUNNER_HOME (para las pruebas) todo cuelga de esa carpeta y no se toca nada de tu usuario.
Las rutas se calculan al importar; si cambias las variables después, llama a recargar().
"""
from __future__ import annotations

import os
from pathlib import Path


def calcular() -> tuple[Path, Path]:
    propia = os.environ.get("VORTEX_RUNNER_HOME")
    if propia:
        base = Path(propia).expanduser()
        return base / "datos", base / "applications"
    share = Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")
    return share / "vortex-runner-datos", share / "applications"


DATOS, LANZADORES = calcular()


def recargar() -> None:
    global DATOS, LANZADORES
    DATOS, LANZADORES = calcular()


def biblioteca() -> Path:
    return DATOS / "biblioteca.json"


def prefijos() -> Path:
    return DATOS / "prefijos"


def iconos() -> Path:
    return DATOS / "iconos"


def registros() -> Path:
    return DATOS / "registros"
