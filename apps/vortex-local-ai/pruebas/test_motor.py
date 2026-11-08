"""Pruebas del motor de Vortexine's Local AI (sin ventana, sin red y sin Ollama de verdad).

Un servidor HTTP falso hace de Ollama en 127.0.0.1 (un puerto libre cualquiera) y todo lo demás va en
carpetas temporales: no se toca ~/.ollama, ni tus chats, ni tu vault.

    cd apps/vortex-local-ai && python3 -m unittest discover -s pruebas -p 'test_motor.py'
"""
from __future__ import annotations

import atexit
import json
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import zlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from unittest import mock

# Antes de importar el motor: nada de arrancar Ollama ni de usar las carpetas del usuario.
_HOME_PRUEBAS = tempfile.mkdtemp(prefix="vortex-local-ai-pruebas-")
atexit.register(shutil.rmtree, _HOME_PRUEBAS, ignore_errors=True)
os.environ["VORTEX_LOCAL_AI_HOME"] = _HOME_PRUEBAS
os.environ["VORTEX_LOCAL_AI_NO_ARRANCAR"] = "1"
os.environ["VORTEX_LOCAL_AI_OLLAMA"] = "http://127.0.0.1:9"   # por si algo usa la URL por defecto
os.environ.pop("VORTEX_LOCAL_AI_VRAM_MB", None)

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from vortex_local_ai import __version__, catalogo, conversaciones, rutas  # noqa: E402
from vortex_local_ai.memoria import Memoria, fragmentar  # noqa: E402
from vortex_local_ai.ollama import Cancelado, ErrorOllama, Ollama, _SeparadorPensar  # noqa: E402
from vortex_local_ai.vault import ErrorVault, Vault, enlaces_de, etiquetas_de, separar_frontmatter  # noqa: E402

rutas.recargar()


# ====================================================================== el Ollama falso

def vector_falso(texto: str, dims: int = 64) -> list[float]:
    """Un «embedding» de juguete: cada palabra suma 1 en una casilla. Textos con las mismas palabras se parecen."""
    v = [0.0] * dims
    for palabra in texto.lower().replace("\n", " ").split():
        palabra = palabra.strip(".,;:¿?¡!()[]#»«")
        if len(palabra) >= 3:
            v[zlib.crc32(palabra.encode()) % dims] += 1.0
    v[-1] += 0.01
    return v


class EstadoFalso:
    def __init__(self):
        self.textos_embebidos = 0
        self.borrados: list[str] = []
        self.peticiones: list[str] = []


class ManejadorFalso(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"   # sin Content-Length en el streaming: se cierra al terminar

    def log_message(self, *args):
        pass

    @property
    def estado(self) -> EstadoFalso:
        return self.server.estado

    def _json(self, datos, codigo=200):
        cuerpo = json.dumps(datos).encode()
        self.send_response(codigo)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(cuerpo)))
        self.end_headers()
        self.wfile.write(cuerpo)

    def _empezar_stream(self):
        self.send_response(200)
        self.send_header("Content-Type", "application/x-ndjson")
        self.end_headers()

    def _linea(self, datos) -> bool:
        try:
            self.wfile.write((json.dumps(datos) + "\n").encode())
            self.wfile.flush()
            return True
        except OSError:
            return False

    def _callado(self, segundos: float):
        """Se queda sin mandar nada (para probar que cancelar corta aunque el servidor no hable)."""
        fin = time.monotonic() + segundos
        while time.monotonic() < fin:
            time.sleep(0.05)

    def _cuerpo(self) -> dict:
        largo = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(largo) or b"{}")

    def do_GET(self):
        self.estado.peticiones.append(self.path)
        if self.path == "/api/version":
            return self._json({"version": "0.99.0"})
        if self.path == "/api/tags":
            return self._json({"models": [
                {"name": "qwen3:8b", "size": 5_200_000_000, "modified_at": "2026-11-01T10:00:00Z",
                 "details": {"family": "qwen3", "families": ["qwen3"], "parameter_size": "8.2B",
                             "quantization_level": "Q4_K_M"}},
                {"name": "emb:latest", "size": 600_000_000, "modified_at": "2026-11-02T10:00:00Z",
                 "details": {"family": "bert", "families": ["bert"], "parameter_size": "600M",
                             "quantization_level": "F16"}},
            ]})
        if self.path == "/api/ps":
            return self._json({"models": [{"name": "qwen3:8b", "size": 6_000_000_000, "size_vram": 4_500_000_000,
                                           "expires_at": "2026-11-07T12:00:00Z"}]})
        self._json({"error": "no existe"}, 404)

    def do_DELETE(self):
        datos = self._cuerpo()
        if self.path == "/api/delete":
            if datos.get("model") == "qwen3:8b":
                self.estado.borrados.append(datos["model"])
                return self._json({})
            return self._json({"error": f"model '{datos.get('model')}' not found"}, 404)
        self._json({"error": "no existe"}, 404)

    def do_POST(self):
        datos = self._cuerpo()
        self.estado.peticiones.append(self.path)
        modelo = datos.get("model", "")
        if self.path == "/api/pull":
            if modelo == "noexiste:1b":
                self._empezar_stream()
                self._linea({"status": "pulling manifest"})
                self._linea({"error": "pull model manifest: file does not exist"})
                return
            self._empezar_stream()
            self._linea({"status": "pulling manifest"})
            if modelo == "lento:1b":
                self._linea({"status": "pulling aaa", "digest": "sha256:aaa", "total": 1000, "completed": 10})
                self._callado(5)
                return
            for completado in (0, 250, 500, 750, 1000):
                self._linea({"status": "pulling aaa", "digest": "sha256:aaa", "total": 1000, "completed": completado})
                time.sleep(0.01)
            for completado in (0, 500):
                self._linea({"status": "pulling bbb", "digest": "sha256:bbb", "total": 500, "completed": completado})
            for estado in ("verifying sha256 digest", "writing manifest", "success"):
                self._linea({"status": estado})
            return
        if self.path == "/api/chat":
            if modelo == "noexiste":
                return self._json({"error": f"model '{modelo}' not found"}, 404)
            self._empezar_stream()
            if modelo == "pensador":
                trozos = ["<thi", "nk>Hmm, ", "pienso</th", "ink>Hola", " mundo", "<"]
                for t in trozos:
                    self._linea({"message": {"role": "assistant", "content": t}, "done": False})
            elif modelo == "campo":
                self._linea({"message": {"role": "assistant", "content": "", "thinking": "Primero pienso"}, "done": False})
                self._linea({"message": {"role": "assistant", "content": "Respuesta"}, "done": False})
            elif modelo == "lento":
                self._linea({"message": {"role": "assistant", "content": "Empiezo"}, "done": False})
                self._callado(5)
                return
            else:
                ultimo = datos["messages"][-1]["content"]
                self._linea({"message": {"role": "assistant", "content": f"Eco: {ultimo}"}, "done": False})
            self._linea({"message": {"role": "assistant", "content": ""}, "done": True,
                         "eval_count": 10, "eval_duration": 1_000_000_000})
            return
        if self.path == "/api/embed":
            if modelo != "emb:latest" and modelo != "emb":
                return self._json({"error": f"model '{modelo}' not found"}, 404)
            textos = datos.get("input")
            textos = [textos] if isinstance(textos, str) else textos
            self.estado.textos_embebidos += len(textos)
            return self._json({"model": modelo, "embeddings": [vector_falso(t) for t in textos]})
        if self.path == "/api/generate":
            return self._json({"done": True})
        self._json({"error": "no existe"}, 404)


