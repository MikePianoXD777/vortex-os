#!/usr/bin/env python3
"""Prueba de las acciones de Miku (acciones_mcp.py) y de su permiso en la ventana (asistente.py).

1. El servidor MCP con un «botón» falso (un socket que contesta sí o no) y programas falsos en el PATH
   (flatpak, xdg-open, wl-copy, wpctl…): no instala, abre ni cambia nada de verdad.
2. El permiso en el asistente (Qt, sin ventanas): la petición pone los botones Autorizar/Cancelar, el
   clic contesta, el tiempo agotado y una pregunta nueva cancelan.

  python3 -I apps/miku/pruebas/acciones.py
"""

import json
import os
import socket
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))
FALLOS = 0


def probar(nombre: str, ok: bool, extra: str = "") -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}" + (f"   [{extra[:300]}]" if not ok and extra else ""))
    FALLOS += 0 if ok else 1


class Boton:
    """El permiso falso: guarda lo que se pidió y contesta lo que diga `responder`."""

    def __init__(self, ruta: Path):
        self.ruta, self.responder, self.pedidos = ruta, True, []
        self.s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.s.bind(str(ruta))
        self.s.listen(4)
        threading.Thread(target=self._bucle, daemon=True).start()

    def _bucle(self):
        while True:
            c, _ = self.s.accept()
            with c:
                m = json.loads(c.makefile("r").readline())
                self.pedidos.append(m)
                c.sendall(json.dumps({"ok": self.responder}).encode() + b"\n")
                c.recv(1)                      # espera a que la acción termine y cierre


def mcp(env: dict, nombre: str, args: dict) -> str:
    entrada = "\n".join(json.dumps(m) for m in [
        {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
        {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": nombre, "arguments": args}},
    ]) + "\n"
    r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/acciones_mcp.py")], input=entrada, text=True,
                       capture_output=True, env=env, timeout=60)
    resp = [json.loads(l) for l in r.stdout.splitlines() if l.strip()][-1]
    return "\n".join(c.get("text", "") for c in resp.get("result", {}).get("content", []))


def falsos(bin_: Path, log: Path) -> None:
    for prog, extra in {
        "flatpak": 'case "$1" in search) printf "com.valvesoftware.Steam\\tSteam\\n";; list) printf "org.gimp.GIMP\\tGIMP\\n";; esac',
        "xdg-open": "", "wl-copy": 'cat > "$LOG.copiado"', "wpctl": "", "nmcli": "", "notify-send": "",
        "xdg-mime": 'echo ""', "gio": "", "xdg-user-dir": 'echo "$HOME/Descargas"',
    }.items():
        (bin_ / prog).write_text(f'#!/bin/sh\necho "{prog} $*" >> "$LOG"\n{extra}\n')
        (bin_ / prog).chmod(0o755)


def parte_mcp() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="vortex-miku-acciones."))
    (tmp / "bin").mkdir()
    (tmp / "home/Descargas").mkdir(parents=True)
    log = tmp / "log"
    falsos(tmp / "bin", log)
    boton = Boton(tmp / "autorizar.sock")
    env = {"PATH": f"{tmp / 'bin'}:/usr/bin:/bin", "HOME": str(tmp / "home"), "LOG": str(log),
           "VORTEX_MIKU_AUTORIZAR": str(boton.ruta), "XDG_DATA_DIRS": str(tmp / "nada")}

    def leer_log():
        return log.read_text() if log.exists() else ""

    print("== Acciones (MCP) con el permiso falso")
    r = mcp(env, "correr_comando", {"comando": "echo hola-miku", "para_que": "saludar"})
    probar("comando autorizado: corre y devuelve su salida", "hola-miku" in r and "código 0" in r, r)
    probar("el permiso muestra el comando exacto", boton.pedidos[-1]["detalle"].endswith("$ echo hola-miku"), str(boton.pedidos[-1]))
    boton.responder = False
    r = mcp(env, "correr_comando", {"comando": f"touch {tmp}/no-debe-existir"})
    probar("comando cancelado: no corre nada", "NO lo autorizó" in r and not (tmp / "no-debe-existir").exists(), r)
    n = len(boton.pedidos)
    r = mcp(env, "correr_comando", {"comando": "sudo pacman -Syu"})
    probar("con sudo: lo rechaza sin siquiera pedir permiso", "sudo" in r and len(boton.pedidos) == n, r)
    boton.responder = True
    r = mcp(env, "instalar_app", {"nombre": "Steam"})
    probar("instalar: pide permiso con el nombre y el ID de Flathub", boton.pedidos[-1]["titulo"] == "Instalar «Steam»"
           and "com.valvesoftware.Steam" in boton.pedidos[-1]["detalle"] and "Empecé a instalar Steam" in r, r)
    r = mcp(env, "quitar_app", {"nombre": "gimp"})
    probar("quitar: desinstala para el usuario", "quité GIMP" in r and "flatpak uninstall --user -y --noninteractive org.gimp.GIMP" in leer_log(), r)
    r = mcp(env, "abrir", {"que": "https://vortexine.example/ayuda"})
    probar("abrir una página: xdg-open con la URL", "Abrí https://vortexine.example/ayuda" in r, r)
    r = mcp(env, "abrir", {"que": "descargas"})
    probar("abrir Descargas", str(tmp / "home/Descargas") in r, r)
    r = mcp(env, "ajuste", {"tipo": "volumen", "valor": "35"})
    probar("ajuste de volumen", "wpctl set-volume -l 1.5 @DEFAULT_AUDIO_SINK@ 35%" in leer_log() and "Listo" in r, r)
    r = mcp(env, "ajuste", {"tipo": "wifi", "valor": "apagar"})
    probar("apagar el Wi-Fi", "nmcli radio wifi off" in leer_log(), r)
    r = mcp(env, "redactar_correo", {"para": "papa@example.com", "asunto": "Hola", "cuerpo": "¿Cenamos hoy?"})
    probar("correo: solo borrador (Gmail web sin app de correo) y lo dice", "No se envió nada" in r
           and "mail.google.com" in leer_log() and "papa%40example.com" in leer_log(), r)
    probar("el permiso del correo muestra para, asunto y texto", "papa@example.com" in boton.pedidos[-1]["detalle"]
           and "¿Cenamos hoy?" in boton.pedidos[-1]["detalle"])
    r = mcp(env, "redactar_mensaje", {"app": "whatsapp", "para": "Irving", "texto": "Ya llegué"})
    copiado = Path(str(log) + ".copiado")
    probar("mensaje: lo copia y abre WhatsApp web, no lo envía", copiado.exists() and copiado.read_text() == "Ya llegué"
           and "https://web.whatsapp.com" in leer_log() and "No se envió nada" in r, r)
    r = mcp({**env, "VORTEX_MIKU_AUTORIZAR": ""}, "ajuste", {"tipo": "volumen", "valor": "10"})
    probar("sin la ventana de Miku: no hace nada", "no hice nada" in r and "set-volume @DEFAULT_AUDIO_SINK@ 10%" not in leer_log(), r)
    subprocess.run(["rm", "-rf", str(tmp)])


