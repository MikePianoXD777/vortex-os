"""Lo que la app instaló o descomprimió, para poder deshacerlo (pestaña «Instalado»).

Se guarda en ~/.local/share/vortex-app-installer/instalado.json: una lista de entradas con
{id, tipo, nombre, fecha, rutas, icono, flatpak, archivo, abrir}. Deshacer manda sus rutas a la
papelera (nunca borra para siempre) o, si es un Flatpak, lo desinstala con flatpak --user.
Una entrada cuyas rutas ya no existen (alguien las borró a mano) desaparece sola de la lista.
"""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import tempfile
import unicodedata
from datetime import datetime
from pathlib import Path

from . import papelera, rutas

TIPOS = ("appimage", "programa", "flatpak", "mod", "descomprimido")


def slug(texto: str) -> str:
    """«Vortex Studio 1.0» → «vortex-studio-1.0» (para nombres de carpetas, íconos e ids)."""
    t = unicodedata.normalize("NFKD", texto or "").encode("ascii", "ignore").decode()
    t = re.sub(r"[^A-Za-z0-9._-]+", "-", t).strip("-._").lower()
    return t[:60] or "app"


def cargar() -> list[dict]:
    try:
        datos = json.loads(rutas.registro().read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return []
    lista = [e for e in datos if isinstance(e, dict) and e.get("id") and e.get("tipo") in TIPOS] \
        if isinstance(datos, list) else []
    vivas = [e for e in lista if e["tipo"] == "flatpak"
             or any(os.path.lexists(r) for r in e.get("rutas") or [])]
    if len(vivas) != len(lista):
        guardar(vivas)
    return vivas


def guardar(lista: list[dict]) -> None:
    destino = rutas.registro()
    destino.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=destino.parent, prefix=".instalado-", suffix=".json")
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump(lista, f, ensure_ascii=False, indent=1)
    os.replace(tmp, destino)


def buscar(id_: str) -> dict | None:
    return next((e for e in cargar() if e["id"] == id_), None)


def agregar(entrada: dict) -> dict:
    entrada = dict(entrada)
    entrada.setdefault("fecha", datetime.now().isoformat(timespec="seconds"))
    entrada.setdefault("rutas", [])
    lista = [e for e in cargar() if e["id"] != entrada["id"]]
    lista.insert(0, entrada)
    guardar(lista)
    return entrada


def id_libre(tipo: str, nombre: str) -> str:
    """Id nuevo para lo que puede repetirse (cada carpeta descomprimida, cada mod)."""
    base = "%s-%s" % (tipo, slug(nombre))
    usados = {e["id"] for e in cargar()}
    n, id_ = 1, base
    while id_ in usados:
        n += 1
        id_ = "%s-%d" % (base, n)
    return id_


def _se_puede_quitar(ruta: str) -> bool:
    """Nunca la carpeta personal, ni Descargas, ni ~/Applications enteras, ni nada fuera de lo absoluto."""
    if not ruta or not os.path.isabs(ruta):
        return False
    p = Path(os.path.abspath(ruta))
    if p in rutas.protegidas():
        return False
    return p not in Path(os.path.abspath(rutas.casa())).parents   # ni /home ni /


def deshacer(id_: str) -> dict:
    """Deshace una entrada. Devuelve la entrada quitada; ValueError si no existe."""
    entrada = buscar(id_)
    if entrada is None:
        raise ValueError(id_)
    if entrada["tipo"] == "flatpak" and entrada.get("flatpak"):
        from . import flatpak
        flatpak.desinstalar(entrada["flatpak"])
    for r in entrada.get("rutas") or []:
        if _se_puede_quitar(r):
            papelera.mandar(r)
    guardar([e for e in cargar() if e["id"] != id_])
    refrescar_lanzador()
    return entrada


def refrescar_lanzador() -> None:
    """Que el lanzador vea al momento los .desktop nuevos o quitados."""
    carpeta = rutas.lanzadores()
    if shutil.which("update-desktop-database") and carpeta.is_dir():
        subprocess.run(["update-desktop-database", "-q", str(carpeta)], stdin=subprocess.DEVNULL,
                       capture_output=True, check=False)
