#!/usr/bin/env python3
"""Prueba del navegador de Claude en vortex-mcp (0.3), de punta a punta y sin abrir ventanas.

Levanta un servidor HTTP de juguete en 127.0.0.1 con una página que tiene de todo (un formulario con
contraseña, cookies, un token en localStorage, errores en la consola, una petición que falla y otra
con Authorization), abre un Firefox headless con un perfil temporal («vortex-mcp navegador abrir
--headless») y habla con vortex-mcp por JSON-RPC como Claude Code. HOME, XDG_* y el perfil van en un
temporal y notify-send es falso: no toca tu Firefox, ni tu config, ni tu escritorio.

Comprueba que abre, lee, hace clic, escribe, elige, ve la consola, la red y los errores de JavaScript,
evalúa JS, da el rendimiento, los estilos y el diagnóstico de una página local, y sobre todo que
NINGUNA salida trae la contraseña, las cookies, el token ni la cabecera Authorization.

Con «brave» de argumento prueba el controlador de Brave/Chromium (CDP). Brave no está instalado, así que
usa Electron (el Chromium que ya trae el sistema) haciéndose pasar por Brave, sin ventanas (ozone headless).

La 0.3 añade los archivos: descargas a ~/Descargas/Claude (las que pide Claude, nunca las de la página),
tipos peligrosos con confirmación, tope de tamaño, subir a un formulario y arrastrando, leer CSV/PDF/
imágenes, extraer zip/tar sin «..» ni enlaces, y nunca subir de carpetas sensibles.

    pruebas/mcp-navegador.sh   (con límite de memoria; o directo: python3 pruebas/mcp-navegador.py [firefox|brave])
"""

import http.server
import io
import json
import struct
import tarfile
import zipfile
import zlib
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


def pdf_simple(linea):
    """Un PDF de una página con una línea de texto (para pdftotext)."""
    flujo = f"BT /F1 18 Tf 20 100 Td ({linea}) Tj ET"
    objs = ["<< /Type /Catalog /Pages 2 0 R >>", "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 144] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>",
            f"<< /Length {len(flujo)} >>\nstream\n{flujo}\nendstream", "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"]
    out, pos = b"%PDF-1.4\n", []
    for i, o in enumerate(objs, 1):
        pos.append(len(out))
        out += f"{i} 0 obj\n{o}\nendobj\n".encode()
    xref = len(out)
    out += f"xref\n0 {len(objs) + 1}\n0000000000 65535 f \n".encode() + "".join(f"{p:010d} 00000 n \n" for p in pos).encode()
    return out + f"trailer\n<< /Size {len(objs) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()


