"""Pruebas del cerebro con cualquier IA (cerebro_abierto.py e ia.py), sin internet ni GPU.

Levanta una IA falsa compatible con OpenAI (contesta en flujo y, la primera vez, pide una herramienta), un
servidor MCP falso (devuelve texto y una imagen) y un Ollama falso (/api/version, /api/show, /api/pull).

    python3 -I apps/miku/pruebas/ia.py
"""
import json
import os
import subprocess
import sys
import tempfile
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

AQUI = Path(__file__).resolve().parent
MIKU = AQUI.parent
TMP = Path(tempfile.mkdtemp(prefix="vortex-miku-ia-"))
# Sin D-Bus, secret-tool no llega al llavero de tu sesión: las claves de prueba van al archivo del HOME temporal.
os.environ.update(HOME=str(TMP / "home"), XDG_CONFIG_HOME=str(TMP / "home/.config"), PATH="/usr/bin:/bin",
                  DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent")
(TMP / "home/.config").mkdir(parents=True)

PEDIDOS: list[dict] = []
CARGAS: list[dict] = []
ESTADO = {"modelo_bajado": False, "codigo": 200}


def sse(trozos: list[dict]) -> bytes:
    return b"".join(b"data: " + json.dumps({"choices": [{"delta": t}]}).encode() + b"\n\n" for t in trozos) + b"data: [DONE]\n\n"


class IAFalsa(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def _json(self, codigo: int, datos: dict) -> None:
        cuerpo = json.dumps(datos).encode()
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)

    def do_GET(self):
        if self.path == "/api/version":
            return self._json(200, {"version": "0.0.0"})
        self._json(404, {})

    def do_POST(self):
        datos = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        if self.path == "/api/show":
            return self._json(200 if ESTADO["modelo_bajado"] else 404, {})
        if self.path == "/api/generate":     # precargar o soltar el modelo
            CARGAS.append(datos)
            return self._json(200, {"done": True})
        if self.path == "/api/pull":
            self.send_response(200)
            self.end_headers()
            for hecho in (0, 500, 1000):
                self.wfile.write(json.dumps({"status": "pulling", "digest": "sha256:a", "total": 1000, "completed": hecho}).encode() + b"\n")
            self.wfile.write(b'{"status": "success"}\n')
            ESTADO["modelo_bajado"] = True
            return
        PEDIDOS.append({"ruta": self.path, "datos": datos, "auth": self.headers.get("Authorization")})
        if ESTADO["codigo"] != 200:
            return self._json(ESTADO["codigo"], {"error": {"message": "no"}})
        mensajes = datos["messages"]
        if not datos.get("stream"):         # mejores momentos: una respuesta entera
            texto = '{"momentos": [{"candidato": 2, "titulo": "La remontada", "por_que": "gritos"}]}'
            return self._json(200, {"choices": [{"message": {"role": "assistant", "content": texto}}]})
        if mensajes[-1]["role"] == "user" and mensajes[-1]["content"] == "mira mi pantalla":
            cuerpo = sse([{"content": "Déjame ver. "},
                          {"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "prueba__eco", "arguments": '{"texto":'}}]},
                          {"tool_calls": [{"index": 0, "function": {"arguments": ' "hola"}'}}]}])
        else:
            cuerpo = sse([{"content": "Ya vi. "}, {"content": "Todo bien."}])
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        self.wfile.write(cuerpo)


MCP_FALSO = r'''
import json, sys
for linea in sys.stdin:
    m = json.loads(linea)
    if "id" not in m:
        continue
    r = {}
    if m["method"] == "tools/list":
        r = {"tools": [{"name": "eco", "description": "repite", "inputSchema": {"type": "object", "properties": {"texto": {"type": "string"}}}}]}
    elif m["method"] == "tools/call":
        r = {"content": [{"type": "text", "text": "eco: " + m["params"]["arguments"].get("texto", "")},
                         {"type": "image", "data": "aGVsbG8=", "mimeType": "image/jpeg"}]}
    print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": r}), flush=True)
'''

servidor = ThreadingHTTPServer(("127.0.0.1", 0), IAFalsa)
threading.Thread(target=servidor.serve_forever, daemon=True).start()
URL = f"http://127.0.0.1:{servidor.server_port}"
os.environ["VORTEX_MIKU_OLLAMA"] = URL
sys.path.insert(0, str(MIKU))
from miku import cerebro_abierto, config, ia  # noqa: E402

fallas = 0


def ver(cond: bool, texto: str) -> None:
    global fallas
    print(("  ok  " if cond else "  MAL ") + texto)
    fallas += not cond


def nuevo_cerebro(cfg: dict) -> tuple:
    eventos: list = []
    mcp = TMP / "mcp_falso.py"
    mcp.write_text(MCP_FALSO)
    c = cerebro_abierto.CerebroAbierto(False, lambda tipo, **d: eventos.append((tipo, d)),
                                       dict(config.DEFECTOS, **cfg), servidores=[("prueba", [sys.executable, "-I", str(mcp)], None)])
    return c, eventos


print("== Configuración")
config.guardar({"ia": "gemini", "ia_modelo": "otro-modelo"})
cfg = config.cargar()
ver(cfg["ia"] == "gemini" and ia.elegido(cfg)["modelo"] == "otro-modelo", "ia y ia_modelo se guardan y se leen")
config.guardar({"ia": "inventada"})
ver(config.cargar()["ia"] == "claude", "una IA desconocida vuelve a Claude")
extra = ia.elegido({"ia": "local"})["extra"]
ver(extra["reasoning_effort"] == "none", "la local va sin «pensar» (más rápida)")
ver(extra["temperature"] == 0.7 and extra["top_p"] == 0.8, "con la temperatura que Qwen recomienda sin pensar")
ver(cerebro_abierto._hoy().startswith("\n\nHoy es ") and str(time.localtime().tm_year) in cerebro_abierto._hoy(),
    "la IA sabe qué día es")

print("== Clave (sin llavero: archivo 0600)")
donde = ia.guardar_clave("gemini", "clave-secreta")
modo = oct(os.stat(ia.archivo_clave("gemini")).st_mode & 0o777)
ver(donde.endswith("ia-gemini.clave") and modo == "0o600", f"se guarda en un archivo 0600 ({modo})")
ver(ia.leer_clave("gemini") == "clave-secreta", "se lee")
ia.guardar_clave("gemini", "\x1b[200~AIza-clave_1\x1b[201~\n")
ver(ia.leer_clave("gemini") == "AIza-clave_1", "al pegar en algunas terminales se cuelan ESC[200~ … ESC[201~: se quitan")
ia.archivo_clave("gemini").write_text("[200~AIza-vieja [201~\n")
ver(ia.leer_clave("gemini") == "AIza-vieja", "…también de una clave ya guardada así (antes del arreglo)")
ia.guardar_clave("gemini", "clave-secreta")
ver("clave-secreta" not in (TMP / "home/.config/vortex/miku.json").read_text(), "no va en miku.json")

print("== Conversación con herramienta e imagen")
c, ev = nuevo_cerebro({"ia": "otra", "ia_url": URL + "/v1", "ia_modelo": "falso"})
c.preguntar("mira mi pantalla", etiqueta=1)
tipos = [t for t, _ in ev]
texto = "".join(d.get("delta", "") for t, d in ev if t == "texto")
ver(tipos[0] == "inicio" and tipos[-1] == "listo", f"empieza y termina bien ({tipos[0]} … {tipos[-1]})")
ver(("herramienta", {"etiqueta": 1, "nombre": "mcp__prueba__eco"}) in ev, "avisa de la herramienta con su nombre mcp__")
ver(texto == "Déjame ver. Ya vi. Todo bien.", f"el texto llega en trozos ({texto!r})")
segundo = PEDIDOS[-1]["datos"]["messages"]
tool = [m for m in segundo if m["role"] == "tool"]
ver(tool and tool[0]["content"] == "eco: hola" and tool[0]["tool_call_id"] == "c1", "el resultado vuelve a la IA (argumentos en trozos)")
imagen = segundo[-1]["content"]
ver(isinstance(imagen, list) and imagen[1]["image_url"]["url"] == "data:image/jpeg;base64,aGVsbG8=", "la imagen le llega como image_url")
ver(PEDIDOS[-1]["datos"]["model"] == "falso" and PEDIDOS[-1]["datos"]["tools"][0]["function"]["name"] == "prueba__eco",
    "manda el modelo y las herramientas")
ver(PEDIDOS[-1]["auth"] is None, "sin clave guardada no manda Authorization")

print("== Memoria")
c.preguntar("¿y ahora?", etiqueta=2)
msgs = PEDIDOS[-1]["datos"]["messages"]
ver(any(m.get("content") == "mira mi pantalla" for m in msgs), "recuerda la pregunta anterior")
ver(not any(isinstance(m.get("content"), list) for m in msgs), "el historial guarda la captura como texto, no la imagen")
c.olvidar()
ver(c.historial == [], "olvidar borra la conversación")

print("== Errores en español")
for codigo, clave in ((401, "clave"), (429, "cuota"), (503, "saturado")):
    ESTADO["codigo"] = codigo
    c, ev = nuevo_cerebro({"ia": "otra", "ia_url": URL + "/v1", "ia_modelo": "falso"})
    c.preguntar("hola", etiqueta=3)
    err = [d for t, d in ev if t == "error"]
    ver(err and err[0]["clave"] == clave, f"{codigo} → «{clave}»: {err[0]['mensaje'] if err else '(nada)'}")
ESTADO["codigo"] = 200
c, ev = nuevo_cerebro({"ia": "gemini"})
os.environ["VORTEX_MIKU_IA_CLAVE"] = ""
ia.borrar_clave("gemini")
ver(c.comprobar()[0] == "clave", "Gemini sin clave pide la clave antes de preguntar")
e = ia._explicar(ia.PROVEEDORES["gemini"], 400, '[{"error": {"code": 400, "message": "API key not valid. Please pass a valid API key."}}]')
ver(e.clave == "clave" and "vortex-miku ia clave" in str(e), "el 400 de Gemini por clave mala se explica en español")
c, ev = nuevo_cerebro({"ia": "otra", "ia_url": "http://127.0.0.1:9/v1", "ia_modelo": "x"})
c.preguntar("hola", etiqueta=4)
err = [d for t, d in ev if t == "error"]
ver(err and err[0]["clave"] == "internet", "sin conexión → «internet»")

print("== IA local (Ollama falso)")
bin_falso = TMP / "bin"
bin_falso.mkdir()
(bin_falso / "ollama").write_text("#!/bin/sh\nexit 0\n")
(bin_falso / "ollama").chmod(0o755)
os.environ["PATH"] = f"{bin_falso}:{os.environ['PATH']}"
local = ia.elegido({"ia": "local"})
ver(local["url"] == URL + "/v1" and local["modelo"] == ia.MODELO_LOCAL, f"apunta a Ollama con {ia.MODELO_LOCAL}")
ver(ia.comprobar(local)[0] == "modelo", "sin el modelo pide bajarlo")
avance: list = []
ia.bajar_modelo(local["modelo"], progreso=lambda d, f: avance.append(f))
ver(avance and avance[-1] == 1.0 and 0.5 in avance, f"baja con progreso ({avance})")
ver(ia.comprobar(local) == ("ok", ""), "con el modelo bajado ya puede hablar")
ver(not CARGAS, "comprobar no sube el modelo a la GPU (lo hace la ventana cuando su voz ya cargó)")
ia.precargar(local["modelo"])
for _ in range(50):
    if CARGAS:
        break
    time.sleep(0.05)
ver(CARGAS == [{"model": ia.MODELO_LOCAL, "keep_alive": "10m"}], f"precargar sube el modelo aparte ({CARGAS})")
ia.precargar(local["modelo"])
time.sleep(0.3)
ver(len(CARGAS) == 1, "precargar otra vez enseguida no repite")
ia.soltar(local["modelo"])
ver(CARGAS[-1] == {"model": ia.MODELO_LOCAL, "keep_alive": 0}, "soltar lo saca de la GPU (al cerrar Miku)")

print("== Mejores momentos con otra IA")
from miku import momentos  # noqa: E402
config.guardar({"ia": "otra", "ia_url": URL + "/v1", "ia_modelo": "falso"})
mini = TMP / "mini.jpg"
mini.write_bytes(b"\xff\xd8\xff")
cands = [dict(n=i, inicio=i * 30.0, fin=i * 30.0 + 15, sonido=0.5, movimiento=0.5, miniatura=str(mini)) for i in (1, 2, 3)]
tarea = TMP / "tarea"
tarea.mkdir()
elegidos = momentos.elegir_con_ia(cands, tarea, lambda *a: None)
ver(elegidos and elegidos[0]["n"] == 2 and elegidos[0]["titulo"] == "La remontada", "la IA elige el momento y le pone título")
imgs = [c for c in PEDIDOS[-1]["datos"]["messages"][0]["content"] if c["type"] == "image_url"]
ver(len(imgs) == 3 and imgs[0]["image_url"]["url"].startswith("data:image/jpeg;base64,"), "le llegan las 3 miniaturas")

print("== vortex-miku ia")
env = dict(os.environ, VORTEX_MIKU_RAIZ=str(MIKU))
r = subprocess.run([sys.executable, "-I", str(MIKU / "vortex-miku"), "ia", "openai", "gpt-prueba"], capture_output=True, text=True,
                   env=env, stdin=subprocess.DEVNULL)
cfg = config.cargar()
ver(r.returncode == 0 and cfg["ia"] == "openai" and cfg["ia_modelo"] == "gpt-prueba", f"ia openai MODELO lo guarda ({r.stdout.strip()[:60]})")
r = subprocess.run([sys.executable, "-I", str(MIKU / "vortex-miku"), "ia", "otra", "no-es-url", "m"], capture_output=True, text=True, env=env)
ver(r.returncode == 2, "ia otra sin URL válida da error de uso")
r = subprocess.run([sys.executable, "-I", str(MIKU / "vortex-miku"), "ia"], capture_output=True, text=True, env=env)
ver(r.returncode == 0 and r.stdout.startswith("ia=openai"), "ia (estado) dice cuál está")

servidor.shutdown()
print("\nTodas las pruebas de la IA pasaron." if not fallas else f"\nFallaron {fallas}.")
sys.exit(1 if fallas else 0)
