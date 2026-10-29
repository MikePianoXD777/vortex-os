"""El ecualizador de Vortex Player es el mismo del panel de música del escritorio (Serpantinum).

No hay dos: los dos lados leen el mismo estado (~/.local/state/serpantinum/music/eq_state.json) y lo
cambian con el mismo script del shell (media/equalizer.sh), que arma el preset «live_eq» de EasyEffects
para todo el sistema. Si mueves una banda aquí, el panel la ve moverse, y al revés. Fuera de
Vortexine's (sin el script) la pestaña del ecualizador no aparece.
"""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

FRECUENCIAS = [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
PRESETS = ["Flat", "Bass", "Treble", "Vocal", "Pop", "Rock", "Jazz", "Classic"]
MIN_BANDAS, MAX_BANDAS = 2, 16


def script() -> Path | None:
    for base in (os.environ.get("SERPANTINUM_DIR", ""), "/usr/share/serpantinum/src"):
        if base and (Path(base) / "quickshell/media/equalizer.sh").is_file():
            return Path(base) / "quickshell/media/equalizer.sh"
    return None


def archivo_estado() -> Path:
    return Path(os.environ.get("VORTEX_PLAYER_EQ") or Path.home() / ".local/state/serpantinum/music/eq_state.json")


def bandas_de(datos: dict) -> list[dict]:
    """Igual que bandasDe() del panel: la lista {f, g}, o las 10 de antes (b1…b10)."""
    b = datos.get("bandas") if isinstance(datos, dict) else None
    if isinstance(b, list) and len(b) >= MIN_BANDAS:
        return [{"f": float(x.get("f", 0)), "g": float(x.get("g", 0))} for x in b]
    def g(i: int) -> float:
        try:
            return float(datos.get(f"b{i + 1}", 0))
        except (TypeError, ValueError, AttributeError):
            return 0.0
    return [{"f": float(f), "g": g(i)} for i, f in enumerate(FRECUENCIAS)]


def estado() -> dict:
    try:
        d = json.loads(archivo_estado().read_text())
    except (OSError, ValueError):
        d = {}
    if not isinstance(d, dict):
        d = {}
    return {"bandas": bandas_de(d), "preset": d.get("preset", "Flat"), "pending": bool(d.get("pending", False))}


def texto_bandas(bandas: list[dict]) -> str:
    """«31:5,63:7,…» para «set_bands»."""
    def n(x: float) -> str:
        return f"{x:g}"
    return ",".join(f"{n(round(b['f']))}:{n(round(max(-12.0, min(12.0, b['g'])), 1))}" for b in bandas)


def correr(*args: str) -> bool:
    s = script()
    if s is None:
        return False
    try:
        return subprocess.run(["bash", str(s), *args], capture_output=True, timeout=30).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def poner_bandas(bandas: list[dict]) -> bool:
    """Guarda la lista y la aplica (lo mismo que mover y «Aplicar» en el panel)."""
    if not MIN_BANDAS <= len(bandas) <= MAX_BANDAS:
        return False
    return correr("set_bands", texto_bandas(bandas)) and correr("apply")


def poner_preset(nombre: str) -> bool:
    return nombre in PRESETS and correr("preset", nombre)