class ServidorFalso:
    def __init__(self):
        self.httpd = ThreadingHTTPServer(("127.0.0.1", 0), ManejadorFalso)
        self.httpd.daemon_threads = True
        self.httpd.estado = EstadoFalso()
        self.url = f"http://127.0.0.1:{self.httpd.server_address[1]}"
        self.hilo = threading.Thread(target=self.httpd.serve_forever, daemon=True)
        self.hilo.start()

    @property
    def estado(self) -> EstadoFalso:
        return self.httpd.estado

    def cerrar(self):
        self.httpd.shutdown()
        self.httpd.server_close()


def puerto_cerrado() -> int:
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    puerto = s.getsockname()[1]
    s.close()
    return puerto


# ====================================================================== Ollama

class PruebaOllama(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.servidor = ServidorFalso()

    @classmethod
    def tearDownClass(cls):
        cls.servidor.cerrar()

    def setUp(self):
        self.ollama = Ollama(self.servidor.url, puede_arrancar=False)

    def test_url_por_defecto(self):
        self.assertEqual(Ollama().url, "http://127.0.0.1:9")    # de VORTEX_LOCAL_AI_OLLAMA
        self.assertFalse(Ollama().puede_arrancar)                 # de VORTEX_LOCAL_AI_NO_ARRANCAR

    def test_disponible_y_modelos(self):
        self.assertTrue(self.ollama.disponible())
        self.assertTrue(self.ollama.asegurar_servidor(timeout=1))
        self.assertEqual(self.ollama.version(), "0.99.0")
        modelos = self.ollama.modelos()
        self.assertEqual([m["nombre"] for m in modelos], ["emb:latest", "qwen3:8b"])
        qwen = modelos[1]
        for clave in ("nombre", "bytes", "familia", "parametros", "cuantizacion", "modificado"):
            self.assertIn(clave, qwen)
        self.assertEqual((qwen["bytes"], qwen["familia"], qwen["parametros"], qwen["cuantizacion"]),
                         (5_200_000_000, "qwen3", "8.2B", "Q4_K_M"))
        self.assertTrue(modelos[0]["embeddings"])
        self.assertFalse(qwen["embeddings"])

    def test_cargados(self):
        (m,) = self.ollama.cargados()
        self.assertEqual((m["nombre"], m["bytes"], m["bytes_vram"], m["en_gpu_pct"]),
                         ("qwen3:8b", 6_000_000_000, 4_500_000_000, 75))

    def test_bajar_con_progreso(self):
        eventos = []
        self.ollama.bajar("falso:1b", al_progreso=lambda c, t, e: eventos.append((c, t, e)))
        self.assertEqual(eventos[0][2], "Buscando el modelo")
        self.assertEqual(eventos[-1], (1500, 1500, "Listo"))
        self.assertIn("Bajando", {e for _, _, e in eventos})
        self.assertIn("Revisando la descarga", {e for _, _, e in eventos})
        completados = [c for c, _, _ in eventos]
        self.assertEqual(completados, sorted(completados), "el progreso nunca va para atrás")

    def test_bajar_cancelado_a_medias(self):
        cancelar = threading.Event()

        def progreso(c, t, e):
            if c >= 250:
                cancelar.set()
        with self.assertRaises(Cancelado) as caso:
            self.ollama.bajar("falso:1b", al_progreso=progreso, cancelar=cancelar)
        self.assertEqual(caso.exception.clave, "cancelado")
        self.assertIsInstance(caso.exception, ErrorOllama)

    def test_bajar_cancelado_con_el_servidor_callado(self):
        """El servidor se queda callado 5 s: cancelar tiene que cortar ya, no esperar al siguiente trozo."""
        cancelar = threading.Event()
        threading.Timer(0.3, cancelar.set).start()
        inicio = time.monotonic()
        with self.assertRaises(Cancelado):
            self.ollama.bajar("lento:1b", cancelar=cancelar)
        self.assertLess(time.monotonic() - inicio, 2.5)

    def test_bajar_modelo_que_no_existe(self):
        with self.assertRaises(ErrorOllama) as caso:
            self.ollama.bajar("noexiste:1b")
        self.assertEqual(caso.exception.clave, "modelo")
        self.assertIn("No encontré", str(caso.exception))
        self.assertIn("noexiste:1b", str(caso.exception))

    def test_chat_con_think(self):
        trozos, pensado = [], []
        respuesta = self.ollama.chat("pensador", [{"role": "user", "content": "hola"}],
                                     al_trozo=trozos.append, al_pensar=pensado.append)
        self.assertEqual(respuesta, "Hola mundo<")
        self.assertEqual("".join(trozos), "Hola mundo<")
        self.assertEqual("".join(pensado), "Hmm, pienso")
        self.assertNotIn("think", "".join(trozos))
        self.assertEqual(self.ollama.ultimas_estadisticas["tokens"], 10)
        self.assertEqual(self.ollama.ultimas_estadisticas["tokens_por_segundo"], 10.0)

    def test_chat_con_campo_thinking(self):
        pensado = []
        respuesta = self.ollama.chat("campo", [{"role": "user", "content": "x"}], al_pensar=pensado.append)
        self.assertEqual(respuesta, "Respuesta")
        self.assertEqual(pensado, ["Primero pienso"])

    def test_chat_eco_y_opciones(self):
        self.assertEqual(self.ollama.chat("qwen3:8b", [{"role": "user", "content": "¿qué tal?"}],
                                          opciones={"temperature": 0.2}), "Eco: ¿qué tal?")

    def test_chat_cancelado_regresa_lo_que_llevaba(self):
        cancelar = threading.Event()
        inicio = time.monotonic()
        respuesta = self.ollama.chat("lento", [{"role": "user", "content": "x"}],
                                     al_trozo=lambda t: cancelar.set(), cancelar=cancelar)
        self.assertEqual(respuesta, "Empiezo")
        self.assertLess(time.monotonic() - inicio, 2.5)

    def test_chat_modelo_que_no_tienes(self):
        with self.assertRaises(ErrorOllama) as caso:
            self.ollama.chat("noexiste", [{"role": "user", "content": "x"}])
        self.assertEqual(caso.exception.clave, "modelo")
        self.assertIn("Bájalo", str(caso.exception))

    def test_embeber_y_borrar(self):
        vectores = self.ollama.embeber("emb", ["hola mundo", "adiós"])
        self.assertEqual(len(vectores), 2)
        self.assertEqual(len(vectores[0]), 64)
        self.assertEqual(len(self.ollama.embeber("emb", "uno")), 1)
        self.ollama.borrar("qwen3:8b")
        self.assertIn("qwen3:8b", self.servidor.estado.borrados)
        with self.assertRaises(ErrorOllama):
            self.ollama.borrar("otro:1b")

    def test_separador_de_think_con_etiquetas_partidas(self):
        s = _SeparadorPensar()
        salida = []
        for trozo in ["a<", "th", "ink>b</", "think", ">c<t"]:
            salida += s.meter(trozo)
        salida += s.terminar()
        self.assertEqual("".join(t for p, t in salida if not p), "ac<t")
        self.assertEqual("".join(t for p, t in salida if p), "b")


class PruebaServidorCaido(unittest.TestCase):
    def setUp(self):
        self.ollama = Ollama(f"http://127.0.0.1:{puerto_cerrado()}", puede_arrancar=False)

    def test_error_claro(self):
        self.assertFalse(self.ollama.disponible())
        with self.assertRaises(ErrorOllama) as caso:
            self.ollama.modelos()
        self.assertIn(caso.exception.clave, ("conexion", "no_instalado"))
        self.assertTrue("No me pude conectar" in str(caso.exception) or "no está instalado" in str(caso.exception))
        with self.assertRaises(ErrorOllama):
            self.ollama.chat("qwen3:8b", [{"role": "user", "content": "x"}])
        with self.assertRaises(ErrorOllama):
            self.ollama.bajar("qwen3:8b")

    def test_no_arranca_nada(self):
        inicio = time.monotonic()
        self.assertFalse(self.ollama.asegurar_servidor(timeout=1))
        self.assertLess(time.monotonic() - inicio, 3)
        self.assertTrue(self.ollama.ultimo_error)
        self.assertEqual(self.ollama._arranque, {})

    def test_url_remota_no_se_arranca(self):
        remota = Ollama("http://192.0.2.1:11434", puede_arrancar=True)
        remota.disponible = lambda: False     # sin esperar a la red
        self.assertFalse(remota.asegurar_servidor(timeout=1))
        self.assertIn("no está en esta PC", remota.ultimo_error)
        self.assertEqual(remota._arranque, {})


# ====================================================================== Vault

INICIO = """---
tags: [inicio]
---
# Inicio

Mira mis [[Ideas]] y sobre todo [[Ideas#Juegos|mis juegos]]. Algo pendiente: [[Fantasma]].
![[foto.png]]

#proyecto #2026 y una Canción bonita.

```python
print("[[NoEsEnlace]] #noetiqueta")
```
Y `[[TampocoEsEnlace]]`.
"""

IDEAS = """---
aliases: [Lluvia de ideas]
tags:
  - ideas
  - juegos/roblox
---
# Ideas

## Juegos
Un juego de carreras. Vuelve al [[Inicio]].
"""

SUB = """Esto viene de la [[lluvia de ideas]] y del [[Inicio|principio]].
"""


class PruebaVault(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="vault-")
        self.raiz = Path(self._tmp.name)
        (self.raiz / "Carpeta").mkdir()
        (self.raiz / "Vacía").mkdir()
        (self.raiz / "Inicio.md").write_text(INICIO, encoding="utf-8")
        (self.raiz / "Ideas.md").write_text(IDEAS, encoding="utf-8")
        (self.raiz / "Carpeta" / "Sub.md").write_text(SUB, encoding="utf-8")
        # Lo de Obsidian que no se debe tocar ni leer como notas
        (self.raiz / ".obsidian").mkdir()
        (self.raiz / ".obsidian" / "app.json").write_text('{"x": 1}', encoding="utf-8")
        (self.raiz / ".obsidian" / "plantilla.md").write_text("[[Ideas]] #oculta", encoding="utf-8")
        self.v = Vault(self.raiz)

    def tearDown(self):
        self._tmp.cleanup()

    def test_notas_ignora_obsidian(self):
        rutas_notas = sorted(n["ruta"] for n in self.v.notas())
        self.assertEqual(rutas_notas, ["Carpeta/Sub.md", "Ideas.md", "Inicio.md"])
        ideas = next(n for n in self.v.notas() if n["ruta"] == "Ideas.md")
        self.assertEqual(ideas["titulo"], "Ideas")
        self.assertEqual(ideas["alias"], ["Lluvia de ideas"])
        self.assertIsInstance(ideas["modificado"], float)
        self.assertNotIn("oculta", self.v.etiquetas())

    def test_etiquetas(self):
        self.assertEqual(self.v.etiquetas(), {"ideas": 1, "inicio": 1, "juegos/roblox": 1, "proyecto": 1})
        self.assertEqual(etiquetas_de("# Título\n#uno y #dos/tres, no #123 ni a#b ni http://x.mx/#ancla"),
                         ["uno", "dos/tres"])

    def test_frontmatter(self):
        frente, cuerpo = separar_frontmatter(IDEAS)
        self.assertEqual(frente["tags"], ["ideas", "juegos/roblox"])
        self.assertEqual(frente["aliases"], ["Lluvia de ideas"])
        self.assertTrue(cuerpo.lstrip().startswith("# Ideas"))
        self.assertEqual(separar_frontmatter("sin nada")[0], {})

    def test_enlaces_fuera_del_codigo(self):
        nombres = [e["nombre"] for e in enlaces_de(INICIO)]
        self.assertEqual(nombres, ["Ideas", "Ideas", "Fantasma", "foto.png"])
        e = enlaces_de(INICIO)[1]
        self.assertEqual((e["seccion"], e["texto"]), ("Juegos", "mis juegos"))
        self.assertTrue(enlaces_de(INICIO)[3]["incrustado"])

    def test_resolver_como_obsidian(self):
        for nombre in ("Ideas", "ideas", "Ideas.md", "Ideas#Juegos", "Ideas|otra cosa", "Lluvia de ideas",
                       "LLUVIA DE IDEAS"):
            self.assertEqual(self.v.resolver(nombre), "Ideas.md", nombre)
        self.assertEqual(self.v.resolver("Carpeta/Sub"), "Carpeta/Sub.md")
        self.assertEqual(self.v.resolver("Sub"), "Carpeta/Sub.md")
        self.assertIsNone(self.v.resolver("Fantasma"))
        self.assertIsNone(self.v.resolver("plantilla"))     # está en .obsidian

    def test_resolver_prefiere_la_misma_carpeta(self):
        (self.raiz / "Carpeta" / "Ideas.md").write_text("otra", encoding="utf-8")
        self.assertEqual(self.v.resolver("Ideas"), "Ideas.md")
        self.assertEqual(self.v.resolver("Ideas", desde="Carpeta/Sub.md"), "Carpeta/Ideas.md")

    def test_enlaces_y_retroenlaces(self):
        enlaces = {e["nombre"]: e for e in self.v.enlaces("Inicio.md")}
        self.assertEqual(enlaces["Ideas"]["ruta"], "Ideas.md")
        self.assertFalse(enlaces["Fantasma"]["existe"])
        self.assertIsNone(enlaces["Fantasma"]["ruta"])
        retro = {r["ruta"]: r for r in self.v.retroenlaces("Ideas.md")}
        self.assertEqual(set(retro), {"Inicio.md", "Carpeta/Sub.md"})     # Sub llega por el alias
        self.assertIn("[[Ideas]]", retro["Inicio.md"]["fragmento"])
        self.assertEqual({r["ruta"] for r in self.v.retroenlaces("Inicio.md")}, {"Ideas.md", "Carpeta/Sub.md"})

    def test_grafo(self):
        g = self.v.grafo()
        nodos = {n["id"]: n for n in g["nodos"]}
        self.assertEqual(set(nodos), {"Inicio.md", "Ideas.md", "Carpeta/Sub.md", "?Fantasma"})
        self.assertFalse(nodos["?Fantasma"]["existe"])
        aristas = {(a["origen"], a["destino"]) for a in g["aristas"]}
        self.assertEqual(aristas, {("Inicio.md", "Ideas.md"), ("Inicio.md", "?Fantasma"), ("Ideas.md", "Inicio.md"),
                                   ("Carpeta/Sub.md", "Ideas.md"), ("Carpeta/Sub.md", "Inicio.md")})
        self.assertEqual(len(g["aristas"]), len(aristas), "sin aristas repetidas")
        self.assertEqual(nodos["Ideas.md"]["grado"], 3)

    def test_renombrar_actualiza_enlaces(self):
        nueva = self.v.renombrar("Ideas.md", "Proyectos")
        self.assertEqual(nueva, "Proyectos.md")
        self.assertFalse((self.raiz / "Ideas.md").exists())
        inicio = self.v.leer("Inicio.md")
        self.assertIn("[[Proyectos]]", inicio)
        self.assertIn("[[Proyectos#Juegos|mis juegos]]", inicio)
        self.assertNotIn("[[Ideas", inicio)
        self.assertIn("[[NoEsEnlace]]", inicio)          # lo del código no se toca
        self.assertIn("[[Fantasma]]", inicio)
        self.assertIn("[[lluvia de ideas]]", self.v.leer("Carpeta/Sub.md"))   # el alias sigue sirviendo
        self.assertEqual(self.v.resolver("lluvia de ideas"), "Proyectos.md")
        self.assertEqual({r["ruta"] for r in self.v.retroenlaces("Proyectos.md")}, {"Inicio.md", "Carpeta/Sub.md"})

    def test_mover_a_carpeta(self):
        nueva = self.v.renombrar("Inicio.md", "Carpeta/Portada.md")
        self.assertEqual(nueva, "Carpeta/Portada.md")
        self.assertIn("[[Portada]]", self.v.leer("Ideas.md"))
        self.assertIn("[[Portada|principio]]", self.v.leer("Carpeta/Sub.md"))

    def test_renombrar_a_una_que_ya_existe(self):
        with self.assertRaises(FileExistsError):
            self.v.renombrar("Ideas.md", "Inicio")
        self.assertTrue((self.raiz / "Ideas.md").exists())

    def test_borrar_va_a_la_papelera(self):
        ruta_papelera = self.v.borrar("Ideas.md")
        self.assertEqual(ruta_papelera, ".trash/Ideas.md")
        self.assertTrue((self.raiz / ".trash" / "Ideas.md").exists())
        self.assertFalse((self.raiz / "Ideas.md").exists())
        self.assertNotIn("Ideas.md", [n["ruta"] for n in self.v.notas()])
        self.assertNotIn(".trash/Ideas.md", [n["ruta"] for n in self.v.notas()])
        # Otra con el mismo nombre no pisa la de la papelera
        ruta = self.v.crear("Ideas")
        self.assertEqual(self.v.borrar(ruta), ".trash/Ideas 1.md")
        self.assertEqual((self.raiz / ".obsidian" / "app.json").read_text(), '{"x": 1}')

    def test_crear_guardar_leer(self):
        self.assertEqual(self.v.crear("Inicio"), "Inicio 1.md")
        self.assertEqual(self.v.crear("Mi: nota/rara?", "Carpeta"), "Carpeta/Mi nota rara.md")
        self.assertEqual(self.v.crear(""), "Sin título.md")
        self.v.guardar("Nueva/Hoja.md", "hola #nueva")
        self.assertEqual(self.v.leer("Nueva/Hoja.md"), "hola #nueva")
        self.assertIn("nueva", self.v.etiquetas())
        self.assertEqual(list(self.raiz.rglob("*.tmp")), [], "no quedan temporales")
        self.assertEqual(self.v.crear_carpeta("Otra/Más"), "Otra/Más")

    def test_rutas_que_se_salen(self):
        for mala in ("../fuera.md", "/etc/passwd", "Carpeta/../../x.md", ""):
            with self.assertRaises(ErrorVault, msg=mala):
                self.v.leer(mala)
        with self.assertRaises(FileNotFoundError):
            self.v.leer("NoExiste.md")

    def test_buscar(self):
        res = self.v.buscar("cancion")
        self.assertEqual([r["ruta"] for r in res], ["Inicio.md"])
        self.assertIn("Canción", res[0]["fragmento"])
        self.assertEqual(self.v.buscar("IDEAS")[0]["ruta"], "Ideas.md", "primero el que lo tiene en el título")
        self.assertEqual([r["ruta"] for r in self.v.buscar("#juegos")], ["Ideas.md"])
        self.assertEqual([r["ruta"] for r in self.v.buscar("juego carreras")], ["Ideas.md"])
        self.assertEqual(self.v.buscar("zzzz"), [])
        self.assertEqual(self.v.buscar(""), [])

    def test_arbol(self):
        a = self.v.arbol()
        self.assertEqual((a["tipo"], a["ruta"]), ("carpeta", ""))
        nombres = [h["nombre"] for h in a["hijos"]]
        self.assertEqual(nombres, ["Carpeta", "Vacía", "Ideas.md", "Inicio.md"])
        self.assertEqual(a["hijos"][0]["hijos"][0]["ruta"], "Carpeta/Sub.md")
        self.assertEqual(a["hijos"][0]["hijos"][0]["titulo"], "Sub")
        self.assertNotIn(".obsidian", nombres)


# ====================================================================== Memoria

NOTAS_MEMORIA = {
    "Pizza.md": "# Pizza casera\n\nLa receta de pizza: harina, agua, levadura y sal. Hornear la pizza a 250 grados.\n",
    "Roblox.md": "# Roblox\n\nMi juego de carreras en Roblox usa Lua y tiene autos rápidos.\n",
    "Escuela/Mate.md": "# Matemáticas\n\nEl examen de fracciones es el lunes. Repasar fracciones y decimales.\n",
}


class PruebaMemoria(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.servidor = ServidorFalso()

    @classmethod
    def tearDownClass(cls):
        cls.servidor.cerrar()

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory(prefix="vault-memoria-")
        self.raiz = Path(self._tmp.name)
        for ruta, texto in NOTAS_MEMORIA.items():
            (self.raiz / ruta).parent.mkdir(parents=True, exist_ok=True)
            (self.raiz / ruta).write_text(texto, encoding="utf-8")
        self.v = Vault(self.raiz)
        self.ollama = Ollama(self.servidor.url, puede_arrancar=False)
        self.m = Memoria(self.v, self.ollama)        # sin modelo: lo debe encontrar («emb:latest»)
        self.m.borrar_indice()

    def tearDown(self):
        self.m.borrar_indice()
        self._tmp.cleanup()

    def test_indice_en_cache(self):
        self.assertTrue(str(self.m.ruta_indice).startswith(str(rutas.CACHE)))
        self.assertTrue(str(rutas.CACHE).startswith(_HOME_PRUEBAS))

    def test_incremental(self):
        progreso = []
        r = self.m.actualizar(al_progreso=lambda h, t, ruta: progreso.append((h, t, ruta)))
        self.assertEqual(self.m.modelo_embeddings, "emb:latest")
        self.assertEqual(self.m.modo, "embeddings")
        self.assertEqual(self.m.aviso, "")
        self.assertEqual((r["notas"], r["nuevas"], r["quitadas"]), (3, 3, 0))
        self.assertEqual(r["fragmentos"], 3)
        self.assertEqual(progreso[-1][:2], (3, 3))

        # Sin cambios: no vuelve a embeber nada
        antes = self.servidor.estado.textos_embebidos
        r = self.m.actualizar()
        self.assertEqual(r["nuevas"], 0)
        self.assertEqual(self.servidor.estado.textos_embebidos, antes)

        # Solo cambió la fecha (mismo contenido): el hash lo detecta y tampoco embebe
        futuro = time.time() + 30
        os.utime(self.raiz / "Pizza.md", (futuro, futuro))
        self.assertEqual(self.m.actualizar()["nuevas"], 0)
        self.assertEqual(self.servidor.estado.textos_embebidos, antes)

        # Cambió una: solo esa
        self.v.guardar("Roblox.md", "# Roblox\n\nAhora el juego es de aviones y pizza voladora.\n")
        r = self.m.actualizar()
        self.assertEqual(r["nuevas"], 1)
        self.assertEqual(self.servidor.estado.textos_embebidos, antes + 1)

    def test_nota_borrada_sale_del_indice(self):
        self.m.actualizar()
        self.v.borrar("Pizza.md")
        r = self.m.actualizar()
        self.assertEqual(r["quitadas"], 1)
        self.assertEqual(self.m.estado()["notas"], 2)
        self.assertNotIn("Pizza.md", [c["ruta"] for c in self.m.contexto("receta de pizza", k=6)])

    def test_contexto_y_mensajes(self):
        self.m.actualizar()
        ctx = self.m.contexto("¿cuál es la receta de la pizza?", k=2)
        self.assertEqual(ctx[0]["ruta"], "Pizza.md")
        self.assertEqual(ctx[0]["titulo"], "Pizza")
        for clave in ("ruta", "titulo", "fragmento", "puntaje"):
            self.assertIn(clave, ctx[0])
        self.assertGreater(ctx[0]["puntaje"], ctx[-1]["puntaje"] - 1e-9)
        self.assertEqual(self.m.contexto("fracciones del examen", k=1)[0]["ruta"], "Escuela/Mate.md")

        historial = [{"role": "user", "content": "hola"}, {"role": "assistant", "content": "¡Hola!", "pensamiento": "x"}]
        mensajes = self.m.mensajes_con_contexto("¿cómo hago pizza?", historial)
        self.assertEqual(mensajes[0]["role"], "system")
        self.assertIn("español", mensajes[0]["content"])
        self.assertIn("[[Pizza]]", mensajes[0]["content"])
        self.assertIn("harina", mensajes[0]["content"])
        self.assertEqual(mensajes[1:3], [{"role": "user", "content": "hola"}, {"role": "assistant", "content": "¡Hola!"}])
        self.assertEqual(mensajes[-1], {"role": "user", "content": "¿cómo hago pizza?"})
        self.assertEqual(self.m.ultimo_contexto[0]["ruta"], "Pizza.md")

    def test_cancelar(self):
        cancelar = threading.Event()
        cancelar.set()
        r = self.m.actualizar(cancelar=cancelar)
        self.assertTrue(r["cancelado"])
        self.assertEqual(r["nuevas"], 0)
        self.assertEqual(self.m.actualizar()["nuevas"], 3)    # después sigue donde se quedó

    def test_cambiar_de_modelo_rehace_todo(self):
        self.m.actualizar()
        otra = Memoria(self.v, self.ollama, modelo_embeddings="emb")
        self.assertEqual(otra.actualizar()["nuevas"], 3)

    def test_sin_embeddings_busca_por_palabras(self):
        caido = Ollama(f"http://127.0.0.1:{puerto_cerrado()}", puede_arrancar=False)
        m = Memoria(self.v, caido)
        self.assertEqual(m.actualizar()["nuevas"], 0)
        self.assertEqual(m.modo, "palabras")
        self.assertIn("por palabras", m.aviso)
        ctx = m.contexto("¿qué tiene mi juego de Roblox?")
        self.assertEqual(ctx[0]["ruta"], "Roblox.md")
        mensajes = m.mensajes_con_contexto("¿qué tiene mi juego de Roblox?", [])
        self.assertIn("[[Roblox]]", mensajes[0]["content"])
        self.assertIn("buscando las palabras", mensajes[0]["content"])
        self.assertEqual(m.contexto("de la el"), [])

    def test_fragmentar(self):
        largo = "# Uno\n\n" + "Una oración bastante normal. " * 80 + "\n\n## Dos\n\nCorto.\n"
        trozos = fragmentar("---\ntags: [x]\n---\n" + largo)
        self.assertTrue(all(len(t) <= 820 for _, t in trozos))
        self.assertEqual(trozos[0][0], "Uno")
        self.assertEqual(trozos[-1], ("Dos", "Corto."))
        self.assertNotIn("tags", " ".join(t for _, t in trozos))


# ====================================================================== Catálogo

class PruebaCatalogo(unittest.TestCase):
    def test_catalogo_completo(self):
        ids = [m["id"] for m in catalogo.CATALOGO]
        self.assertEqual(len(ids), len(set(ids)))
        sin_vault = [m for m in catalogo.CATALOGO if m["para"] != "vault"]
        self.assertTrue(12 <= len(sin_vault) <= 16)
        for m in catalogo.CATALOGO:
            for clave in ("id", "nombre", "descripcion", "para", "gb", "vram_gb"):
                self.assertIn(clave, m)
            self.assertIn(m["para"], catalogo.TIPOS)
            self.assertGreater(m["vram_gb"], m["gb"] * 0.9)
        self.assertEqual(catalogo.por_id(catalogo.EMBEDDINGS)["para"], "vault")
        self.assertEqual(catalogo.por_id("nomic-embed-text:latest")["id"], "nomic-embed-text")
        self.assertEqual({m["para"] for m in catalogo.CATALOGO}, set(catalogo.TIPOS))

    def test_cabe(self):
        rtx3080 = {"nombre": "RTX 3080", "total_mb": 10240, "libre_mb": 9000, "tipo": "nvidia", "ram_mb": 32768}
        self.assertEqual(catalogo.cabe("qwen3.5:9b", rtx3080), "si")
        self.assertEqual(catalogo.cabe("gemma4:12b", rtx3080), "justo")
        self.assertEqual(catalogo.cabe("qwen3.6:27b", rtx3080), "no")
        self.assertEqual(catalogo.cabe("gpt-oss:20b", rtx3080), "justo")    # moe: aguanta ir en parte en la CPU
        self.assertEqual(catalogo.cabe(catalogo.por_id("llama3.2:3b"), rtx3080), "si")
        self.assertEqual(catalogo.cabe({"bytes": 5_000_000_000}, rtx3080), "si")
        self.assertEqual(catalogo.cabe("no-existe:1b", rtx3080), "no")
        cpu = {"nombre": "CPU", "total_mb": 16384, "libre_mb": 8000, "tipo": "cpu", "ram_mb": 16384}
        self.assertEqual(catalogo.cabe("llama3.2:3b", cpu), "si")
        self.assertEqual(catalogo.cabe("gemma4:12b", cpu), "justo")
        self.assertEqual(catalogo.cabe("gpt-oss:20b", cpu), "no")

    def test_recomendado(self):
        def gpu(gb):
            return {"nombre": "x", "total_mb": gb * 1024, "libre_mb": gb * 1024, "tipo": "nvidia", "ram_mb": 16384}
        self.assertEqual(catalogo.recomendado(gpu(10))["id"], "qwen3.5:9b")
        self.assertEqual(catalogo.recomendado(gpu(12))["id"], "gemma4:12b")
        self.assertEqual(catalogo.recomendado(gpu(4))["id"], "qwen3.5:2b")
        self.assertEqual(catalogo.recomendado(gpu(1))["id"], "llama3.2:3b")    # ninguno cabe: el más chico
        self.assertEqual(catalogo.recomendado(gpu(32))["id"], "qwen3.6:35b-a3b")
        self.assertEqual(catalogo.recomendado(gpu(10))["para"], "chat")

    def test_recomendado_sin_grafica(self):
        def cpu(gb):
            return {"nombre": "CPU", "total_mb": gb * 1024, "libre_mb": gb * 1024, "tipo": "cpu", "ram_mb": gb * 1024}
        self.assertEqual(catalogo.recomendado(cpu(16))["id"], "qwen3.5:4b")   # uno ligero, aunque quepa uno mayor
        self.assertEqual(catalogo.recomendado(cpu(64))["id"], "qwen3.5:4b")
        self.assertEqual(catalogo.recomendado(cpu(8))["id"], "qwen3.5:2b")

    def _con_vortex_ia(self, salida: dict) -> dict:
        r = subprocess.CompletedProcess([], 0, stdout=json.dumps(salida), stderr="")
        with mock.patch.object(catalogo.shutil, "which", lambda n: "/x/" + n if n == "vortex-ia" else None), \
                mock.patch.object(catalogo.subprocess, "run", lambda *a, **k: r):
            return catalogo.vram_disponible()

    def test_tarjeta_que_se_queda_corta(self):
        for gb, entrada in ((2.0, True), (4.0, True), (2.0, False)):
            v = self._con_vortex_ia({"maquina": {"gpu": {"tipo": "amd", "nombre": "RX 550", "vram_gb": gb, "libre_gb": 1.0,
                                                         "de_entrada": entrada}}})
            self.assertEqual(v["tipo"], "cpu")
            self.assertTrue(v["de_entrada"])
            self.assertEqual(v["total_mb"], v["ram_mb"])

    def test_vortex_ia_buena_grafica(self):
        v = self._con_vortex_ia({"maquina": {"gpu": {"tipo": "amd", "nombre": "RX 6700 XT", "vram_gb": 12.0, "libre_gb": 11.0,
                                                     "de_entrada": False}}})
        self.assertEqual((v["tipo"], v["nombre"], v["total_mb"]), ("amd", "RX 6700 XT", 12288))

    def test_vram_de_sysfs_amd(self):
        with tempfile.TemporaryDirectory() as tmp:
            dispositivo = Path(tmp) / "card1" / "device"
            dispositivo.mkdir(parents=True)
            (dispositivo / "vendor").write_text("0x1002\n")
            (dispositivo / "mem_info_vram_total").write_text(str(8 * 1024 ** 3))
            (dispositivo / "mem_info_vram_used").write_text(str(1 * 1024 ** 3))
            (Path(tmp) / "card1-DP-1").mkdir()
            v = catalogo._de_sysfs(16384, 8000, Path(tmp))
            self.assertEqual((v["tipo"], v["total_mb"], v["libre_mb"]), ("amd", 8192, 7168))

    def test_vram_disponible_fija(self):
        os.environ["VORTEX_LOCAL_AI_VRAM_MB"] = "6144"
        try:
            v = catalogo.vram_disponible()
        finally:
            del os.environ["VORTEX_LOCAL_AI_VRAM_MB"]
        self.assertEqual(v["total_mb"], 6144)
        for clave in ("nombre", "total_mb", "libre_mb", "tipo"):
            self.assertIn(clave, v)


# ====================================================================== Conversaciones y rutas

class PruebaConversaciones(unittest.TestCase):
    def test_rutas_de_prueba(self):
        self.assertEqual(rutas.DATOS, Path(_HOME_PRUEBAS) / "datos")
        self.assertEqual(rutas.CACHE, Path(_HOME_PRUEBAS) / "cache")
        self.assertEqual(rutas.VAULT_POR_DEFECTO, Path(_HOME_PRUEBAS) / "vault")
        self.assertEqual(__version__, "1.0.0")

    def test_guardar_abrir_lista_borrar(self):
        conv = conversaciones.nueva("qwen3:8b")
        conv["mensajes"] = [{"role": "user", "content": "¿Cómo hago una pizza casera con queso y jamón?"},
                            {"role": "assistant", "content": "Así…", "pensamiento": "La persona quiere\nuna receta",
                             "citas": ["Recetas/Pizza.md"]}]
        id = conversaciones.guardar(conv)
        self.assertTrue((rutas.chats() / f"{id}.json").exists())
        abierta = conversaciones.abrir(id)
        self.assertEqual(abierta["mensajes"], conv["mensajes"])
        self.assertTrue(abierta["titulo"].startswith("Cómo hago una pizza"))
        lista = conversaciones.lista()
        self.assertIn(id, [c["id"] for c in lista])
        resumen = next(c for c in lista if c["id"] == id)
        self.assertEqual(resumen["n_mensajes"], 2)
        self.assertNotIn("mensajes", resumen)

        nota = conversaciones.a_nota(abierta)
        self.assertTrue(nota.startswith("---\ntags: [chat, vortex-local-ai]"))
        self.assertIn("## Tú", nota)
        self.assertIn("> [!note]- Pensamiento", nota)
        self.assertIn("> una receta", nota)
        self.assertIn("[[Pizza]]", nota)

        conversaciones.borrar(id)
        self.assertIsNone(conversaciones.abrir(id))
        conversaciones.borrar(id)   # dos veces no truena
        self.assertIsNone(conversaciones.abrir("../../etc/passwd"))
        with self.assertRaises(ValueError):
            conversaciones.borrar("../../etc/passwd")

    def test_titulo_automatico(self):
        self.assertEqual(conversaciones.titulo_automatico(""), "Chat nuevo")
        self.assertEqual(conversaciones.titulo_automatico("   "), "Chat nuevo")
        self.assertEqual(conversaciones.titulo_automatico("hola"), "Hola")
        self.assertEqual(conversaciones.titulo_automatico("Explícame [[Fracciones|las fracciones]] **ya**"),
                         "Explícame las fracciones ya")
        largo = conversaciones.titulo_automatico("uno dos tres cuatro cinco seis siete ocho")
        self.assertEqual(largo, "Uno dos tres cuatro cinco seis…")


if __name__ == "__main__":
    unittest.main()
