"""~/.config/vortex/miku.json: {"avatar": true, "voz": "<id>", "modelo": "<ruta opcional>", "elevenlabs_voz": "<Voice ID>"}.

Lo que falte toma su valor por defecto y un JSON inválido no truena: se usan los valores por defecto
(y al guardar no se pierden las claves que no conocemos).
"""

import json
import os
from pathlib import Path

from . import rutas

DEFECTOS = {
    "avatar": True,                 # Miku en 3D; apagado queda solo el asistente de voz
    "voz": "supertonic-f1",          # id de voces.json
    "modelo": "",                   # ruta a otro .vrm (vacío = el de siempre)
    "seguir_escuchando": True,      # tras contestar, vuelve a escuchar unos segundos
    "silencio": 1.5,                # segundos callada que cierran tu turno
    "internet": False,              # buscar en la web y leer páginas (cada vez con tu permiso)
    "elevenlabs_voz": "",           # Voice ID de ElevenLabs (con la voz «elevenlabs»); la clave va en el llavero
    "elevenlabs_modelo": "",        # vacío = eleven_v4_turbo
}


def _como_bool(v, defecto: bool) -> bool:
    return v if isinstance(v, bool) else defecto


def _como_numero(v, defecto: float, minimo: float, maximo: float) -> float:
    if isinstance(v, bool) or not isinstance(v, (int, float)):
        return defecto
    return min(max(float(v), minimo), maximo)


def cargar_crudo(ruta: Path | None = None) -> dict:
    """El JSON tal cual está en disco ({} si falta, no se puede leer o no es un objeto)."""
    try:
        datos = json.loads((ruta or rutas.CONFIG).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return {}
    return datos if isinstance(datos, dict) else {}


def cargar(ruta: Path | None = None) -> dict:
    """La configuración efectiva: cada clave con su valor por defecto si falta o viene mal."""
    crudo = cargar_crudo(ruta)
    cfg = dict(DEFECTOS)
    cfg["avatar"] = _como_bool(crudo.get("avatar"), DEFECTOS["avatar"])
    cfg["seguir_escuchando"] = _como_bool(crudo.get("seguir_escuchando"), DEFECTOS["seguir_escuchando"])
    cfg["internet"] = _como_bool(crudo.get("internet"), DEFECTOS["internet"])
    for clave in ("voz", "modelo", "elevenlabs_voz", "elevenlabs_modelo"):
        v = crudo.get(clave)
        if isinstance(v, str) and v.strip():
            cfg[clave] = v.strip()
    cfg["silencio"] = _como_numero(crudo.get("silencio"), DEFECTOS["silencio"], 0.6, 5.0)
    return cfg


def guardar(cambios: dict, ruta: Path | None = None) -> None:
    """Pone las claves de `cambios` en miku.json sin tocar las demás (crea la carpeta y el archivo).

    Si el archivo existente no es un JSON válido se reescribe solo con `cambios`.
    """
    destino = ruta or rutas.CONFIG
    datos = cargar_crudo(destino)
    datos.update(cambios)
    destino.parent.mkdir(parents=True, exist_ok=True)
    tmp = destino.with_name(destino.name + f".{os.getpid()}.tmp")
    tmp.write_text(json.dumps(datos, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    os.replace(tmp, destino)
