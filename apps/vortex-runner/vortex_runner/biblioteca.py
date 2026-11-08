"""La biblioteca: los programas y juegos de Windows que agregaste, en biblioteca.json.

Cada entrada: {id, nombre, exe, prefijo, proton (id o "" = el de serie), argumentos, icono, agregado,
ultima_vez, minutos, lanzador}. El prefijo es la carpeta de Wine de ese programa (su «disco C:» y su
registro): prefijos/<id>. Un programa instalado con «Instalar programa» vive dentro de su prefijo.

Guardar es atómico (archivo nuevo y renombrar): si se corta la luz, no se pierde la biblioteca.
"""
from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import time
import unicodedata
from pathlib import Path

from . import pe, rutas

EXTENSIONES = (".exe", ".msi", ".bat", ".lnk")
# Lo que deja un instalador y no es el programa: desinstaladores, otros instaladores, ayudantes.
NO_PROGRAMA = re.compile(
    r"(unins|uninst|setup|install|redist|vcredist|vc_redist|directx|dxsetup|dotnet|ndp\d|crash|report|"
    r"helper|update|updater|elevate|launcher_helper|cefsharp|ue4prereq|prereq|easyanticheat_setup|"
    r"notification_helper|unity ?crash)", re.I)


class ErrorBiblioteca(Exception):
    """Algo que se le puede decir tal cual a la persona."""


def _slug(texto: str) -> str:
    t = unicodedata.normalize("NFKD", texto).encode("ascii", "ignore").decode()
    t = re.sub(r"[^a-zA-Z0-9]+", "-", t).strip("-").lower()
    return t[:32] or "programa"


def _nuevo_id(nombre: str, exe: str) -> str:
    h = hashlib.sha1(f"{exe}{time.time_ns()}".encode()).hexdigest()[:6]
    return f"{_slug(nombre)}-{h}"


def parece_instalador(ruta: str | Path) -> bool:
    n = Path(ruta).name.lower()
    return n.endswith(".msi") or bool(re.search(r"(setup|install|instal)", n))


def cargar() -> list[dict]:
    try:
        datos = json.loads(rutas.biblioteca().read_text(encoding="utf-8"))
        return [e for e in datos.get("programas", []) if isinstance(e, dict) and e.get("id")]
    except (OSError, ValueError, AttributeError):
        return []


def guardar(programas: list[dict]) -> None:
    archivo = rutas.biblioteca()
    archivo.parent.mkdir(parents=True, exist_ok=True)
    tmp = archivo.with_suffix(".tmp")
    tmp.write_text(json.dumps({"version": 1, "programas": programas}, ensure_ascii=False, indent=1), encoding="utf-8")
    os.replace(tmp, archivo)


def por_id(pid: str, programas: list[dict] | None = None) -> dict | None:
    return next((e for e in (programas if programas is not None else cargar()) if e["id"] == pid), None)


def por_exe(exe: str | Path, programas: list[dict] | None = None) -> dict | None:
    real = str(Path(exe).expanduser().resolve())
    return next((e for e in (programas if programas is not None else cargar())
                 if str(Path(e["exe"]).resolve()) == real), None)


def nuevo_prefijo(nombre: str) -> Path:
    p = rutas.prefijos() / _nuevo_id(nombre, "")
    p.mkdir(parents=True, exist_ok=True)
    return p


def agregar(exe: str | Path, nombre: str | None = None, prefijo: str | Path | None = None) -> dict:
    """Agrega un .exe (o .msi, .bat) a la biblioteca, con su nombre e ícono. Si ya estaba, lo devuelve."""
    exe = Path(exe).expanduser()
    if not exe.is_file():
        raise ErrorBiblioteca(f"No encuentro {exe.name}.")
    if exe.suffix.lower() not in EXTENSIONES:
        raise ErrorBiblioteca(f"{exe.name} no es un programa de Windows (.exe, .msi o .bat).")
    programas = cargar()
    ya = por_exe(exe, programas)
    if ya:
        return ya
    nombre = (nombre or pe.nombre(exe)).strip() or exe.stem
    pid = _nuevo_id(nombre, str(exe))
    icono = pe.guardar_icono(exe, rutas.iconos() / pid)
    if prefijo is None:
        prefijo = rutas.prefijos() / pid
    Path(prefijo).mkdir(parents=True, exist_ok=True)
    entrada = {"id": pid, "nombre": nombre, "exe": str(exe.resolve()), "prefijo": str(prefijo), "proton": "",
               "argumentos": "", "icono": str(icono) if icono else "", "agregado": int(time.time()),
               "ultima_vez": 0, "minutos": 0, "lanzador": False}
    programas.append(entrada)
    guardar(programas)
    return entrada


