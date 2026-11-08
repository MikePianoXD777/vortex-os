"""Los Proton que hay en tu PC, y cuál usar.

Busca en las carpetas de herramientas de Steam (la del sistema, donde pacman pone el de CachyOS, y las de
tu usuario) y en los Proton oficiales de Steam (steamapps/common/Proton*). Además ofrece «GE-Proton»,
que umu-run baja solo la primera vez (unos 500 MB) y luego mantiene al día.

El de serie es el Proton de CachyOS en su versión para el runtime de Steam (proton-cachyos-slr), que es
la que va con umu-run; si no está, el nativo, y si tampoco, el más nuevo que encuentre.

Para pruebas: VORTEX_RUNNER_PROTONES=carpeta1:carpeta2 cambia dónde se busca.
"""
from __future__ import annotations

import os
import re
from pathlib import Path

GE = "GE-Proton"
PREFERIDOS = ("proton-cachyos-slr", "proton-cachyos-native", "proton-cachyos")
NOMBRES = {
    "proton-cachyos-slr": "Proton CachyOS",
    "proton-cachyos-native": "Proton CachyOS (nativo)",
    "proton-cachyos": "Proton CachyOS",
}


def _carpetas() -> list[Path]:
    propias = os.environ.get("VORTEX_RUNNER_PROTONES")
    if propias is not None:
        return [Path(p) for p in propias.split(":") if p]
    casa = Path.home()
    steam = [casa / ".local/share/Steam", casa / ".steam/root", casa / ".steam/steam",
             casa / ".var/app/com.valvesoftware.Steam/data/Steam"]
    salida = [Path("/usr/share/steam/compatibilitytools.d")]
    salida += [s / "compatibilitytools.d" for s in steam]
    salida += [s / "steamapps/common" for s in steam]
    return salida


def _es_proton(carpeta: Path) -> bool:
    return (carpeta / "proton").is_file()


def _version(carpeta: Path) -> str:
    try:
        return (carpeta / "version").read_text(encoding="utf-8", errors="replace").split()[-1][:40]
    except (OSError, IndexError):
        return ""


def _orden_numerico(nombre: str) -> tuple:
    return tuple(int(n) for n in re.findall(r"\d+", nombre)[:4])


def lista() -> list[dict]:
    """[{id, nombre, ruta, version}] sin repetir, primero los preferidos; GE-Proton al final."""
    vistos: dict[str, dict] = {}
    for base in _carpetas():
        try:
            hijos = sorted(base.iterdir())
        except OSError:
            continue
        for c in hijos:
            if not c.is_dir() or not _es_proton(c):
                continue
            if base.name == "common" and not c.name.lower().startswith("proton"):
                continue
            real = str(c.resolve())
            if any(v["ruta"] == real for v in vistos.values()) or c.name in vistos:
                continue
            vistos[c.name] = {"id": c.name, "nombre": NOMBRES.get(c.name, c.name),
                              "ruta": real, "version": _version(c)}
    pref = [vistos.pop(p) for p in PREFERIDOS if p in vistos]
    otros = sorted(vistos.values(), key=lambda v: _orden_numerico(v["id"]), reverse=True)
    return pref + otros + [{"id": GE, "nombre": "GE-Proton (se baja solo)", "ruta": GE, "version": ""}]


def por_defecto(protones: list[dict] | None = None) -> dict:
    protones = protones if protones is not None else lista()
    return protones[0]


def buscar(pid: str | None, protones: list[dict] | None = None) -> dict:
    """El Proton con ese id; si ya no está (lo quitaste), el de serie."""
    protones = protones if protones is not None else lista()
    return next((p for p in protones if p["id"] == pid), None) or por_defecto(protones)
