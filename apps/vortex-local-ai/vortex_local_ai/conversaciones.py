"""Los chats guardados: un JSON por conversación en DATOS/chats/<id>.json.

Una conversación:
    {"id", "titulo", "modelo", "creado", "modificado",
     "mensajes": [{"role": "user"|"assistant"|"system", "content", …opcionales: "pensamiento", "citas",
                   "modelo", "fecha"}],
     "vault": bool}   # opcional: True si fue «Preguntar a mi vault»
"""
from __future__ import annotations

import json
import os
import re
import secrets
import tempfile
import time
from datetime import datetime
from pathlib import Path

from . import rutas

_ID = re.compile(r"^[A-Za-z0-9_-]{1,80}$")


def _carpeta() -> Path:
    carpeta = rutas.chats()
    carpeta.mkdir(parents=True, exist_ok=True)
    return carpeta


def _archivo(id: str) -> Path:
    if not isinstance(id, str) or not _ID.match(id):
        raise ValueError(f"«{id}» no es un id de chat válido.")
    return _carpeta() / f"{id}.json"


def _nuevo_id() -> str:
    return time.strftime("%Y%m%d-%H%M%S") + "-" + secrets.token_hex(3)


def nueva(modelo: str = "", titulo: str = "") -> dict:
    """Una conversación vacía (todavía sin guardar)."""
    ahora = time.time()
    return {"id": _nuevo_id(), "titulo": titulo, "modelo": modelo, "creado": ahora, "modificado": ahora,
            "mensajes": []}


def lista() -> list[dict]:
    """Los chats guardados, el más reciente primero, sin los mensajes (con «n_mensajes»)."""
    salida = []
    for archivo in _carpeta().glob("*.json"):
        try:
            conv = json.loads(archivo.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if not isinstance(conv, dict):
            continue
        mensajes = conv.get("mensajes") or []
        resumen = {k: v for k, v in conv.items() if k != "mensajes"}
        resumen["id"] = archivo.stem
        resumen.setdefault("titulo", "Chat")
        resumen.setdefault("modificado", archivo.stat().st_mtime)
        resumen["n_mensajes"] = sum(1 for m in mensajes if isinstance(m, dict) and m.get("role") != "system")
        salida.append(resumen)
    salida.sort(key=lambda c: c.get("modificado") or 0, reverse=True)
    return salida


def abrir(id: str) -> dict | None:
    """La conversación completa, o None si no existe (o el archivo está roto)."""
    try:
        conv = json.loads(_archivo(id).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(conv, dict):
        return None
    conv["id"] = id
    conv.setdefault("mensajes", [])
    return conv


def guardar(conv: dict) -> str:
    """Guarda la conversación (escritura atómica) y regresa su id. Le pone id, fechas y título si faltan."""
    conv.setdefault("mensajes", [])
    if not conv.get("id"):
        conv["id"] = _nuevo_id()
    ahora = time.time()
    conv.setdefault("creado", ahora)
    conv["modificado"] = ahora
    if not conv.get("titulo"):
        primero = next((m.get("content", "") for m in conv["mensajes"]
                        if isinstance(m, dict) and m.get("role") == "user"), "")
        conv["titulo"] = titulo_automatico(primero)
    destino = _archivo(conv["id"])
    fd, temporal = tempfile.mkstemp(prefix=".", suffix=".tmp", dir=destino.parent)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(conv, f, ensure_ascii=False, indent=1)
        os.replace(temporal, destino)
    except BaseException:
        try:
            os.unlink(temporal)
        except OSError:
            pass
        raise
    return conv["id"]


def borrar(id: str) -> None:
    try:
        _archivo(id).unlink()
    except FileNotFoundError:
        pass


def titulo_automatico(primer_mensaje: str) -> str:
    """Un título corto con las primeras palabras del primer mensaje («Chat nuevo» si no hay)."""
    texto = re.sub(r"```.*?(```|$)", " ", primer_mensaje or "", flags=re.S)
    texto = re.sub(r"!?\[\[([^\]|]*\|)?([^\]]*)\]\]", r"\2", texto)    # [[nota|texto]] → texto
    texto = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", texto)             # [texto](url) → texto
    texto = re.sub(r"[#*_`>~]+", " ", texto)
    texto = " ".join(texto.split())
    if not texto:
        return "Chat nuevo"
    palabras = texto.split(" ")
    titulo = " ".join(palabras[:6])
    if len(titulo) > 48:
        titulo = titulo[:48].rsplit(" ", 1)[0] or titulo[:48]
    titulo = titulo.rstrip(" ,.;:¿?¡!-")
    if len(palabras) > 6 or len(titulo) < len(texto.rstrip(" ,.;:¿?¡!-")):
        titulo += "…"
    titulo = titulo.lstrip("¿¡ ")
    return (titulo[:1].upper() + titulo[1:]) if titulo else "Chat nuevo"


def _fecha(segundos) -> str:
    try:
        return datetime.fromtimestamp(float(segundos)).strftime("%Y-%m-%d %H:%M")
    except (TypeError, ValueError, OSError):
        return ""


def _cita(texto: str) -> str:
    return "\n".join("> " + linea if linea else ">" for linea in texto.strip().split("\n"))


def a_nota(conv: dict) -> str:
    """El chat en Markdown para guardarlo en el vault (con frontmatter y el pensamiento plegado)."""
    titulo = conv.get("titulo") or "Chat"
    modelo = conv.get("modelo") or ""
    lineas = ["---", "tags: [chat, vortex-local-ai]"]
    if modelo:
        lineas.append(f'modelo: "{modelo}"')
    fecha = _fecha(conv.get("creado"))
    if fecha:
        lineas.append(f"fecha: {fecha}")
    lineas += ["---", "", f"# {titulo}", ""]
    for m in conv.get("mensajes") or []:
        if not isinstance(m, dict) or m.get("role") == "system":
            continue
        if m.get("role") == "user":
            lineas += ["## Tú", ""]
        else:
            quien = m.get("modelo") or modelo
            lineas += [f"## IA ({quien})" if quien else "## IA", ""]
        if m.get("pensamiento"):
            lineas += ["> [!note]- Pensamiento", _cita(m["pensamiento"]), ""]
        lineas += [(m.get("content") or "").strip(), ""]
        citas = [c for c in m.get("citas") or [] if c]
        if citas:
            enlaces = ", ".join(f"[[{Path(str(c)).stem}]]" for c in citas)
            lineas += [f"*Fuentes: {enlaces}*", ""]
    return "\n".join(lineas).rstrip() + "\n"
