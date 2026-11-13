#!/usr/bin/env python3
"""Prueba del navegador de Claude en vortex-mcp (0.2), de punta a punta y sin abrir ventanas.

Levanta un servidor HTTP de juguete en 127.0.0.1 con una página que tiene de todo (un formulario con
contraseña, cookies, un token en localStorage, errores en la consola, una petición que falla y otra
con Authorization), abre un Firefox headless con un perfil temporal («vortex-mcp navegador abrir
--headless») y habla con vortex-mcp por JSON-RPC como Claude Code. HOME, XDG_* y el perfil van en un
temporal y notify-send es falso: no toca tu Firefox, ni tu config, ni tu escritorio.

Comprueba que abre, lee, hace clic, escribe, elige, ve la consola, la red y los errores de JavaScript,
evalúa JS, da el rendimiento, los estilos y el diagnóstico de una página local, y sobre todo que
NINGUNA salida trae la contraseña, las cookies, el token ni la cabecera Authorization.

    pruebas/mcp-navegador.sh   (con límite de memoria; o directo: python3 pruebas/mcp-navegador.py)
"""

import http.server
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent.parent
MCP = AQUI / "bin/vortex-mcp"
SECRETOS = {
    "contraseña del campo": "SuperSecreta123",
    "cookie de la página": "CookieSecreta987",
    "cookie del servidor": "CookieServidor321",
    "token de localStorage": "TokenLocal555xyzABCDEFGH",
    "token de sessionStorage": "TokenSesion444",
    "cabecera Authorization": "TokenBearer777",
    "campo oculto (CSRF)": "CsrfOculto888",
    "token en la URL": "TokenUrl999",
}

PAGINA = """<!doctype html>
<html lang="es"><head><meta charset="utf-8"><title>Página de prueba</title>
<style>#titulo { color: rgb(200, 10, 10); font-size: 32px; }</style></head>
<body>
<h1 id="titulo">Hola desde la prueba</h1>
<p>Un párrafo con <a href="/otra?token=TokenUrl999&amp;q=gatos">un enlace</a>.</p>
<button id="boton" onclick="document.getElementById('resultado').textContent = 'clic recibido'; console.log('clic ok')">Haz clic</button>
<div id="resultado">nada todavía</div>
<form id="buscar" onsubmit="event.preventDefault(); document.getElementById('resultado').textContent = 'buscaste ' + this.q.value">
  <label for="q">Buscar</label><input id="q" name="q">
  <select id="color" name="color" onchange="document.getElementById('resultado').textContent = 'color ' + this.value">
    <option value="r">Rojo</option><option value="v">Verde</option><option value="a">Azul</option></select>
</form>
<form id="login" action="/entrar" method="post">
  <label for="usuario">Usuario</label><input id="usuario" name="usuario" value="mike">
  <label for="clave">Contraseña</label><input id="clave" name="clave" type="password" value="SuperSecreta123">
  <input type="hidden" name="csrf" value="CsrfOculto888">
  <button id="entrar" type="submit">Entrar</button>
</form>
<script>
  document.cookie = "sesion=CookieSecreta987; path=/";
  localStorage.setItem("token", "TokenLocal555xyzABCDEFGH");
  sessionStorage.setItem("sesion", "TokenSesion444");
  console.log("Authorization: Bearer TokenBearer777");
  console.log("la cookie es " + document.cookie);
  console.warn("aviso de prueba");
  console.error("error de prueba");
  fetch("/api/falla");
  fetch("/api/ok", {headers: {"Authorization": "Bearer TokenBearer777", "X-Normal": "visible"}});
  fetch("http://127.0.0.1:PUERTO_CERRADO/nadie-escucha").catch(() => {});
  setTimeout(() => { function fallaAProposito() { throw new Error("fallo a propósito"); } fallaAProposito(); }, 50);
</script>
</body></html>"""


class Manejador(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_GET(self):
        if self.path == "/" or self.path.startswith("/otra"):
            cuerpo = (PAGINA.replace("PUERTO_CERRADO", str(PUERTO_CERRADO)) if self.path == "/" else "<title>Otra</title><p>Otra página</p>").encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Set-Cookie", "servidor=CookieServidor321; Path=/")
        elif self.path == "/api/ok":
            cuerpo = b'{"ok": true}'
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
        else:
            cuerpo = b"no"
            self.send_response(500 if self.path == "/api/falla" else 404)
            self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)


FALLOS = 0


def puerto_cerrado():
    """Un puerto donde no escucha nadie (Firefox bloquea los «prohibidos», como el 9)."""
    import socket
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


