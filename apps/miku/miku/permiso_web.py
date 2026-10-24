#!/usr/bin/env python3
"""Hook de Claude Code (PreToolUse) para la búsqueda web y la lectura de páginas de Miku.

Claude Code lo corre antes de cada WebSearch o WebFetch (se lo pasa cerebro.py con --settings). Pide
permiso en la ventana de Miku por el mismo socket que las acciones (VORTEX_MIKU_AUTORIZAR), mostrando la
búsqueda o la dirección exacta: así una página no puede hacer que Miku mande tus datos a otra dirección a
escondidas. Contesta a Claude Code «allow» solo con tu Autorizar; con Cancelar, sin respuesta o si algo
falla, «deny».

Entra (stdin): {"tool_name": "WebSearch", "tool_input": {"query": ...}} o WebFetch con {"url", "prompt"}.
Sale (stdout): {"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": ..., ...}}
"""

import json
import os
import socket
import sys

ESPERA = 130


def decidir(permitir: bool, motivo: str) -> None:
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse",
                                             "permissionDecision": "allow" if permitir else "deny",
                                             "permissionDecisionReason": motivo}}, ensure_ascii=False))
    sys.exit(0)


def pedir(titulo: str, detalle: str) -> bool:
    ruta = os.environ.get("VORTEX_MIKU_AUTORIZAR", "")
    if not ruta:
        return False
    try:
        with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
            s.settimeout(ESPERA)
            s.connect(ruta)
            s.sendall((json.dumps({"tipo": "pedir", "titulo": titulo, "detalle": detalle}, ensure_ascii=False) + "\n").encode())
            return bool(json.loads(s.makefile("r", encoding="utf-8").readline() or "{}").get("ok"))
    except (OSError, ValueError):
        return False


def main() -> None:
    try:
        datos = json.load(sys.stdin)
    except ValueError:
        decidir(False, "No entendí qué quería usar Claude.")
    herramienta = datos.get("tool_name", "")
    entrada = datos.get("tool_input") or {}
    if herramienta == "WebSearch":
        consulta = str(entrada.get("query", "")).strip()
        titulo, detalle = "Buscar en internet", f"Búsqueda: {consulta}"
    elif herramienta == "WebFetch":
        url = str(entrada.get("url", "")).strip()
        titulo, detalle = "Leer una página web", url
    else:
        decidir(False, f"Miku no usa {herramienta}.")
    if pedir(titulo, detalle):
        decidir(True, "La persona lo autorizó en la ventana de Miku.")
    decidir(False, "La persona NO lo autorizó (o no contestó). No lo intentes de otra forma.")


if __name__ == "__main__":
    main()
