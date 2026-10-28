"""Mods de Minecraft (.jar de Fabric, Quilt, Forge o NeoForge).

Si existe el comando «vortex mods» (de Vortexine's OS), la app le pide las instancias de Minecraft
(«vortex mods instancias») y le pasa el mod («vortex mods mover ARCHIVO RUTA»). Si no, la persona
elige la carpeta a mano.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tomllib
import zipfile
from pathlib import Path

from .comprimidos import Conflicto, nombre_libre
from .textos import tr


def leer_mod(ruta: Path) -> dict | None:
    """{nombre, id, version, loader, minecraft} si el .jar es un mod; None si es otro programa de Java."""
    try:
        with zipfile.ZipFile(ruta) as z:
            nombres = set(z.namelist())
            if "fabric.mod.json" in nombres or "quilt.mod.json" in nombres:
                quilt = "quilt.mod.json" in nombres and "fabric.mod.json" not in nombres
                datos = json.loads(z.read("quilt.mod.json" if quilt else "fabric.mod.json").decode("utf-8", "replace"),
                                   strict=False)
                if quilt:
                    meta = datos.get("quilt_loader", {}) if isinstance(datos, dict) else {}
                    return {"nombre": (meta.get("metadata") or {}).get("name") or meta.get("id") or ruta.stem,
                            "id": meta.get("id", ""), "version": str(meta.get("version", "")), "loader": "quilt",
                            "minecraft": ""}
                depende = datos.get("depends") if isinstance(datos.get("depends"), dict) else {}
                mc = depende.get("minecraft", "")
                return {"nombre": datos.get("name") or datos.get("id") or ruta.stem, "id": datos.get("id", ""),
                        "version": str(datos.get("version", "")), "loader": "fabric",
                        "minecraft": mc if isinstance(mc, str) else ", ".join(map(str, mc))}
            for archivo, loader in (("META-INF/neoforge.mods.toml", "neoforge"), ("META-INF/mods.toml", "forge")):
                if archivo in nombres:
                    try:
                        datos = tomllib.loads(z.read(archivo).decode("utf-8", "replace"))
                    except tomllib.TOMLDecodeError:
                        datos = {}
                    mod = (datos.get("mods") or [{}])[0] if isinstance(datos.get("mods"), list) else {}
                    version = str(mod.get("version", ""))
                    return {"nombre": mod.get("displayName") or mod.get("modId") or ruta.stem,
                            "id": mod.get("modId", ""), "version": "" if version.startswith("$") else version,
                            "loader": loader, "minecraft": ""}
    except (OSError, zipfile.BadZipFile, ValueError, KeyError, AttributeError, TypeError):
        return None
    return None


def _vortex() -> str | None:
    return shutil.which("vortex")


def instancias() -> list[dict] | None:
    """Las instancias de Minecraft según «vortex mods instancias», o None si ese comando no existe."""
    exe = _vortex()
    if not exe:
        return None
    try:
        r = subprocess.run([exe, "mods", "instancias"], stdin=subprocess.DEVNULL, capture_output=True,
                           text=True, errors="replace", timeout=30, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    texto = r.stdout.strip()
    try:   # una lista JSON o una instancia JSON por renglón
        datos = json.loads(texto) if texto.startswith("[") else [json.loads(l) for l in texto.splitlines() if l.strip()]
    except ValueError:
        return None
    return [{"nombre": str(d.get("nombre", "")), "launcher": str(d.get("launcher", "")),
             "ruta": str(d.get("ruta", "")), "version": str(d.get("version", "")), "loader": str(d.get("loader", ""))}
            for d in datos if isinstance(d, dict) and d.get("ruta")]


def mover(archivo: Path, ruta_instancia: str) -> Path:
    """Pasa el mod a la instancia con «vortex mods mover». Devuelve dónde quedó."""
    exe = _vortex()
    if not exe:
        raise RuntimeError(tr("mods_sin_comando"))
    destino = Path(ruta_instancia) / "mods" / archivo.name
    if os.path.lexists(destino):
        raise Conflicto(destino, nombre_libre(destino))
    r = subprocess.run([exe, "mods", "mover", str(archivo), ruta_instancia], stdin=subprocess.DEVNULL,
                       capture_output=True, text=True, errors="replace", timeout=60, check=False)
    if r.returncode != 0:
        raise RuntimeError(tr("mods_no_se_movio"))
    lineas = [l for l in r.stdout.strip().splitlines() if l.strip()]
    return Path(lineas[-1]) if lineas else destino


def copiar_a_carpeta(archivo: Path, carpeta: Path, nombre: str | None = None) -> Path:
    """Copia el mod a la carpeta que eligió la persona (a su «mods/» si eligió la del juego)."""
    carpeta = Path(carpeta)
    if (carpeta / "mods").is_dir() and carpeta.name != "mods":
        carpeta = carpeta / "mods"
    destino = carpeta / (nombre or archivo.name)
    if os.path.lexists(destino):
        raise Conflicto(destino, nombre_libre(destino))
    carpeta.mkdir(parents=True, exist_ok=True)
    shutil.copy2(archivo, destino)
    return destino