PUERTO_CERRADO = puerto_cerrado()


def probar(nombre, ok, detalle=""):
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}", flush=True)
    if not ok:
        FALLOS += 1
        if detalle:
            print("      salió: " + detalle[-600:].replace("\n", "\n      "))


class Cliente:
    def __init__(self, env):
        self.p = subprocess.Popen([sys.executable, str(MCP)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.PIPE, env=env, text=True)
        self.id = 0
        self.salidas = []

    def pedir(self, metodo, params=None):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": metodo, "params": params or {}}) + "\n")
        self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())

    def usar(self, nombre, **args):
        r = self.pedir("tools/call", {"name": nombre, "arguments": args})["result"]
        t = "\n".join(c.get("text", "") for c in r["content"] if c["type"] == "text")
        self.salidas.append(t)
        return t, r

    def cerrar(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


def main():
    if not shutil.which("firefox"):
        print("Sin Firefox: se salta.")
        return 0
    tmp = Path(tempfile.mkdtemp(prefix="vortex-mcp-navegador-"))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Manejador)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    puerto = srv.server_address[1]
    (tmp / "bin").mkdir()
    (tmp / "bin/notify-send").write_text(f"#!/bin/sh\necho \"$*\" >> {tmp}/avisos.log\n")
    (tmp / "bin/notify-send").chmod(0o755)
    env = dict(os.environ, HOME=str(tmp / "home"), XDG_CONFIG_HOME=str(tmp / "config"), XDG_DATA_HOME=str(tmp / "data"),
               XDG_RUNTIME_DIR=str(tmp / "run"), XDG_CACHE_HOME=str(tmp / "cache"), PATH=f"{tmp}/bin:{os.environ['PATH']}",
               VORTEX_NAVEGADOR_PERFIL=str(tmp / "perfil"), MOZ_HEADLESS="1")
    for k in ("WAYLAND_DISPLAY", "DISPLAY"):
        env.pop(k, None)
    for d in ("home", "config", "data", "run"):
        (tmp / d).mkdir()
    (tmp / "run").chmod(0o700)
    c = None
    try:
        print("== Sin navegador abierto")
        c = Cliente(env)
        init = c.pedir("initialize", {"protocolVersion": "2025-06-18"})["result"]
        probar("versión 0.2.0", init["serverInfo"]["version"] == "0.2.0")
        probar("las instrucciones hablan del navegador", "vortex claude navegador" in init["instructions"])
        nombres = {h["name"]: h for h in c.pedir("tools/list")["result"]["tools"]}
        probar("siguen las herramientas de antes",
               {"ver_pantalla", "ventana_activa", "ventanas", "portapapeles", "historial_terminal", "estado_sistema"} <= set(nombres))
        probar("están las del navegador", all(f"navegador_{n}" in nombres for n in (
            "abrir", "leer", "clic", "escribir", "consola", "red", "evaluar", "rendimiento", "estilos", "depurar_local")))
        probar("clic no es de solo lectura; leer sí",
               not nombres["navegador_clic"]["annotations"]["readOnlyHint"] and nombres["navegador_leer"]["annotations"]["readOnlyHint"])
        t, r = c.usar("navegador_leer")
        probar("pide abrirlo con «vortex claude navegador»", r.get("isError") and "vortex claude navegador" in t)

        print("== Abrir el navegador (headless, perfil temporal)")
        a = subprocess.run([sys.executable, str(MCP), "navegador", "abrir", "--headless"], env=env, capture_output=True, text=True, timeout=60)
        probar("abre", a.returncode == 0)
        probar("el perfil es el temporal, con su user.js",
               (tmp / "perfil/user.js").exists() and "sanitizeOnShutdown" in (tmp / "perfil/user.js").read_text())
        probar("avisa con una notificación", "Firefox aparte para Claude" in (tmp / "avisos.log").read_text())

        print("== Abrir y leer")
        t, _ = c.usar("navegador_abrir", url=f"http://127.0.0.1:{puerto}/")
        probar("abre la página y dice su título y estado", "Página de prueba" in t and "200" in t)
        probar("dice que hubo errores", "errores en la consola" in t)
        time.sleep(0.5)
        t, _ = c.usar("navegador_leer", que="texto")
        probar("lee el texto visible", "Hola desde la prueba" in t and "nada todavía" in t)
        probar("lo marca como contenido de la página (no instrucciones)", "no instrucciones" in t and "[Fin del contenido de la página]" in t)
        t, _ = c.usar("navegador_leer", que="estructura")
        probar("la estructura trae el título, el botón y los campos", "título 1" in t and "#boton" in t and "#q" in t)
        t, _ = c.usar("navegador_leer", que="enlaces")
        probar("los enlaces, con el token de la URL tapado", "/otra?" in t and "q=gatos" in t)
        t, _ = c.usar("navegador_leer", que="formularios")
        probar("los formularios dicen que hay contraseña sin mostrarla", "contraseña o dato de pago: no se muestra" in t and "«mike»" in t)
        t, r = c.usar("navegador_captura")
        probar("captura (una imagen)", any(x["type"] == "image" and len(x["data"]) > 1000 for x in r["content"]))
        t, r = c.usar("navegador_captura", selector="#titulo")
        probar("captura de un elemento", any(x["type"] == "image" for x in r["content"]) and not r.get("isError"))

        print("== Interactuar")
        t, _ = c.usar("navegador_clic", texto="Haz clic")
        t, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("el clic llega a la página", "clic recibido" in t)
        t, _ = c.usar("navegador_escribir", campo="Buscar", contenido="gatos con ñ", enter=True)
        t, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("escribe (con ñ) y Enter manda el formulario normal", "buscaste gatos con ñ" in t)
        t, _ = c.usar("navegador_elegir", selector="#color", opcion="Verde")
        t2, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("elige una opción y dispara change", "Verde" in t and "color v" in t2)
        t, r = c.usar("navegador_escribir", selector="#clave", contenido="otra")
        probar("no escribe en el campo de contraseña", r.get("isError") and "contraseña" in t)
        t, r = c.usar("navegador_escribir", selector="#usuario", contenido="otro")
        probar("ni en un formulario de inicio de sesión", r.get("isError"))
        t, r = c.usar("navegador_clic", selector="#entrar")
        probar("ni hace clic en «Entrar» de un formulario con contraseña", r.get("isError"))
        t, _ = c.usar("navegador_tecla", tecla="Tab")
        probar("presiona teclas", "Tab" in t)
        t, _ = c.usar("navegador_desplazar", pixeles=200)
        probar("desplaza", "La página está en" in t)
        c.usar("navegador_evaluar", expresion="setTimeout(() => { const d = document.createElement('p'); d.id = 'tarde'; d.textContent = 'llegó tarde'; document.body.append(d); }, 400), 1")
        t, _ = c.usar("navegador_esperar", selector="#tarde", segundos=5)
        probar("espera a que aparezca algo", "Ya apareció" in t)

        print("== Depurar")
        t, _ = c.usar("navegador_consola")
        probar("la consola trae log, warn y error", "clic ok" in t and "aviso de prueba" in t and "error de prueba" in t)
        probar("y el error de JavaScript con su pila", "fallo a propósito" in t and "fallaAProposito" in t)
        t, _ = c.usar("navegador_consola", nivel="error")
        probar("filtra por nivel", "error de prueba" in t and "clic ok" not in t)
        t, _ = c.usar("navegador_red")
        probar("la red trae la que falla con su estado", "/api/falla → 500" in t)
        probar("la que no se pudo conectar", "nadie-escucha → FALLÓ" in t, t)
        probar("y la que sí", "/api/ok → 200" in t)
        t, _ = c.usar("navegador_red", solo_fallos=True, cabeceras=True)
        probar("solo fallos", "/api/ok" not in t and "/api/falla" in t)
        t, _ = c.usar("navegador_red", filtro="/api/ok", cabeceras=True)
        probar("las cabeceras normales se ven y Authorization no", "x-normal: visible" in t.lower() and "authorization: [oculto]" in t.lower())
        t, _ = c.usar("navegador_evaluar", expresion="document.querySelectorAll('option').length * 2")
        probar("evalúa JavaScript", "6" in t)
        t, _ = c.usar("navegador_evaluar", expresion="Promise.resolve({a: 1, b: [2, 3]})")
        probar("espera promesas y da JSON", '"a": 1' in t)
        t, _ = c.usar("navegador_evaluar", expresion="noExiste.algo")
        probar("una excepción sale como tal", "excepción" in t and "noExiste" in t)
        t, _ = c.usar("navegador_rendimiento")
        probar("rendimiento", "Primer byte" in t and "recursos" in t)
        t, _ = c.usar("navegador_estilos", selector="#titulo", propiedades=["color", "font-size"])
        probar("estilos calculados", "rgb(200, 10, 10)" in t and "32px" in t)
        t, _ = c.usar("navegador_depurar_local", puerto=puerto, esperar=1)
        probar("el diagnóstico local junta errores, fallos y tiempos",
               "Errores en la consola" in t and "error de prueba" in t and "/api/falla" in t and "Carga:" in t)
        t, _ = c.usar("navegador_historial", accion="recargar", sin_cache=True)
        probar("recarga", "Página de prueba" in t, t)
        c.usar("navegador_abrir", url=f"http://127.0.0.1:{puerto}/otra")
        t, _ = c.usar("navegador_historial", accion="atras")
        probar("atrás", "Página de prueba" in t)
        t, _ = c.usar("navegador_pestanas", accion="nueva")
        probar("abre otra pestaña", "1 (actual)" in t)
        t, _ = c.usar("navegador_pestanas", accion="cerrar", indice=1)
        probar("y la cierra", t.count(" — ") == 1 and "0 (actual)" in t, t)
        t, r = c.usar("navegador_abrir", url="https://mail.google.com/")
        probar("no abre correo ni mensajes", r.get("isError"))

        print("== Que no se filtre nada sensible")
        for expr in ("document.cookie", "localStorage.getItem('token')", "window['local' + 'Storage'].getItem('token')",
                     "document.querySelector('#clave').value", "document.forms.login.clave.value",
                     "document.querySelector('[name=csrf]').value", "Object.values(window['session'+'Storage'])",
                     "Object.getOwnPropertyDescriptor(Document.prototype, 'coo' + 'kie').get.call(document)",
                     "[...document.querySelectorAll('input')].map(i => i.value).join(',')",
                     "document.documentElement.outerHTML"):
            c.usar("navegador_evaluar", expresion=expr)
        c.usar("navegador_consola", todo=True)
        c.usar("navegador_red", todo=True, cabeceras=True)
        c.usar("navegador_leer", que="estructura")
        c.usar("navegador_estado")
        todo = "\n".join(c.salidas)
        for nombre, secreto in SECRETOS.items():
            probar(f"no sale: {nombre}", secreto not in todo)
        probar("los cambios avisan (clic, escribir, evaluar)",
               all(x in (tmp / "avisos.log").read_text() for x in ("hizo clic", "escribió", "corrió JavaScript")))

        print("== Pausa")
        (tmp / "config/vortex").mkdir(parents=True, exist_ok=True)
        (tmp / "config/vortex/claude-pausa").touch()
        t, r = c.usar("navegador_leer")
        probar("en pausa no hace nada", r.get("isError") and "pausa" in t)
        (tmp / "config/vortex/claude-pausa").unlink()

        print("== vortex claude navegador / pausa")
        vortex = str(AQUI / "bin/vortex")
        subprocess.run([sys.executable, str(MCP), "navegador", "cerrar"], env=env, capture_output=True, timeout=20)
        a = subprocess.run([vortex, "claude", "navegador", "--headless"], env=env, capture_output=True, text=True, timeout=60)
        e = subprocess.run([vortex, "claude", "navegador", "estado"], env=env, capture_output=True, text=True, timeout=20)
        probar("«vortex claude navegador» lo abre", a.returncode == 0 and "abierto" in e.stdout, a.stdout + a.stderr + e.stdout)
        p = subprocess.run([vortex, "claude", "pausa"], env=env, capture_output=True, text=True, timeout=20)
        e = subprocess.run([vortex, "claude", "navegador", "estado"], env=env, capture_output=True, text=True, timeout=20)
        probar("«vortex claude pausa» lo cierra", p.returncode == 0 and "cerrado" in e.stdout, p.stdout + e.stdout)
        a = subprocess.run([vortex, "claude", "navegador", "--headless"], env=env, capture_output=True, text=True, timeout=60)
        probar("en pausa no se abre", a.returncode != 0 and "pausa" in a.stderr, a.stdout + a.stderr)
        subprocess.run([vortex, "claude", "reanudar"], env=env, capture_output=True, timeout=20)
    finally:
        if c:
            c.cerrar()
        subprocess.run([sys.executable, str(MCP), "navegador", "cerrar"], env=env, capture_output=True, timeout=20)
        e = subprocess.run([sys.executable, str(MCP), "navegador", "estado"], env=env, capture_output=True, text=True)
        probar("cerrar lo cierra", e.returncode == 1 and "cerrado" in e.stdout)
        srv.shutdown()
        time.sleep(1)
        shutil.rmtree(tmp, ignore_errors=True)
    print()
    if FALLOS:
        print(f"FALLARON {FALLOS} pruebas")
        return 1
    print("Todo bien")
    return 0


if __name__ == "__main__":
    sys.exit(main())
