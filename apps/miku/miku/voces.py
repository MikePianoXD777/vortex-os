"""Catálogo de voces (voces.json): todas son voces preestablecidas de Supertonic 3 (F1 a F5).

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
    """¿Se puede hablar ya con esta voz? Necesita el modelo y el archivo de su estilo."""
    return modelo_listo() and archivo_estilo(voz).is_file()


def descargar_estilo(voz: dict, progreso=None) -> None:
    """Baja el archivito de una voz opcional (con su sha256). Las de fábrica no necesitan nada más."""
    a = voz.get("archivo")
    if a and not archivo_estilo(voz).is_file():
        descargas.bajar_archivo(rutas.VOZ, a, progreso)