def png(w, h):
    def trozo(tipo, datos):
        return struct.pack(">I", len(datos)) + tipo + datos + struct.pack(">I", zlib.crc32(tipo + datos) & 0xFFFFFFFF)
    crudo = b"".join(b"\x00" + b"\xff\x40\x00" * w for _ in range(h))
    return (b"\x89PNG\r\n\x1a\n" + trozo(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
            + trozo(b"IDAT", zlib.compress(crudo)) + trozo(b"IEND", b""))


def comprimido(tipo, miembros):
    b = io.BytesIO()
    if tipo == "zip":
        with zipfile.ZipFile(b, "w") as z:
            for nombre, datos in miembros:
                z.writestr(nombre, datos)
    else:
        with tarfile.open(fileobj=b, mode="w") as tf:
            for nombre, datos in miembros:
                info = tarfile.TarInfo(nombre)
                if isinstance(datos, tuple):  # («enlace», destino)
                    info.type, info.linkname = tarfile.SYMTYPE, datos[1]
                    tf.addfile(info)
                else:
                    info.size = len(datos)
                    tf.addfile(info, io.BytesIO(datos))
    return b.getvalue()


MAX_PRUEBA = 1_000_000  # VORTEX_CLAUDE_MAX_ARCHIVO en la prueba
ARCHIVOS = {
    "datos.csv": b"nombre,edad\nAna,12\nLuis,13\n",
    "informe.pdf": pdf_simple("Ventas de noviembre: 42 cajas"),
    "instalar.sh": b"#!/bin/sh\necho hola\n",
    "programa": b"\x7fELF\x02\x01\x01" + b"\x00" * 64,
    "otro.run": b"#!/bin/sh\nexit 0\n",
    "grande.dat": b"\x00" * (MAX_PRUEBA * 2),
    "foto.png": png(3, 2),
    "bueno.zip": comprimido("zip", [("a.txt", b"hola zip"), ("sub/b.csv", b"x,y\n1,2\n")]),
    "malo.zip": comprimido("zip", [("bien.txt", b"ok"), ("../fuera.txt", b"me sali")]),
    "enlace.tar": comprimido("tar", [("normal.txt", b"ok"), ("atajo", ("enlace", "/etc/passwd"))]),
}
PAGINA_ARCHIVOS = """<!doctype html><html lang="es"><head><meta charset="utf-8"><title>Archivos</title></head><body>
<a id="csv" href="/dl/datos.csv" download>Tabla CSV</a>
<a id="pdf" href="/dl/informe.pdf">Informe en PDF</a>
<a id="sh" href="/dl/instalar.sh">Instalador</a>
</body></html>"""
PAGINA_AUTO = """<!doctype html><title>Sola</title><p>Esta página intenta bajar algo sola.</p>
<script>setTimeout(() => { location.href = '/dl/datos.csv'; }, 100);</script>"""
PAGINA_SUBIR = """<!doctype html><html lang="es"><head><meta charset="utf-8"><title>Subir</title></head><body>
<form id="f" action="/subida" method="post" enctype="multipart/form-data">
  <label for="archivo">Archivo</label><input type="file" id="archivo" name="archivo">
  <button id="enviar" type="submit">Enviar</button>
</form>
<div id="zona" style="width:300px;height:100px;border:1px dashed">Arrastra aquí</div>
<div id="resultado">nada</div>
<script>
  const z = document.getElementById('zona');
  z.addEventListener('dragover', e => e.preventDefault());
  z.addEventListener('drop', e => { e.preventDefault();
    document.getElementById('resultado').textContent = 'soltaste ' + [...e.dataTransfer.files].map(f => f.name + ' ' + f.size).join(', '); });
</script></body></html>"""
RECIBIDO = []


class Manejador(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def do_POST(self):
        RECIBIDO.append(self.rfile.read(int(self.headers.get("Content-Length") or 0)))
        cuerpo = "<title>Recibido</title><p>Recibido, gracias.</p>".encode()
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)

    def do_GET(self):
        paginas = {"/archivos": PAGINA_ARCHIVOS, "/auto": PAGINA_AUTO, "/subir": PAGINA_SUBIR}
        if self.path in paginas:
            cuerpo = paginas[self.path].encode()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
        elif self.path.startswith("/dl/") and self.path[4:] in ARCHIVOS:
            cuerpo = ARCHIVOS[self.path[4:]]
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", f'attachment; filename="{self.path[4:]}"')
        elif self.path == "/" or self.path.startswith("/otra"):
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
        self.preguntas = []  # lo que el servidor le preguntó a la persona (elicitation)
        self.aceptar = True

    def pedir(self, metodo, params=None):
        self.id += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.id, "method": metodo, "params": params or {}}) + "\n")
        self.p.stdin.flush()
        while True:
            msg = json.loads(self.p.stdout.readline())
            if "method" not in msg:
                return msg
            # El servidor pregunta: contesta como si la persona dijera que sí (o que no).
            self.preguntas.append(msg)
            resp = {"action": "accept", "content": {"confirmar": True}} if self.aceptar else {"action": "decline"}
            self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": msg["id"], "result": resp}) + "\n")
            self.p.stdin.flush()

    def usar(self, nombre, /, **args):
        r = self.pedir("tools/call", {"name": nombre, "arguments": args})["result"]
        t = "\n".join(c.get("text", "") for c in r["content"] if c["type"] == "text")
        self.salidas.append(t)
        return t, r

    def cerrar(self):
        self.p.stdin.close()
        self.p.wait(timeout=10)


MAIN_JS = """const { app, BrowserWindow } = require('electron');
const url = process.argv.filter(a => /^(https?:|about:)/.test(a)).pop() || 'about:blank';
app.whenReady().then(() => { const w = new BrowserWindow({ show: true, width: 1280, height: 800 }); w.loadURL(url); });
app.on('window-all-closed', () => {});
"""


