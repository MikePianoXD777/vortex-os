"""Catálogo de voces (voces.json): las de Supertonic 3 (F1 a F5, locales), la natural (Qwen3-TTS en la GPU o
la CPU) y ElevenLabs (nube).

La F1 es la de fábrica y se baja junto con el modelo la primera vez que se abre a Miku (descargas.py);
las otras cuatro son un archivito de ~290 KB que se baja al elegirlas con «vortex-miku voz ID».
Solo biblioteca estándar: lo usan el comando «vortex-miku voz» (sin PySide ni numpy) y la app.
"""

import json
from pathlib import Path

from . import descargas, rutas

VOZ_DEFECTO = "supertonic-f1"


def catalogo() -> list[dict]:
    """Las voces de voces.json. Si falta o está roto, solo la de fábrica."""
    try:
        datos = json.loads(rutas.CATALOGO_VOCES.read_text(encoding="utf-8"))
        voces = [v for v in datos.get("voces", []) if isinstance(v, dict) and v.get("id")]
        if voces:
            return voces
    except (OSError, ValueError):
        pass
    return [{"id": VOZ_DEFECTO, "nombre": "Miku (F1)", "motor": "supertonic", "estilo_voz": "F1", "incluida": True}]


def buscar(id_voz: str) -> dict | None:
    return next((v for v in catalogo() if v["id"] == id_voz), None)


def archivo_estilo(voz: dict) -> Path:
    """El JSON con el estilo (el «timbre») de la voz dentro de ~/.local/share/vortex/miku/voz/."""
    return rutas.VOZ / "voice_styles" / f"{voz.get('estilo_voz', 'F1')}.json"


def modelo_listo() -> bool:
    """¿Está bajado el modelo de voz (Supertonic 3 y la F1)?"""
    return descargas.paquete_listo("voz")


def disponible(voz: dict) -> bool:
    """¿Se puede hablar ya con esta voz? Supertonic necesita el modelo y su estilo; ElevenLabs, clave y Voice ID."""
    if voz.get("motor") == "elevenlabs":
        from . import config, elevenlabs
        return bool(config.cargar().get("elevenlabs_voz")) and bool(elevenlabs.leer_clave())
    if voz.get("motor") == "qwen":
        return rutas.NATURAL_LISTO.is_file() and rutas.NATURAL_PYTHON.is_file()
    return modelo_listo() and archivo_estilo(voz).is_file()


def modo_natural() -> str:
    """Dónde corre la voz natural: «gpu» (PyTorch en una NVIDIA serie 20+), «cuda» (qwentts.cpp en una NVIDIA
    serie 10+, la misma voz) o «cpu» (lenta). Lo guarda «vortex-miku natural instalar»."""
    try:
        modo = rutas.NATURAL_MODO.read_text(encoding="utf-8").strip()
    except OSError:
        return "gpu"
    return modo if modo in ("gpu", "cuda", "cpu") else "gpu"


def modelo_natural_cpu(voz: dict) -> str:
    """Modo cpu: qué modelo se bajó. Desde la 1.7, el 1.7B (la misma voz que en la GPU) si hay RAM; las
    instalaciones de antes (sin el archivo) tienen el 0.6B."""
    try:
        modelo = rutas.NATURAL_MODELO.read_text(encoding="utf-8").strip()
    except OSError:
        return voz["modelo_cpu"]
    return modelo if modelo in (voz["modelo"], voz["modelo_cpu"]) else voz["modelo_cpu"]


def cuantizacion_natural() -> str:
    """Modo cuda: con qué cuantización se bajó el modelo (Q8_0 o Q4_K_M)."""
    try:
        cuant = rutas.NATURAL_CUANT.read_text(encoding="utf-8").strip()
    except OSError:
        return "Q4_K_M"
    return cuant if cuant in ("Q8_0", "Q4_K_M") else "Q4_K_M"


def descargar_estilo(voz: dict, progreso=None) -> None:
    """Baja el archivito de una voz opcional (con su sha256). Las de fábrica no necesitan nada más."""
    a = voz.get("archivo")
    if a and not archivo_estilo(voz).is_file():
        descargas.bajar_archivo(rutas.VOZ, a, progreso)
