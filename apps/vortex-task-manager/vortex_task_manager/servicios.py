"""Servicios de systemd para la página «Servicios»: los tuyos (systemctl --user) y los del sistema.

De cada servicio: si está corriendo, si falló y si arranca solo al prender («Al iniciar»: enabled o
disabled; los static, generated, masked… no se pueden cambiar desde aquí). Las acciones sobre los del
sistema las autoriza polkit (el shell te pregunta tu contraseña).
"""
from __future__ import annotations

import json
import subprocess

CAMBIABLES = ("enabled", "disabled")
ACCIONES = ("start", "stop", "restart", "enable", "disable")


def _systemctl(usuario: bool, *args: str, timeout: float = 10) -> subprocess.CompletedProcess:
    return subprocess.run(["systemctl", *(["--user"] if usuario else []), *args],
                          capture_output=True, text=True, timeout=timeout)


def _json(usuario: bool, *args: str) -> list[dict]:
    try:
        r = _systemctl(usuario, *args, "-o", "json", "--no-pager")
        datos = json.loads(r.stdout or "[]")
    except (OSError, subprocess.TimeoutExpired, ValueError):
        return []
    return datos if isinstance(datos, list) else []


def unir(unidades: list[dict], archivos: list[dict]) -> list[dict]:
    """list-units (lo que corre) + list-unit-files (lo que arranca al prender) → filas ordenadas."""
    por_nombre: dict[str, dict] = {}
    for a in archivos:
        nombre = a.get("unit_file", "")
        if not nombre.endswith(".service") or nombre.endswith("@.service"):
            continue
        por_nombre[nombre] = {"unidad": nombre, "descripcion": "", "activo": "inactive", "sub": "dead",
                              "alInicio": a.get("state", "")}
    for u in unidades:
        nombre = u.get("unit", "")
        if not nombre.endswith(".service") or u.get("load") == "not-found":
            continue
        fila = por_nombre.setdefault(nombre, {"unidad": nombre, "alInicio": ""})
        fila.update(descripcion=u.get("description", ""), activo=u.get("active", "inactive"), sub=u.get("sub", ""))
    filas = []
    for f in por_nombre.values():
        if not f["alInicio"] and f["descripcion"] in ("", f["unidad"]):
            continue      # pasajeros (dbus-:1.14-…@0.service): sin archivo ni descripción, no se manejan
        f["nombre"] = f["unidad"].removesuffix(".service")
        f["cambiable"] = f["alInicio"] in CAMBIABLES
        filas.append(f)
    # Primero los que fallaron, luego los que corren y luego los demás; cada grupo por nombre.
    peso = {"failed": 0, "active": 1, "activating": 1, "deactivating": 1, "reloading": 1}
    return sorted(filas, key=lambda f: (peso.get(f["activo"], 2), f["nombre"].lower()))


def listar(usuario: bool) -> list[dict]:
    return unir(_json(usuario, "list-units", "--type=service", "--all"),
                _json(usuario, "list-unit-files", "--type=service"))


def accion(unidad: str, que: str, usuario: bool) -> str | None:
    """start/stop/restart/enable/disable. None si salió bien; si no, el mensaje de systemctl."""
    if que not in ACCIONES or not unidad.endswith(".service") or unidad.startswith("-"):
        return "Acción no válida."
    try:
        r = _systemctl(usuario, que, unidad, timeout=90)
    except (OSError, subprocess.TimeoutExpired):
        return f"systemctl tardó demasiado con {unidad}."
    if r.returncode == 0:
        return None
    error = r.stderr.strip().splitlines()
    return error[-1] if error else f"No se pudo ({que} {unidad})."