def sin_ejecutar(p):
    return not (p.stat().st_mode & 0o111)


def main():
    cual = sys.argv[1] if len(sys.argv) > 1 else "firefox"
    if cual == "firefox" and not shutil.which("firefox"):
        print("Sin Firefox: se salta.")
        return 0
    if cual == "brave" and not shutil.which("electron43"):
        print("Sin electron43 (el Chromium que hace de Brave): se salta.")
        return 0
    cdp = cual != "firefox"
    print(f"### Navegador: {cual}{' (Electron haciéndose pasar por Brave, CDP)' if cdp else ' (WebDriver BiDi)'}")
    tmp = Path(tempfile.mkdtemp(prefix="vortex-mcp-navegador-"))
    srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Manejador)
    srv.handle_error = lambda *a: None  # las descargas que se cortan a propósito no ensucian la salida
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    puerto = srv.server_address[1]
    base = f"http://127.0.0.1:{puerto}"
    (tmp / "bin").mkdir()
    (tmp / "bin/notify-send").write_text(f"#!/bin/sh\necho \"$*\" >> {tmp}/avisos.log\n")
    (tmp / "bin/notify-send").chmod(0o755)
    env = dict(os.environ, HOME=str(tmp / "home"), XDG_CONFIG_HOME=str(tmp / "config"), XDG_DATA_HOME=str(tmp / "data"),
               XDG_RUNTIME_DIR=str(tmp / "run"), XDG_CACHE_HOME=str(tmp / "cache"), XDG_STATE_HOME=str(tmp / "state"),
               PATH=f"{tmp}/bin:{os.environ['PATH']}", VORTEX_NAVEGADOR_PERFIL=str(tmp / "perfil"), MOZ_HEADLESS="1",
               VORTEX_CLAUDE_MAX_ARCHIVO=str(MAX_PRUEBA), VORTEX_SIN_ELICITACION="")
    for k in ("WAYLAND_DISPLAY", "DISPLAY", "VORTEX_NAVEGADOR_BINARIO", "VORTEX_CLAUDE_ARCHIVOS"):
        env.pop(k, None)
    if cdp:
        (tmp / "electron").mkdir()
        (tmp / "electron/main.js").write_text(MAIN_JS)
        (tmp / "bin/brave-falso").write_text(
            f"#!/bin/sh\necho \"$*\" >> {tmp}/brave-args.log\n"
            f"exec electron43 {tmp}/electron/main.js --ozone-platform=headless --ozone-override-screen-size=1280,800 --disable-gpu --use-gl=disabled \"$@\"\n")
        (tmp / "bin/brave-falso").chmod(0o755)
        env["VORTEX_NAVEGADOR_BINARIO"] = str(tmp / "bin/brave-falso")
    for d in ("home", "config", "data", "run"):
        (tmp / d).mkdir()
    (tmp / "run").chmod(0o700)
    carpeta = tmp / "home/Descargas/Claude"
    perfil = tmp / ("perfil-brave" if cdp else "perfil")
    nombre_nav = "Brave" if cdp else "Firefox"
    c = None

    def avisos():
        time.sleep(0.2)
        return (tmp / "avisos.log").read_text() if (tmp / "avisos.log").exists() else ""

    def en_carpeta():
        return sorted(str(p.relative_to(carpeta)) for p in carpeta.rglob("*") if p.is_file() and ".entrando" not in p.parts) if carpeta.exists() else []

    try:
        print("== Sin navegador abierto")
        c = Cliente(env)
        init = c.pedir("initialize", {"protocolVersion": "2025-06-18"})["result"]
        probar("versión 0.3.0", init["serverInfo"]["version"] == "0.3.0")
        probar("las instrucciones hablan del navegador y de los archivos",
               "vortex claude navegador" in init["instructions"] and "navegador_descargar" in init["instructions"])
        nombres = {h["name"]: h for h in c.pedir("tools/list")["result"]["tools"]}
        probar("siguen las herramientas de antes",
               {"ver_pantalla", "ventana_activa", "ventanas", "portapapeles", "historial_terminal", "estado_sistema"} <= set(nombres))
        probar("están las del navegador", all(f"navegador_{n}" in nombres for n in (
            "abrir", "leer", "clic", "escribir", "consola", "red", "evaluar", "rendimiento", "estilos", "depurar_local",
            "descargar", "subir")))
        probar("y las de archivos", all(f"archivos_{n}" in nombres for n in ("listar", "leer", "mover", "borrar", "extraer")))
        probar("clic y descargar no son de solo lectura; leer sí",
               not nombres["navegador_clic"]["annotations"]["readOnlyHint"] and nombres["navegador_leer"]["annotations"]["readOnlyHint"]
               and not nombres["navegador_descargar"]["annotations"]["readOnlyHint"] and nombres["archivos_leer"]["annotations"]["readOnlyHint"])
        t, r = c.usar("navegador_leer")
        probar("pide abrirlo con «vortex claude navegador»", r.get("isError") and "vortex claude navegador" in t)

        print(f"== Abrir el navegador (headless, perfil temporal: {nombre_nav})")
        a = subprocess.run([sys.executable, str(MCP), "navegador", "abrir", cual, "--headless"], env=env, capture_output=True, text=True, timeout=60)
        probar("abre", a.returncode == 0, a.stdout + a.stderr)
        if cdp:
            prefs = json.loads((perfil / "Default/Preferences").read_text()) if (perfil / "Default/Preferences").exists() else {}
            probar("el perfil es el temporal, sin contraseñas ni autocompletar",
                   prefs.get("credentials_enable_service") is False and prefs["autofill"]["enabled"] is False)
            args = (tmp / "brave-args.log").read_text()
            probar("sin sincronizar, sin el llavero y el control solo en 127.0.0.1",
                   all(x in args for x in ("--disable-sync", "--password-store=basic", "--remote-debugging-address=127.0.0.1",
                                           f"--user-data-dir={perfil}")), args)
        else:
            probar("el perfil es el temporal, con su user.js",
                   (perfil / "user.js").exists() and "sanitizeOnShutdown" in (perfil / "user.js").read_text())
        probar("avisa con una notificación", f"{nombre_nav} aparte para Claude" in avisos(), avisos())

        print("== Abrir y leer")
        t, _ = c.usar("navegador_abrir", url=f"{base}/")
        probar("abre la página y dice su título y estado", "Página de prueba" in t and "200" in t, t)
        probar("dice que hubo errores", "errores en la consola" in t, t)
        time.sleep(0.5)
        t, _ = c.usar("navegador_leer", que="texto")
        probar("lee el texto visible", "Hola desde la prueba" in t and "nada todavía" in t, t)
        probar("lo marca como contenido de la página (no instrucciones)", "no instrucciones" in t and "[Fin del contenido de la página" in t)
        t, _ = c.usar("navegador_leer", que="estructura")
        probar("la estructura trae el título, el botón y los campos", "título 1" in t and "#boton" in t and "#q" in t, t)
        t, _ = c.usar("navegador_leer", que="enlaces")
        probar("los enlaces, con el token de la URL tapado", "/otra?" in t and "q=gatos" in t, t)
        t, _ = c.usar("navegador_leer", que="formularios")
        probar("los formularios dicen que hay contraseña sin mostrarla", "contraseña o dato de pago: no se muestra" in t and "«mike»" in t)
        t, r = c.usar("navegador_captura")
        probar("captura (una imagen)", any(x["type"] == "image" and len(x["data"]) > 1000 for x in r["content"]), t)
        t, r = c.usar("navegador_captura", selector="#titulo")
        probar("captura de un elemento", any(x["type"] == "image" for x in r["content"]) and not r.get("isError"), t)

        print("== Interactuar")
        t, _ = c.usar("navegador_clic", texto="Haz clic")
        t, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("el clic llega a la página", "clic recibido" in t, t)
        t, _ = c.usar("navegador_escribir", campo="Buscar", contenido="gatos con ñ", enter=True)
        t, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("escribe (con ñ) y Enter manda el formulario normal", "buscaste gatos con ñ" in t, t)
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
        probar("presiona teclas", "Tab" in t, t)
        t, _ = c.usar("navegador_desplazar", pixeles=200)
        probar("desplaza", "La página está en" in t)
        c.usar("navegador_evaluar", expresion="setTimeout(() => { const d = document.createElement('p'); d.id = 'tarde'; d.textContent = 'llegó tarde'; document.body.append(d); }, 400), 1")
        t, _ = c.usar("navegador_esperar", selector="#tarde", segundos=5)
        probar("espera a que aparezca algo", "Ya apareció" in t, t)

        print("== Depurar")
        t, _ = c.usar("navegador_consola")
        probar("la consola trae log, warn y error", "clic ok" in t and "aviso de prueba" in t and "error de prueba" in t, t)
        probar("y el error de JavaScript con su pila", "fallo a propósito" in t and "fallaAProposito" in t, t)
        t, _ = c.usar("navegador_consola", nivel="error")
        probar("filtra por nivel", "error de prueba" in t and "clic ok" not in t)
        t, _ = c.usar("navegador_red")
        probar("la red trae la que falla con su estado", "/api/falla → 500" in t, t)
        probar("la que no se pudo conectar", "nadie-escucha → FALLÓ" in t, t)
        probar("y la que sí", "/api/ok → 200" in t)
        t, _ = c.usar("navegador_red", solo_fallos=True, cabeceras=True)
        probar("solo fallos", "/api/ok" not in t and "/api/falla" in t)
        t, _ = c.usar("navegador_red", filtro="/api/ok", cabeceras=True)
        probar("las cabeceras normales se ven y Authorization no", "x-normal: visible" in t.lower() and "authorization: [oculto]" in t.lower(), t)
        t, _ = c.usar("navegador_evaluar", expresion="document.querySelectorAll('option').length * 2")
        probar("evalúa JavaScript", "6" in t)
        t, _ = c.usar("navegador_evaluar", expresion="Promise.resolve({a: 1, b: [2, 3]})")
        probar("espera promesas y da JSON", '"a": 1' in t)
        t, _ = c.usar("navegador_evaluar", expresion="noExiste.algo")
        probar("una excepción sale como tal", "excepción" in t and "noExiste" in t, t)
        t, _ = c.usar("navegador_rendimiento")
        probar("rendimiento", "Primer byte" in t and "recursos" in t)
        t, _ = c.usar("navegador_estilos", selector="#titulo", propiedades=["color", "font-size"])
        probar("estilos calculados", "rgb(200, 10, 10)" in t and "32px" in t)
        t, _ = c.usar("navegador_depurar_local", puerto=puerto, esperar=1)
        probar("el diagnóstico local junta errores, fallos y tiempos",
               "Errores en la consola" in t and "error de prueba" in t and "/api/falla" in t and "Carga:" in t, t)
        t, _ = c.usar("navegador_historial", accion="recargar", sin_cache=True)
        probar("recarga", "Página de prueba" in t, t)
        c.usar("navegador_abrir", url=f"{base}/otra")
        t, _ = c.usar("navegador_historial", accion="atras")
        probar("atrás", "Página de prueba" in t, t)
        if cdp:
            print("  (se salta abrir y cerrar pestañas: Electron no deja crear pestañas por CDP; Brave sí)")
        else:
            t, _ = c.usar("navegador_pestanas", accion="nueva")
            probar("abre otra pestaña", "1 (actual)" in t)
            t, _ = c.usar("navegador_pestanas", accion="cerrar", indice=1)
            probar("y la cierra", t.count(" — ") == 1 and "0 (actual)" in t, t)
        t, r = c.usar("navegador_abrir", url="https://mail.google.com/")
        probar("no abre correo ni mensajes", r.get("isError"))

        print("== Descargar")
        c.usar("navegador_abrir", url=f"{base}/auto")
        time.sleep(2)
        probar("una página no baja nada por su cuenta", en_carpeta() == [], str(en_carpeta()))
        c.usar("navegador_abrir", url=f"{base}/archivos")
        t, _ = c.usar("navegador_clic", selector="#csv")
        time.sleep(1)
        probar("un clic normal tampoco: avisa que use navegador_descargar", en_carpeta() == [] and "navegador_descargar" in t, t)
        t, r = c.usar("navegador_descargar", selector="#csv")
        probar("baja con un clic en el enlace", not r.get("isError") and "datos.csv" in en_carpeta(), t)
        probar("la carpeta es ~/Descargas/Claude con permisos 700", carpeta.is_dir() and (carpeta.stat().st_mode & 0o777) == 0o700)
        if "datos.csv" in en_carpeta():
            probar("el archivo queda en 600, sin ejecutar", ((carpeta / "datos.csv").stat().st_mode & 0o777) == 0o600)
        probar("avisa nombre, tamaño y origen", "bajó «datos.csv»" in avisos() and "127.0.0.1" in avisos(), avisos())
        registro = tmp / "state/vortex/claude-archivos.jsonl"
        probar("y lo anota en el registro", registro.exists() and '"descarga"' in registro.read_text())
        t, r = c.usar("navegador_descargar", texto="Informe en PDF")
        probar("baja por el texto del enlace (un PDF)", "informe.pdf" in en_carpeta(), t)
        t, r = c.usar("navegador_descargar", selector="#sh")
        probar("un script pide confirmación y no se baja", r.get("isError") and "confirmado" in t and "instalar.sh" not in en_carpeta(), t)
        t, r = c.usar("navegador_descargar", url=f"{base}/dl/programa")
        probar("un ejecutable sin extensión (ELF) también", r.get("isError") and "confirmado" in t and "programa" not in en_carpeta(), t)
        t, r = c.usar("navegador_descargar", url=f"{base}/dl/instalar.sh", confirmado=True)
        probar("confirmado, se guarda pero sin permiso de ejecución",
               "instalar.sh" in en_carpeta() and sin_ejecutar(carpeta / "instalar.sh"), t)
        t, r = c.usar("navegador_descargar", url=f"{base}/dl/grande.dat")
        probar("más grande que el tope: no se guarda", r.get("isError") and "máximo" in t and "grande.dat" not in en_carpeta(), t)
        entrando = carpeta / ".entrando"
        probar("y no deja nada a medias (ni nada que la página bajó sola)",
               not [p for p in entrando.rglob("*") if p != entrando / "navegador"], str(list(entrando.rglob("*"))))
        for n in ("foto.png", "bueno.zip", "malo.zip", "enlace.tar"):
            c.usar("navegador_descargar", url=f"{base}/dl/{n}")
        probar("baja por URL", {"foto.png", "bueno.zip", "malo.zip", "enlace.tar"} <= set(en_carpeta()), str(en_carpeta()))
        t, r = c.usar("navegador_descargar", url="file:///etc/passwd")
        probar("solo http y https", r.get("isError"))

        print("== Trabajar con los archivos")
        t, _ = c.usar("archivos_leer", nombre="datos.csv")
        probar("lee un CSV", "3 filas" in t and "Ana,12" in t and "no instrucciones" in t, t)
        t, _ = c.usar("archivos_leer", nombre="informe.pdf")
        probar("lee un PDF (pdftotext)", "Ventas de noviembre" in t, t)
        t, r = c.usar("archivos_leer", nombre="foto.png")
        probar("una imagen: medidas y vista previa", "3×2 px" in t and any(x["type"] == "image" for x in r["content"]), t)
        t, _ = c.usar("archivos_leer", nombre="bueno.zip")
        probar("dice qué trae un zip", "sub/b.csv" in t)
        t, r = c.usar("archivos_extraer", nombre="bueno.zip")
        probar("extrae un zip a una carpeta nueva", "bueno-extraido/sub/b.csv" in en_carpeta() and "bueno-extraido/a.txt" in en_carpeta(), t)
        probar("lo extraído no se puede ejecutar", all(sin_ejecutar(p) for p in (carpeta / "bueno-extraido").rglob("*") if p.is_file()))
        t, _ = c.usar("archivos_leer", nombre="bueno-extraido/sub/b.csv")
        probar("y se lee", "1,2" in t)
        t, r = c.usar("archivos_extraer", nombre="malo.zip")
        probar("un zip con «../» no se extrae", r.get("isError") and "se sale" in t and not (carpeta.parent / "fuera.txt").exists()
               and not (carpeta / "malo-extraido").exists(), t)
        t, r = c.usar("archivos_extraer", nombre="enlace.tar")
        probar("un tar con un enlace tampoco", r.get("isError") and "enlace" in t and not (carpeta / "enlace-extraido").exists(), t)
        t, _ = c.usar("archivos_mover", origen="datos.csv", destino="tabla.csv")
        probar("renombra", "tabla.csv" in en_carpeta() and "datos.csv" not in en_carpeta(), t)
        t, r = c.usar("archivos_mover", origen="tabla.csv", destino="../fuera.csv")
        probar("pero no saca nada de la carpeta", r.get("isError") and not (carpeta.parent / "fuera.csv").exists())
        for n in ("../../.bashrc", "/etc/passwd", "..", "."):
            t, r = c.usar("archivos_leer", nombre=n)
            probar(f"no lee fuera de la carpeta ({n})", r.get("isError"), t)
        t, r = c.usar("archivos_borrar", nombre="../Claude")
        probar("ni borra la carpeta entera", r.get("isError") and carpeta.is_dir())
        t, _ = c.usar("archivos_listar")
        probar("lista la carpeta", "tabla.csv" in t and "instalar.sh" in t and ".entrando" not in t, t)

        print("== Subir")
        c.usar("navegador_abrir", url=f"{base}/subir")
        t, r = c.usar("navegador_subir", selector="#archivo", archivos=["tabla.csv"])
        probar("pone un archivo de la carpeta en el formulario (localhost, sin preguntar)", not r.get("isError"), t)
        c.usar("navegador_clic", selector="#enviar")
        time.sleep(1)
        probar("y al enviar llega al servidor", any(b"tabla.csv" in x and b"Ana,12" in x for x in RECIBIDO), str(RECIBIDO)[:300])
        probar("avisa qué subió y a dónde", "subió tabla.csv" in avisos() and "127.0.0.1" in avisos(), avisos())
        c.usar("navegador_abrir", url=f"{base}/subir")
        t, r = c.usar("navegador_subir", selector="#zona", archivos=["informe.pdf"], arrastrar=True)
        t2, _ = c.usar("navegador_leer", que="texto", selector="#resultado")
        probar("arrastrando a una zona", "soltaste informe.pdf" in t2, t + t2)
        home = tmp / "home"
        (home / ".ssh").mkdir(mode=0o700)
        (home / ".ssh/id_rsa").write_text("LLAVE PRIVADA")
        (home / "Notas/.obsidian").mkdir(parents=True)
        (home / "Notas/lista.md").write_text("notas")
        (home / "proyecto").mkdir()
        (home / "proyecto/.env").write_text("API_KEY=x")
        (home / ".config/vortex").mkdir(parents=True, exist_ok=True)
        (home / ".config/vortex/algo.txt").write_text("x")
        (carpeta / "atajo.txt").symlink_to(home / ".ssh/id_rsa")
        for ruta in ("~/.ssh/id_rsa", str(home / "Notas/lista.md"), str(home / "proyecto/.env"), str(home / ".config/vortex/algo.txt"),
                     "atajo.txt", "../../.ssh/id_rsa", "/etc/hostname"):
            t, r = c.usar("navegador_subir", selector="#archivo", archivos=[ruta], confirmado=True)
            probar(f"nunca sube de una ruta sensible ({ruta})", r.get("isError") and "nunca se sube" in t, t)
        (home / "Documentos").mkdir()
        (home / "Documentos/carta.txt").write_text("una carta")
        t, r = c.usar("navegador_subir", selector="#archivo", archivos=[str(home / "Documentos/carta.txt")])
        probar("de otra carpeta pide confirmación", r.get("isError") and "confirmado" in t, t)
        t, r = c.usar("navegador_subir", selector="#archivo", archivos=[str(home / "Documentos/carta.txt")], confirmado=True)
        probar("y con la confirmación lo sube", not r.get("isError"), t)
        t, _ = c.usar("archivos_borrar", nombre="tabla.csv")
        time.sleep(0.5)  # notify-send va aparte
        probar("borra dentro de la carpeta y avisa", "tabla.csv" not in en_carpeta() and "borró «tabla.csv»" in avisos(), t)

        print("== Que no se filtre nada sensible")
        c.usar("navegador_abrir", url=f"{base}/")
        time.sleep(0.5)
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
        probar("la llave de ~/.ssh no sale", "LLAVE PRIVADA" not in todo)
        probar("los cambios avisan (clic, escribir, evaluar)",
               all(x in avisos() for x in ("hizo clic", "escribió", "corrió JavaScript")))
        probar("lo que solo mira no avisa (lo normal)", "leyó la página" not in avisos() and "miró la consola" not in avisos())
        v = subprocess.run([str(AQUI / "bin/vortex"), "claude", "avisos", "todo"], env=env, capture_output=True, text=True, timeout=20)
        c.usar("navegador_leer")
        c.usar("navegador_consola")
        c.usar("archivos_leer", nombre="informe.pdf")
        probar("con «vortex claude avisos todo», también avisa al mirar",
               v.returncode == 0 and all(x in avisos() for x in ("leyó la página", "miró la consola", "leyó «informe.pdf»")), avisos()[-400:])
        subprocess.run([str(AQUI / "bin/vortex"), "claude", "avisos", "cambios"], env=env, capture_output=True, timeout=20)

        print("== Pausa")
        (tmp / "config/vortex").mkdir(parents=True, exist_ok=True)
        (tmp / "config/vortex/claude-pausa").touch()
        t, r = c.usar("navegador_leer")
        probar("en pausa no hace nada", r.get("isError") and "pausa" in t)
        t, r = c.usar("navegador_descargar", url=f"{base}/dl/datos.csv")
        t2, r2 = c.usar("archivos_listar")
        probar("ni baja ni mira archivos", r.get("isError") and r2.get("isError") and "pausa" in t)
        (tmp / "config/vortex/claude-pausa").unlink()

        print("== Confirmar con un cuadro (elicitation de MCP)")
        e2 = Cliente(env)
        try:
            e2.pedir("initialize", {"protocolVersion": "2025-06-18", "capabilities": {"elicitation": {}}})
            t, r = e2.usar("navegador_descargar", url=f"{base}/dl/otro.run")
            probar("si el cliente sabe preguntar, le pregunta a la persona y sigue si dice que sí",
                   len(e2.preguntas) == 1 and "otro.run" in e2.preguntas[0]["params"]["message"] and "otro.run" in en_carpeta(), t)
            e2.aceptar = False
            t, r = e2.usar("navegador_descargar", url=f"{base}/dl/programa")
            probar("y si dice que no, no lo hace", r.get("isError") and "no lo confirmó" in t and "programa" not in en_carpeta(), t)
        finally:
            e2.cerrar()

        print("== vortex claude navegador / pausa")
        vortex = str(AQUI / "bin/vortex")
        subprocess.run([sys.executable, str(MCP), "navegador", "cerrar"], env=env, capture_output=True, timeout=20)
        if cdp:
            probar("al cerrar se borra el perfil de Brave entero", not perfil.exists())
        a = subprocess.run([vortex, "claude", "navegador", cual, "--headless"], env=env, capture_output=True, text=True, timeout=60)
        e = subprocess.run([vortex, "claude", "navegador", "estado"], env=env, capture_output=True, text=True, timeout=20)
        probar(f"«vortex claude navegador {cual}» lo abre", a.returncode == 0 and f"abierto: {cual}" in e.stdout, a.stdout + a.stderr + e.stdout)
        p = subprocess.run([vortex, "claude", "pausa"], env=env, capture_output=True, text=True, timeout=20)
        e = subprocess.run([vortex, "claude", "navegador", "estado"], env=env, capture_output=True, text=True, timeout=20)
        probar("«vortex claude pausa» lo cierra", p.returncode == 0 and "cerrado" in e.stdout, p.stdout + e.stdout)
        a = subprocess.run([vortex, "claude", "navegador", "--headless"], env=env, capture_output=True, text=True, timeout=60)
        probar("en pausa no se abre", a.returncode != 0 and "pausa" in a.stderr, a.stdout + a.stderr)
        subprocess.run([vortex, "claude", "reanudar"], env=env, capture_output=True, timeout=20)
        (tmp / "config/vortex/claude-navegador").write_text(cual + "\n")
        a = subprocess.run([sys.executable, str(MCP), "navegador", "--headless"], env=env, capture_output=True, text=True, timeout=60)
        e = subprocess.run([sys.executable, str(MCP), "navegador", "estado"], env=env, capture_output=True, text=True, timeout=20)
        probar("sin decir cuál, abre el de ~/.config/vortex/claude-navegador", f"abierto: {cual}" in e.stdout, a.stdout + a.stderr + e.stdout)
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
        print(f"FALLARON {FALLOS} pruebas ({cual})")
        return 1
    print(f"Todo bien ({cual})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