def hook(env: dict, datos: dict) -> dict:
    r = subprocess.run([sys.executable, "-I", str(RAIZ / "miku/permiso_web.py")], input=json.dumps(datos), text=True,
                       capture_output=True, env=env, timeout=60)
    return json.loads(r.stdout or "{}").get("hookSpecificOutput", {})


def parte_web() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="vortex-miku-web."))
    boton = Boton(tmp / "autorizar.sock")
    env = {"PATH": "/usr/bin:/bin", "VORTEX_MIKU_AUTORIZAR": str(boton.ruta)}
    print("== Internet: el hook del permiso")
    r = hook(env, {"tool_name": "WebSearch", "tool_input": {"query": "versión de Python"}})
    probar("búsqueda autorizada: allow, y el permiso muestra la búsqueda exacta", r.get("permissionDecision") == "allow"
           and boton.pedidos[-1] == {"tipo": "pedir", "titulo": "Buscar en internet", "detalle": "Búsqueda: versión de Python"}, str(r))
    boton.responder = False
    r = hook(env, {"tool_name": "WebFetch", "tool_input": {"url": "https://ejemplo.org/x?d=secreto", "prompt": "lee"}})
    probar("página cancelada: deny, y el permiso muestra la dirección completa", r.get("permissionDecision") == "deny"
           and boton.pedidos[-1]["detalle"] == "https://ejemplo.org/x?d=secreto", str(r))
    r = hook({**env, "VORTEX_MIKU_AUTORIZAR": ""}, {"tool_name": "WebSearch", "tool_input": {"query": "x"}})
    probar("sin la ventana de Miku: deny", r.get("permissionDecision") == "deny", str(r))
    r = hook(env, {"tool_name": "Bash", "tool_input": {"command": "id"}})
    probar("otra herramienta: deny sin pedir permiso", r.get("permissionDecision") == "deny", str(r))

    print("== Internet: el comando de Claude")
    from miku import cerebro
    con = cerebro.construir_comando("claude", "p", "hola", None, None, str(boton.ruta), True)
    sin = cerebro.construir_comando("claude", "p", "hola", None, None, str(boton.ruta), False)
    sin_ventana = cerebro.construir_comando("claude", "p", "hola", None, None, None, True)
    tools = lambda c: c[c.index("--tools") + 1]
    probar("con internet: WebSearch y WebFetch, con el hook del permiso", tools(con) == "WebSearch,WebFetch"
           and "permiso_web.py" in con[con.index("--settings") + 1] and "WebFetch" in con[con.index("--allowedTools"):con.index("--disallowedTools")])
    probar("sin internet: ninguna herramienta propia de Claude", tools(sin) == "" and "--settings" not in sin
           and "WebFetch" in sin[sin.index("--disallowedTools"):])
    probar("sin la ventana para pedir permiso, nada de internet", tools(sin_ventana) == "" and "--settings" not in sin_ventana)

    print("== Internet: la casilla del instalador")
    from miku import config, rutas
    marca, cfg = tmp / "miku-internet", tmp / "miku.json"
    rutas.MARCA_INTERNET = marca
    probar("sin marca y sin elegir nada: apagado", config.cargar(cfg)["internet"] is False)
    marca.write_text("x")
    probar("con la marca del instalador: encendido", config.cargar(cfg)["internet"] is True)
    cfg.write_text('{"internet": false}')
    probar("lo que elijas en miku.json gana a la marca", config.cargar(cfg)["internet"] is False)
    subprocess.run(["rm", "-rf", str(tmp)])


