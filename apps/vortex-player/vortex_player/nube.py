"""Tu biblioteca en la nube: un JSON en la carpeta oculta de la app en tu Google Drive, el mismo que lee
Vortex Player en el teléfono (`Nube.kt`). Guarda las canciones de YouTube (por su id) y las playlists:

    {"formato": 1,
     "canciones": {"<id>": {"titulo", "artista", "album", "duracion", "agregada"}},
     "quitadas":  {"<id>": cuándo},
     "playlists": {"<uuid>": {"nombre", "url", "canciones": ["<id>", …], "modificada"}},
     "borradas":  {"<uuid>": cuándo}}

Los tiempos van en milisegundos. Mezclar dos copias nunca pierde nada: las canciones se juntan, una
canción quitada se queda quitada salvo que alguien la vuelva a bajar después, y de una playlist gana la
que se cambió al último.
"""
from __future__ import annotations

import copy
import json
from dataclasses import dataclass, field

FORMATO = 1
SECCIONES = ("canciones", "quitadas", "playlists", "borradas")


def vacio() -> dict:
    return {"formato": FORMATO, **{k: {} for k in SECCIONES}}


def leer(texto: str | bytes | None) -> dict:
    try:
        d = json.loads(texto or "")
    except ValueError:
        return vacio()
    if not isinstance(d, dict):
        return vacio()
    for k in SECCIONES:
        if not isinstance(d.get(k), dict):
            d[k] = {}
    d["formato"] = FORMATO
    return d


def texto(doc: dict) -> str:
    return json.dumps(doc, ensure_ascii=False, sort_keys=True)


def mezclar(a: dict, b: dict) -> dict:
    """Junta dos copias (la de la nube y la de aquí). Da lo mismo el orden."""
    salida = vacio()
    canciones, quitadas, playlists, borradas = (salida[k] for k in SECCIONES)
    for doc in (a, b):
        for vid, c in doc.get("canciones", {}).items():
            if vid not in canciones or c.get("agregada", 0) > canciones[vid].get("agregada", 0):
                canciones[vid] = copy.deepcopy(c)
        for vid, t in doc.get("quitadas", {}).items():
            quitadas[vid] = max(t, quitadas.get(vid, 0))
        for u, p in doc.get("playlists", {}).items():
            if u not in playlists or p.get("modificada", 0) > playlists[u].get("modificada", 0):
                playlists[u] = copy.deepcopy(p)
        for u, t in doc.get("borradas", {}).items():
            borradas[u] = max(t, borradas.get(u, 0))
    # Lo que se volvió a agregar después de quitarlo, revive; lo demás se queda quitado
    for vid in list(quitadas):
        if vid in canciones:
            if canciones[vid].get("agregada", 0) > quitadas[vid]:
                del quitadas[vid]
            else:
                del canciones[vid]
    for u in list(borradas):
        if u in playlists:
            if playlists[u].get("modificada", 0) > borradas[u]:
                del borradas[u]
            else:
                del playlists[u]
    return salida


@dataclass
class Local:
    """Lo que hay en esta PC, como lo entiende la nube."""
    canciones: dict[str, dict] = field(default_factory=dict)    # id → {titulo, artista, album, duracion, agregada}
    quitadas: dict[str, int] = field(default_factory=dict)
    playlists: dict[str, dict] = field(default_factory=dict)    # uuid → {nombre, url, canciones, modificada}
    borradas: dict[str, int] = field(default_factory=dict)


@dataclass
class Plan:
    """Lo que hay que hacer aquí para quedar igual que la nube."""
    bajar: list[tuple[str, dict]]          # (id, datos)
    borrar: list[str]
    playlists: list[tuple[str, dict]]      # (uuid, playlist)
    quitar_playlists: list[str]


def desde(local: Local) -> dict:
    return {"formato": FORMATO, "canciones": copy.deepcopy(local.canciones), "quitadas": dict(local.quitadas),
            "playlists": copy.deepcopy(local.playlists), "borradas": dict(local.borradas)}


def plan(doc: dict, local: Local) -> Plan:
    def misma(a: dict, b: dict) -> bool:
        return all(a.get(k) == b.get(k) for k in ("nombre", "url", "canciones", "modificada"))

    return Plan(
        bajar=sorted(((vid, c) for vid, c in doc["canciones"].items() if vid not in local.canciones),
                     key=lambda x: x[1].get("agregada", 0)),
        borrar=[vid for vid, t in doc["quitadas"].items()
                if vid in local.canciones and local.canciones[vid].get("agregada", 0) <= t],
        playlists=[(u, p) for u, p in doc["playlists"].items()
                   if u not in local.playlists or (local.playlists[u].get("modificada", 0) <= p.get("modificada", 0)
                                                   and not misma(local.playlists[u], p))],
        quitar_playlists=[u for u, t in doc["borradas"].items()
                          if u in local.playlists and local.playlists[u].get("modificada", 0) <= t],
    )


def iguales(a: dict, b: dict) -> bool:
    return texto(a) == texto(b)
