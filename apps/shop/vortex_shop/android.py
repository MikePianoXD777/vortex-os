"""Juegos de Android (4.0): la Play Store dentro de Vortexine's OS, con Waydroid.

La tienda no habla con Google: todo lo hace el comando `vortex-android` (preparar Android, abrir la
Play Store en un juego, abrir y quitar lo instalado). Aquí solo se llama en un hilo y se traduce a
algo que QML pinta.

Para pruebas: VORTEX_SHOP_ANDROID=/ruta/a/un/vortex-android falso cambia el comando que se llama.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
from pathlib import Path
from typing import Callable

URL_REGISTRO = "https://www.google.com/android/uncertified"
_PAQUETE = re.compile(r"^[A-Za-z0-9_.]+$")

# Juegos que funcionan bien en Waydroid (x86 con traducción de ARM) y no traen anti-trampas que
# bloqueen Android en PC. Solo nombre y paquete: la ficha completa (fotos, reseñas) la da la Play Store.
POPULARES: list[dict] = [
    {"paquete": "com.kiloo.subwaysurf", "nombre": "Subway Surfers", "genero": "Carreras sin fin"},
    {"paquete": "com.supercell.clashroyale", "nombre": "Clash Royale", "genero": "Estrategia"},
    {"paquete": "com.supercell.brawlstars", "nombre": "Brawl Stars", "genero": "Acción"},
    {"paquete": "com.supercell.clashofclans", "nombre": "Clash of Clans", "genero": "Estrategia"},
    {"paquete": "com.innersloth.spacemafia", "nombre": "Among Us", "genero": "Fiesta"},
    {"paquete": "com.kitkagames.fallbuddies", "nombre": "Stumble Guys", "genero": "Fiesta"},
    {"paquete": "com.robtopx.geometrydashlite", "nombre": "Geometry Dash Lite", "genero": "Ritmo"},
    {"paquete": "com.king.candycrushsaga", "nombre": "Candy Crush Saga", "genero": "Puzles"},
    {"paquete": "com.fingersoft.hillclimb", "nombre": "Hill Climb Racing", "genero": "Carreras"},
    {"paquete": "com.ea.game.pvz2_row", "nombre": "Plants vs. Zombies 2", "genero": "Estrategia"},
    {"paquete": "com.rovio.baba", "nombre": "Angry Birds 2", "genero": "Puzles"},
    {"paquete": "com.mojang.minecraftpe", "nombre": "Minecraft", "genero": "Construcción"},
]


class ErrorAndroid(Exception):
    """Algo de Android no salió; el texto es para la persona."""


def _comando() -> str:
    return os.environ.get("VORTEX_SHOP_ANDROID") or "vortex-android"


def _correr(*args: str, timeout: int = 180) -> str:
    try:
        r = subprocess.run([_comando(), *args], capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        raise ErrorAndroid("Falta el comando vortex-android: corre «vortex actualizar».") from None
    except subprocess.TimeoutExpired:
        raise ErrorAndroid("Android tardó demasiado en responder. Vuelve a intentarlo.") from None
    if r.returncode != 0:
        detalle = (r.stderr.strip().splitlines() or [""])[-1].removeprefix("vortex-android: ")
        raise ErrorAndroid(_explicar(detalle))
    return r.stdout


def _explicar(detalle: str) -> str:
    """Los errores del comando, dichos para la persona."""
    if "primero" in detalle and "preparar" in detalle:
        return "Primero hay que preparar Android (botón «Preparar Android»)."
    if "no arrancó" in detalle:
        return "Android no arrancó. Ciérralo desde el panel y vuelve a intentarlo."
    if "abre la Play Store" in detalle:
        return "Abre la Play Store una vez y vuelve a pedir el número."
    return (detalle[:1].upper() + detalle[1:] + ".") if detalle else "No se pudo. Vuelve a intentarlo."


def _paquete(paquete: str) -> str:
    if not _PAQUETE.match(paquete or ""):
        raise ErrorAndroid("Ese juego no tiene un nombre válido.")
    return paquete


def estado() -> dict:
    try:
        datos = json.loads(_correr("estado", timeout=20) or "{}")
    except (ErrorAndroid, json.JSONDecodeError):
        datos = {}
    base = {"instalado": False, "preparado": False, "corriendo": False, "nvidia": False, "registrado": False}
    return base | {k: bool(v) for k, v in datos.items() if k in base}


def preparar(progreso: Callable[[int, str], None]) -> None:
    """La primera vez (pide la contraseña una vez con pkexec). Avisa el paso por `progreso(pct, texto)`."""
    try:
        p = subprocess.Popen([_comando(), "preparar"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    except FileNotFoundError:
        raise ErrorAndroid("Falta el comando vortex-android: corre «vortex actualizar».") from None
    assert p.stdout is not None
    for linea in p.stdout:
        m = re.match(r"PASO (\d+)/(\d+) (.*)", linea.strip())
        if m:
            n, total = int(m[1]), int(m[2])
            progreso(int((n - 1) * 100 / total), m[3])
    errores = p.stderr.read() if p.stderr else ""
    if p.wait() != 0:
        if p.returncode in (126, 127) and not errores.strip():
            raise ErrorAndroid("No se dio permiso. Vuelve a intentarlo y escribe tu contraseña.")
        detalle = (errores.strip().splitlines() or [""])[-1].removeprefix("vortex-android: ")
        raise ErrorAndroid(_explicar(detalle))
    progreso(100, "Listo")


def tienda(paquete: str = "") -> None:
    _correr("tienda", *([_paquete(paquete)] if paquete else []))


def buscar(texto: str) -> None:
    _correr("buscar", texto.strip())


def abrir(paquete: str) -> None:
    _correr("abrir", _paquete(paquete))


def quitar(paquete: str) -> None:
    _correr("quitar", _paquete(paquete))


def apps() -> list[dict]:
    try:
        lista = json.loads(_correr("apps", timeout=30) or "[]")
    except (ErrorAndroid, json.JSONDecodeError):
        return []
    salida = []
    for a in lista:
        icono = a.get("icono") or ""
        if icono and Path(icono).is_file():
            icono = Path(icono).as_uri()
        elif icono:
            icono = ""
        salida.append({"paquete": a.get("paquete", ""), "nombre": a.get("nombre") or a.get("paquete", ""),
                       "icono": icono})
    return sorted(salida, key=lambda a: a["nombre"].lower())


def id_google() -> str:
    numero = _correr("id-google").strip()
    if not numero.isdigit():
        raise ErrorAndroid("Abre la Play Store una vez y vuelve a pedir el número.")
    return numero


def registrado() -> None:
    _correr("registrado", timeout=10)


def parar() -> None:
    _correr("parar", timeout=60)