def parte_asistente() -> None:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    from PySide6.QtCore import QCoreApplication, QTimer
    from miku import asistente
    app = QCoreApplication.instance() or QCoreApplication(sys.argv)
    print("== El permiso en la ventana (asistente)")
    a = asistente.Asistente({"avatar": False, "voz": "supertonic-f1", "modelo": "", "seguir_escuchando": False,
                             "silencio": 1.5}, avatar=False)
    botones, avisos = [], []
    a.acciones.connect(lambda b: botones.append(b))
    a.aviso.connect(avisos.append)
    ruta = a.cerebro.autorizar
    probar("el asistente abre su socket de permisos y se lo da a Claude", bool(ruta) and Path(ruta).exists(), str(ruta))

    def pedir(titulo: str, detalle: str, accion: str | None, esperar_ms: int = 600):
        resultado = {}

        def cliente():
            s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            s.connect(ruta)
            s.sendall(json.dumps({"tipo": "pedir", "titulo": titulo, "detalle": detalle}).encode() + b"\n")
            resultado["r"] = json.loads(s.makefile("r").readline() or "{}")
            s.close()
        h = threading.Thread(target=cliente, daemon=True)
        h.start()
        if accion:
            QTimer.singleShot(esperar_ms, lambda: a.accion(accion))
        QTimer.singleShot(esperar_ms + 700, app.quit)
        app.exec()
        h.join(2)
        return resultado.get("r", {})

    r = pedir("Correr un comando", "$ echo hola", "autorizar")
    probar("muestra Autorizar y Cancelar con lo que va a hacer", ["autorizar", "Autorizar"] in (botones[-2] if len(botones) > 1 else [])
           and any("Correr un comando" in t and "$ echo hola" in t for t in avisos), str(botones[-3:]))
    probar("Autorizar contesta que sí y quita los botones", r.get("ok") is True and botones[-1] == [], str(r))
    r = pedir("Instalar «Steam»", "com.valvesoftware.Steam", "cancelar")
    probar("Cancelar contesta que no", r.get("ok") is False and any("Cancelado" in t for t in avisos), str(r))
    asistente.PERMISO_SEG = 1
    a._permiso_reloj.setInterval(800)
    r = pedir("Abrir «Steam»", "la app", None, esperar_ms=1500)
    probar("sin respuesta se cancela sola", r.get("ok") is False and any("Se acabó el tiempo" in t for t in avisos), str(r))
    a._permiso_reloj.setInterval(asistente.PERMISO_SEG * 1000 * 60)
    r = pedir("Subir el volumen", "$ wpctl set-volume … 30%", "autorizar_todo")
    probar("muestra también «Autorizar todo»", any(["autorizar_todo", "Autorizar todo"] in b for b in botones), str(botones[-3:]))
    probar("«Autorizar todo» autoriza esta acción", r.get("ok") is True, str(r))
    n = len(botones)
    r = pedir("Abrir «Vortex Studio»", "la app", None, esperar_ms=300)
    probar("…y las siguientes pasan sin preguntar (sin botones), diciendo qué hizo", r.get("ok") is True
           and not any(b for b in botones[n:]) and any("sin preguntar" in t and "Vortex Studio" in t for t in avisos), str(r))
    a.cerrar_todo()
    probar("al cerrar a Miku se borra el socket", not Path(ruta).exists())


def main() -> int:
    parte_mcp()
    parte_web()
    parte_asistente()
    print("\nTodo bien" if not FALLOS else f"\nFALLARON {FALLOS} pruebas")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
