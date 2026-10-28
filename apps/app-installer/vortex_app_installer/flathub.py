"""Buscar en Flathub el mismo programa que un .deb, un .rpm o un .exe (la API abierta de Flathub,
la misma que usa Vortexine's Shop). VORTEX_APP_INSTALLER_FLATHUB cambia la dirección (pruebas)."""
from __future__ import annotations

import json
import os
import re
import unicodedata
import urllib.error
import urllib.request

from . import AGENTE
from .textos import idioma

API = "https://flathub.org/api/v2"


class SinRed(Exception):
    pass


def buscar(texto: str, por_pagina: int = 8) -> list[dict]:
    """[{id, nombre, resumen, icono}] de Flathub para ese texto. SinRed si no hay conexión."""
    base = os.environ.get("VORTEX_APP_INSTALLER_FLATHUB", API).rstrip("/")
    cuerpo = json.dumps({"query": texto[:100], "hits_per_page": por_pagina, "page": 1}).encode()
    pedido = urllib.request.Request(base + "/search?locale=" + idioma(), data=cuerpo, method="POST",
                                    headers={"Content-Type": "application/json", "User-Agent": AGENTE})
    try:
        with urllib.request.urlopen(pedido, timeout=12) as r:
            datos = json.loads(r.read().decode("utf-8", "replace"))
    except (urllib.error.URLError, OSError, ValueError, TimeoutError) as e:
        raise SinRed(str(e)) from None
    salida = []
    for hit in datos.get("hits") or [] if isinstance(datos, dict) else []:
        app_id = hit.get("app_id") if isinstance(hit, dict) else None
        if not isinstance(app_id, str) or hit.get("type") in ("runtime", "addon"):
            continue
        salida.append({"id": app_id, "nombre": str(hit.get("name") or app_id),
                       "resumen": str(hit.get("summary") or ""), "icono": str(hit.get("icon") or "")})
    return salida


def _norma(t: str) -> str:
    t = unicodedata.normalize("NFKD", t or "").encode("ascii", "ignore").decode().lower()
    return re.sub(r"[^a-z0-9]", "", t)


_SOBRAS = ("stable", "beta", "nightly", "bin", "desktop", "linux", "amd64", "x8664", "app")


def _raiz(nombre: str) -> str:
    n = _norma(nombre)
    cambio = True
    while cambio:
        cambio = False
        for s in _SOBRAS:
            if n.endswith(s) and len(n) > len(s) + 2:
                n, cambio = n[: -len(s)], True
    return n


_PRUEBA = re.compile(r"unstable|beta|nightly|canary|\bdev\b|preview|insiders|\bgit\b", re.I)


def para_buscar(nombre_paquete: str) -> str:
    """«google-chrome-stable» → «google chrome»: lo que se le pregunta a Flathub."""
    partes = [p for p in re.split(r"[-_.\s]+", nombre_paquete) if p]
    while len(partes) > 1 and _norma(partes[-1]) in _SOBRAS:
        partes.pop()
    return " ".join(partes) or nombre_paquete


def _nombre_limpio(nombre: str) -> str:
    return re.sub(r"\([^)]*\)", "", nombre)   # «Google Chrome (unstable)» → «Google Chrome»


def mejor(nombre_paquete: str, resultados: list[dict]) -> dict | None:
    """El resultado que es el mismo programa («google-chrome-stable» ↔ «Google Chrome»), o None.
    Una versión de prueba (beta, unstable, dev…) solo vale si el paquete también lo es."""
    objetivo = _raiz(nombre_paquete)
    if len(objetivo) < 2:
        return None
    de_prueba = bool(_PRUEBA.search(nombre_paquete.replace("-", " ")))
    validos = [r for r in resultados if de_prueba or not (_PRUEBA.search(r["nombre"]) or
                                                           _PRUEBA.search(r["id"].rsplit(".", 1)[-1]))]
    for r in validos:
        candidatos = {_raiz(_nombre_limpio(r["nombre"])), _raiz(r["id"].rsplit(".", 1)[-1])}
        if objetivo in candidatos:
            return r
    for r in validos:
        nombre = _raiz(_nombre_limpio(r["nombre"]))
        if len(nombre) >= 4 and (nombre in objetivo or objetivo in nombre):
            return r
    return None
