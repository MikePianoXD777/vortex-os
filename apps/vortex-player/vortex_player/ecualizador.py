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


# --- 2.4: perfiles (los mismos que en el teléfono, Ecualizador.kt) -----------------------------------------
# dB sobre FRECUENCIAS; se pasan a las bandas que tengas (2 a 16) interpolando en escala logarítmica.

PERFILES: dict[str, list[float]] = {
    "Plano": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "Graves": [6, 6, 5, 3, 1, 0, 0, 0, 0, 0],
    "Graves al máximo": [9, 8, 6, 3, 0, -1, 0, 0, 0, 0],
    "Menos graves": [-6, -5, -4, -2, 0, 0, 0, 0, 0, 0],
    "Agudos": [0, 0, 0, 0, 0, 1, 3, 5, 6, 7],
    "Voz": [-2, -2, -1, 1, 3, 4, 4, 3, 1, 0],
    "Pop": [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2],
    "Rock": [5, 4, 3, 1, -1, -1, 1, 3, 4, 5],
    "Metal": [4, 4, 2, 0, -2, 0, 2, 4, 5, 4],
    "Electrónica": [6, 5, 2, 0, -2, 1, 0, 2, 4, 5],
    "Hip hop": [6, 5, 3, 2, -1, -1, 1, 0, 2, 3],
    "Reggaetón": [7, 6, 3, 0, -1, 0, 1, 2, 2, 2],
    "J-pop y anime": [2, 2, 1, 0, 1, 2, 3, 4, 4, 3],
    "Clásica": [4, 3, 2, 1, -1, -1, 0, 2, 3, 4],
    "Jazz": [3, 2, 1, 2, -2, -2, 0, 1, 3, 4],
    "Piano": [2, 1, 0, 2, 3, 1, 3, 4, 3, 2],
    "Acústica": [4, 3, 2, 1, 1, 1, 2, 3, 3, 2],
    "Lo-fi": [3, 3, 2, 1, 0, -1, -2, -3, -4, -5],
    "Volumen bajo": [6, 5, 3, 0, -1, 0, 0, 2, 4, 5],
}


def db_en(perfil: list[float], hz: float) -> float:
    """Los dB del perfil en `hz`, interpolando entre sus frecuencias en escala logarítmica."""
    import math
    x = math.log(max(1.0, hz))
    f = [math.log(v) for v in FRECUENCIAS]
    if x <= f[0]:
        return perfil[0]
    if x >= f[-1]:
        return perfil[-1]
    i = max(k for k, v in enumerate(f) if v <= x)
    t = (x - f[i]) / (f[i + 1] - f[i])
    return perfil[i] + (perfil[i + 1] - perfil[i]) * t


def bandas_de_perfil(nombre: str, bandas: list[dict]) -> list[dict]:
    """Tus bandas (las mismas frecuencias) con las ganancias del perfil, redondeadas a 0,1 dB."""
    p = PERFILES[nombre]
    return [{"f": b["f"], "g": round(db_en(p, b["f"]), 1)} for b in bandas]


def perfil_de(bandas: list[dict]) -> str:
    """El perfil que coincide con tus bandas, o «Personalizado»."""
    for nombre in PERFILES:
        if all(abs(b["g"] - g["g"]) < 0.15 for b, g in zip(bandas, bandas_de_perfil(nombre, bandas))):
            return nombre
    return "Personalizado"


def poner_perfil(nombre: str, bandas: list[dict]) -> bool:
    return nombre in PERFILES and poner_bandas(bandas_de_perfil(nombre, bandas))