CAMPOS_EDITABLES = {"nombre", "proton", "argumentos", "exe"}


def cambiar(pid: str, **campos) -> dict:
    programas = cargar()
    e = por_id(pid, programas)
    if e is None:
        raise ErrorBiblioteca("Ese programa ya no está en tu biblioteca.")
    for k, v in campos.items():
        if k in CAMPOS_EDITABLES:
            e[k] = str(v).strip() if k != "nombre" else (str(v).strip() or e["nombre"])
    guardar(programas)
    if e.get("lanzador"):
        crear_lanzador(e)
    return e


def sumar_tiempo(pid: str, segundos: float, inicio: float) -> None:
    programas = cargar()
    e = por_id(pid, programas)
    if e is None:
        return
    e["ultima_vez"] = int(inicio)
    e["minutos"] = int(e.get("minutos", 0)) + max(0, int(round(segundos / 60)))
    guardar(programas)


def quitar(pid: str, borrar_prefijo: bool = False) -> None:
    """Lo saca de la biblioteca. Con borrar_prefijo, también su prefijo (lo instalado y sus partidas)."""
    programas = cargar()
    e = por_id(pid, programas)
    if e is None:
        return
    quitar_lanzador(e)
    otros = [x for x in programas if x["id"] != pid]
    guardar(otros)
    if e.get("icono"):
        Path(e["icono"]).unlink(missing_ok=True)
    pref = Path(e["prefijo"])
    # Solo se borra si es nuestro (dentro de prefijos/) y nadie más lo usa
    if borrar_prefijo and rutas.prefijos() in pref.parents and not any(x["prefijo"] == e["prefijo"] for x in otros):
        shutil.rmtree(pref, ignore_errors=True)


# ---- después de un instalador ------------------------------------------------------------------

def exes_en(prefijo: str | Path) -> set[str]:
    raiz = Path(prefijo) / "drive_c"
    salida: set[str] = set()
    if not raiz.is_dir():
        return salida
    for carpeta, dirs, archivos in os.walk(raiz):
        # El Windows de mentira de Proton no es tuyo
        if carpeta == str(raiz):
            dirs[:] = [d for d in dirs if d.lower() not in ("windows", "programdata")]
        for a in archivos:
            if a.lower().endswith(".exe"):
                salida.add(os.path.join(carpeta, a))
    return salida


def exes_nuevos(prefijo: str | Path, antes: set[str]) -> list[dict]:
    """Los .exe que dejó un instalador, el programa más probable primero: [{exe, nombre, tam}]."""
    salida = []
    for ruta in exes_en(prefijo) - antes:
        if NO_PROGRAMA.search(Path(ruta).name):
            continue
        try:
            tam = os.path.getsize(ruta)
        except OSError:
            continue
        salida.append({"exe": ruta, "nombre": pe.nombre(ruta), "tam": tam})
    salida.sort(key=lambda x: (-("program files" in x["exe"].lower()), -x["tam"]))
    return salida


# ---- lanzador (.desktop) -----------------------------------------------------------------------

def _desktop(e: dict) -> Path:
    return rutas.LANZADORES / f"vortex-runner-{e['id']}.desktop"


def _comando() -> str:
    """El lanzador de Runner con su ruta completa (entre comillas por si tiene espacios), para que el
    acceso funcione aunque ~/.local/bin no esté en el PATH del lanzador de apps."""
    junto = Path(__file__).resolve().parent.parent / "vortex-runner"
    return f'"{junto}"' if junto.is_file() else "vortex-runner"


def crear_lanzador(e: dict) -> Path:
    """Un acceso en el lanzador de apps: lo abre con Vortexine's Runner, con su Proton y su prefijo."""
    rutas.LANZADORES.mkdir(parents=True, exist_ok=True)
    nombre = e["nombre"].replace("\n", " ")
    texto = (
        "[Desktop Entry]\nType=Application\n"
        f"Name={nombre}\nComment=Programa de Windows (Vortexine's Runner)\n"
        f"Exec={_comando()} --correr {e['id']}\n"
        f"Icon={e.get('icono') or 'vortex-runner'}\n"
        "Terminal=false\nCategories=Game;\nStartupNotify=false\n"
    )
    destino = _desktop(e)
    destino.write_text(texto, encoding="utf-8")
    if not e.get("lanzador"):
        programas = cargar()
        x = por_id(e["id"], programas)
        if x is not None:
            x["lanzador"] = True
            guardar(programas)
        e["lanzador"] = True
    return destino


def quitar_lanzador(e: dict) -> None:
    _desktop(e).unlink(missing_ok=True)
    if e.get("lanzador"):
        programas = cargar()
        x = por_id(e["id"], programas)
        if x is not None:
            x["lanzador"] = False
            guardar(programas)
        e["lanzador"] = False
