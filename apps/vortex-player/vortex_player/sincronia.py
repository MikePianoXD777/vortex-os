"""Deja esta PC igual que tu biblioteca en la nube (y la nube con lo nuevo de aquí): sube lo que cambió,
quita lo que borraste en el teléfono y arma las playlists. Lo que falta bajar lo baja el puente con su cola
de descargas, y al terminar se vuelve a sincronizar para meter lo nuevo a sus playlists."""
from __future__ import annotations

import contextlib
from typing import Callable

from . import nube
from .biblioteca import Biblioteca


def sincronizar(bib: Biblioteca, drive, borrar: Callable[[str, str], None],
                candado=contextlib.nullcontext()) -> tuple[dict, nube.Plan]:
    """`drive` hace buscar/leer/crear/escribir; `borrar(id, ruta)` quita una canción de esta PC; `candado`
    protege la base (lo de internet va fuera de él). Devuelve la biblioteca de la nube ya mezclada y el
    plan (en `plan.bajar` lo que falta bajar)."""
    archivo = drive.buscar()
    remoto = nube.leer(drive.leer(archivo)) if archivo else nube.vacio()
    with candado:
        nuevo = nube.mezclar(remoto, nube.desde(bib.estado_nube()))
    if archivo is None:
        drive.crear(nube.texto(nuevo))
    elif not nube.iguales(nuevo, remoto):
        drive.escribir(archivo, nube.texto(nuevo))
    with candado:
        plan = nube.plan(nuevo, bib.estado_nube())
        rutas = bib.rutas_por_id()
        for vid in plan.borrar:
            if vid in rutas:
                borrar(vid, rutas[vid])
        for u in plan.quitar_playlists:
            bib.quitar_playlist_nube(u)
        armar_playlists(bib, nuevo)
    return nuevo, plan


def armar_playlists(bib: Biblioteca, doc: dict) -> None:
    rutas = bib.rutas_por_id()
    for u, p in nube.plan(doc, bib.estado_nube()).playlists:
        bib.poner_playlist(u, p, rutas)
