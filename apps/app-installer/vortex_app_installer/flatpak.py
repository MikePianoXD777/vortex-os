"""Flatpak para tu usuario (flatpak --user): sin contraseña, como la tienda.

Instala un .flatpakref, un .flatpak (paquete suelto) o una app de Flathub, con el avance que va
diciendo flatpak, y desinstala lo que se instaló así.
"""
from __future__ import annotations

import configparser
import os
import re
import shutil
import signal
import subprocess
import threading
import time
from pathlib import Path

from .textos import tr

REMOTO = "flathub"
URL_REPO = "https://dl.flathub.org/repo/flathub.flatpakrepo"


class ErrorFlatpak(Exception):
    pass


class Cancelado(Exception):
    pass


def _exe() -> str:
    exe = shutil.which("flatpak")
    if not exe:
        raise ErrorFlatpak(tr("falta_flatpak"))
    return exe


def hay_flatpak() -> bool:
    return shutil.which("flatpak") is not None


def leer_ref(ruta: Path) -> dict:
    """{nombre_programa, app_id, titulo} de un .flatpakref (un archivo de texto tipo INI)."""
    cp = configparser.ConfigParser(interpolation=None)
    try:
        cp.read(ruta, encoding="utf-8")
        g = cp["Flatpak Ref"]
    except (configparser.Error, KeyError, OSError, UnicodeDecodeError):
        return {}
    return {"app_id": g.get("Name", ""), "titulo": g.get("Title", ""),
            "nombre_programa": g.get("Title", "") or g.get("Name", "")}


def apps_instaladas() -> set[str]:
    try:
        r = subprocess.run([_exe(), "list", "--user", "--app", "--columns=application"], stdin=subprocess.DEVNULL,
                           capture_output=True, text=True, errors="replace", timeout=60, check=False)
    except (OSError, subprocess.TimeoutExpired, ErrorFlatpak):
        return set()
    return {l.strip() for l in r.stdout.splitlines() if l.strip()}


def _correr(args: list[str], progreso=None, cancelar: threading.Event | None = None) -> str:
    env = dict(os.environ, LC_ALL="C.UTF-8", LANG="C.UTF-8")
    proc = subprocess.Popen([_exe(), *args], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, env=env, start_new_session=True)
    salida = bytearray()

    def leer():
        while True:
            bloque = proc.stdout.read1(4096) if hasattr(proc.stdout, "read1") else proc.stdout.read(4096)
            if not bloque:
                return
            salida.extend(bloque)
            if progreso:
                trozo = bytes(salida[-400:]).decode("utf-8", "replace")
                cifras = re.findall(r"(\d{1,3})%", trozo)
                if cifras:
                    progreso(min(1.0, int(cifras[-1]) / 100))

    hilo = threading.Thread(target=leer, daemon=True)
    hilo.start()
    while proc.poll() is None:
        if cancelar is not None and cancelar.is_set():
            try:
                os.killpg(proc.pid, signal.SIGTERM)
            except OSError:
                proc.terminate()
            proc.wait()
            raise Cancelado()
        time.sleep(0.15)
    hilo.join(timeout=2)
    texto = salida.decode("utf-8", "replace")
    if proc.returncode != 0:
        t = texto.lower()
        if "could not resolve" in t or "network" in t or "unable to connect" in t or "timeout" in t:
            raise ErrorFlatpak(tr("sin_internet"))
        if "already installed" in t:
            return texto
        raise ErrorFlatpak(tr("flatpak_fallo"))
    return texto


def preparar_flathub() -> None:
    _correr(["remote-add", "--user", "--if-not-exists", REMOTO, URL_REPO])


def instalar_archivo(ruta: Path, tipo: str, progreso=None, cancelar=None) -> str | None:
    """Instala un .flatpakref o un .flatpak. Devuelve el id de la app instalada (si se pudo saber)."""
    antes = apps_instaladas()
    opcion = "--from" if tipo == "flatpakref" else "--bundle"
    _correr(["install", "--user", "-y", "--noninteractive", opcion, str(ruta)], progreso, cancelar)
    nuevas = apps_instaladas() - antes
    if tipo == "flatpakref":
        app = leer_ref(ruta).get("app_id")
        if app:
            return app
    return sorted(nuevas)[0] if nuevas else None


def instalar_flathub(app_id: str, progreso=None, cancelar=None) -> str:
    preparar_flathub()
    _correr(["install", "--user", "-y", "--noninteractive", REMOTO, app_id], progreso, cancelar)
    return app_id


def desinstalar(app_id: str) -> None:
    _correr(["uninstall", "--user", "-y", "--noninteractive", app_id])


def abrir(app_id: str) -> None:
    subprocess.Popen([_exe(), "run", app_id], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                     stderr=subprocess.DEVNULL, start_new_session=True)
