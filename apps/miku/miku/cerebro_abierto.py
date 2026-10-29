"""El cerebro de Miku con cualquier IA compatible con OpenAI (la local con Ollama, Gemini, ChatGPT…).

Hace lo que con Claude hace `claude -p`: lleva la conversación, levanta los mismos servidores MCP (la vista
de vortex-mcp, solo ver_pantalla, ventana_activa, ventanas y estado_sistema, y las acciones de
acciones_mcp.py, que piden tu permiso en la ventana) y corre las herramientas que pida la IA, hasta 8
vueltas por pregunta. La IA no tiene nada más: ni comandos, ni archivos, ni internet. Las capturas de
pantalla le llegan como imagen (si el modelo no ve imágenes, se le dice). La conversación vive solo en
memoria y se olvida al cerrar a Miku.

Avisa con los mismos eventos que cerebro.Cerebro, así el asistente no distingue entre uno y otro.
"""

import json
import os
import queue
import socket
import subprocess
import sys
import threading
import time

from . import cerebro, ia

VUELTAS = 8
MENSAJES_MAXIMOS = 40               # historial que se le manda (los más viejos se olvidan)
TIEMPO_LECTURA = {"local": 120.0}   # la primera vez Ollama carga el modelo en la GPU
TIEMPO_LECTURA_DEFECTO = 60.0
TIEMPO_HERRAMIENTA = 300.0          # una acción espera hasta 2 min tu permiso y luego corre
SIN_IMAGENES = "(No puedo ver imágenes con este modelo: la captura no me llegó.)"


class ServidorMCP:
    """Un servidor MCP por stdio (JSON-RPC por líneas), como los que arranca Claude Code."""

    def __init__(self, nombre: str, comando: list[str], env: dict | None = None):
        self.nombre = nombre
        self.proc = subprocess.Popen(comando, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                     text=True, bufsize=1, env=env)
        self._id = 0
        self._esperas: dict[int, queue.Queue] = {}
        self._lock = threading.Lock()
        threading.Thread(target=self._leer, daemon=True).start()
        self.pedir("initialize", {"protocolVersion": "2025-06-18", "capabilities": {},
                                  "clientInfo": {"name": "vortex-miku", "version": "1.6"}}, timeout=15)
        self._mandar({"jsonrpc": "2.0", "method": "notifications/initialized"})
        self.herramientas = self.pedir("tools/list", {}, timeout=15).get("tools", [])

    def vivo(self) -> bool:
        return self.proc.poll() is None

    def _mandar(self, msg: dict) -> None:
        with self._lock:
            self.proc.stdin.write(json.dumps(msg) + "\n")
            self.proc.stdin.flush()

    def _leer(self) -> None:
        for linea in self.proc.stdout:
            try:
                msg = json.loads(linea)
            except ValueError:
                continue
            espera = self._esperas.pop(msg.get("id"), None) if isinstance(msg, dict) else None
            if espera is not None:
                espera.put(msg)
        for espera in list(self._esperas.values()):      # se cayó: que nadie se quede esperando
            espera.put({"error": {"message": "el servidor se cerró"}})

    def pedir(self, metodo: str, params: dict, timeout: float) -> dict:
        with self._lock:
            self._id += 1
            id_ = self._id
        espera: queue.Queue = queue.Queue()
        self._esperas[id_] = espera
        try:
            self._mandar({"jsonrpc": "2.0", "id": id_, "method": metodo, "params": params})
            msg = espera.get(timeout=timeout)
        except (OSError, ValueError, queue.Empty):
            self._esperas.pop(id_, None)
            raise RuntimeError(f"{self.nombre} no contestó") from None
        if "error" in msg:
            raise RuntimeError((msg["error"] or {}).get("message") or "error")
        return msg.get("result") or {}

    def cerrar(self) -> None:
        if self.vivo():
            self.proc.terminate()
            try:
                self.proc.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def _cortar(respuesta) -> None:
    """Corta una respuesta HTTP desde otro hilo (cancelar): cerrar el socket despierta al que lee."""
    try:
        respuesta.fp.raw._sock.shutdown(socket.SHUT_RDWR)
    except (AttributeError, OSError):
        pass
    try:
        respuesta.close()
    except OSError:
        pass


def _sin_imagenes(mensajes: list) -> list:
    """Los mensajes con las imágenes cambiadas por un texto (para el historial, o si el modelo no ve)."""
    limpios = []
    for m in mensajes:
        if isinstance(m.get("content"), list):
            textos = [c.get("text", "") for c in m["content"] if c.get("type") == "text"]
            if any(c.get("type") == "image_url" for c in m["content"]):
                textos.append("[captura de pantalla]")
            m = dict(m, content=" ".join(t for t in textos if t))
        limpios.append(m)
    return limpios


DIAS = ("lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo")
MESES = ("enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre",
         "noviembre", "diciembre")


def _hoy() -> str:
    """La fecha y la hora, que Claude Code ya sabe y estas IA no («¿qué día es hoy?»)."""
    t = time.localtime()
    return (f"\n\nHoy es {DIAS[t.tm_wday]} {t.tm_mday} de {MESES[t.tm_mon - 1]} de {t.tm_year}, "
            f"y son las {t.tm_hour}:{t.tm_min:02d}.")


def _recortar(historial: list) -> list:
    """Los últimos mensajes, empezando siempre en una pregunta (sin cortar una llamada a medias)."""
    if len(historial) <= MENSAJES_MAXIMOS:
        return historial
    resto = historial[-MENSAJES_MAXIMOS:]
    while resto and resto[0].get("role") != "user":
        resto = resto[1:]
    return resto


class CerebroAbierto:
    """Mismo trato que cerebro.Cerebro: `preguntar()` corre en un hilo y avisa por `al_evento(tipo, …)`."""

    def __init__(self, avatar: bool, al_evento, cfg: dict, servidores: list | None = None):
        self.avatar = avatar
        self.al_evento = al_evento
        self.proveedor = ia.elegido(cfg)
        self.persona = cerebro.leer_persona(avatar)
        self.sentimientos = ""                  # cómo se siente ahora (corazon.contexto, lo pone el asistente)
        self.autorizar: str | None = None       # socket del permiso de las acciones (lo pone el asistente)
        self.internet = False                   # la búsqueda web es de Claude: aquí no hay
        self.ocupado = threading.Event()
        self.historial: list = []
        self._servidores_fijos = servidores     # (nombre, comando, env) — las pruebas ponen los suyos
        self._servidores: list[ServidorMCP] = []
        self._respuesta = None
        self._token: threading.Event | None = None
        self._lock = threading.Lock()
        self.mcp = cerebro.encontrar_mcp()
        if not self.mcp and servidores is None:
            self.persona += "\n\nEn esta sesión no tienes acceso a la pantalla: si te piden mirarla, explica que no puedes."

    # ---- Lo que usa el asistente ----------------------------------------------------------------

    def comprobar(self) -> tuple[str, str]:
        return ia.comprobar(self.proveedor)

    def cancelar(self) -> None:
        if self._token is not None:
            self._token.set()
        with self._lock:
            r = self._respuesta
        if r is not None:
            _cortar(r)

    def olvidar(self) -> None:
        self.cancelar()
        self.historial = []
        for s in self._servidores:
            s.cerrar()
        self._servidores = []

    def preguntar(self, texto: str, etiqueta=None, reintento: bool = False) -> None:
        token = self._token = threading.Event()

        def avisar(tipo, **datos):
            if not token.is_set():
                self.al_evento(tipo, etiqueta=etiqueta, **datos)

        clave, mensaje = self.comprobar()
        if clave != "ok":
            avisar("error", clave=clave, mensaje=mensaje)
            return
        try:
            herramientas, rutas_h = self._herramientas()
        except (OSError, RuntimeError):
            herramientas, rutas_h = [], {}          # sin herramientas también se puede platicar
        nuevos = [{"role": "user", "content": texto}]
        avisar("inicio")
        texto_total = ""
        ve_imagenes = True
        try:
            for _ in range(VUELTAS):
                mensajes = [{"role": "system", "content": self.persona + self.sentimientos + _hoy()}] + self.historial + nuevos
                if not ve_imagenes:
                    mensajes = _sin_imagenes(mensajes)
                try:
                    contenido, llamadas = self._una_vuelta(mensajes, herramientas, avisar, token)
                except ia.ErrorIA as e:
                    hay_imagenes = any(isinstance(m.get("content"), list) for m in nuevos)
                    if ve_imagenes and hay_imagenes and e.clave == "otro":
                        ve_imagenes = False          # el modelo no ve imágenes: otra vez, sin ellas
                        for m in nuevos:
                            if isinstance(m.get("content"), list):
                                m["content"] = SIN_IMAGENES
                        continue
                    raise
                if token.is_set():
                    return
                texto_total += contenido
                if not llamadas:
                    nuevos.append({"role": "assistant", "content": contenido})
                    break
                nuevos.append({"role": "assistant", "content": contenido or None, "tool_calls": llamadas})
                imagenes = []
                for ll in llamadas:
                    nombre = ll["function"]["name"]
                    avisar("herramienta", nombre=rutas_h.get(nombre, (None, None, nombre))[2])
                    resultado, imgs = self._correr(rutas_h, nombre, ll["function"].get("arguments") or "{}")
                    if token.is_set():
                        return
                    nuevos.append({"role": "tool", "tool_call_id": ll["id"], "content": resultado})
                    imagenes += imgs
                if imagenes:
                    partes = [{"type": "text", "text": "La captura de pantalla que pediste:"}]
                    partes += [{"type": "image_url", "image_url": {"url": f"data:{mime};base64,{datos}"}}
                               for mime, datos in imagenes]
                    nuevos.append({"role": "user", "content": partes if ve_imagenes else SIN_IMAGENES})
            else:
                if not texto_total:
                    texto_total = "Me enredé con las herramientas y no terminé. ¿Me lo pides otra vez?"
                    avisar("texto", delta=texto_total)
                    avisar("bloque_fin")
        except ia.ErrorIA as e:
            avisar("error", clave=e.clave, mensaje=str(e))
            return
        if token.is_set():
            return
        self.historial = _recortar(self.historial + _sin_imagenes(nuevos))
        avisar("listo", texto=texto_total)

    # ---- Por dentro -----------------------------------------------------------------------------

    def _herramientas(self) -> tuple[list, dict]:
        """Las herramientas en formato OpenAI y de dónde sale cada una: {nombre: (servidor, herramienta, mcp__…)}."""
        if not self._servidores or not all(s.vivo() for s in self._servidores):
            for s in self._servidores:
                s.cerrar()
            self._servidores = []
            if self._servidores_fijos is not None:
                definiciones = self._servidores_fijos
            else:
                definiciones = []
                if self.mcp:
                    definiciones.append(("vortex", [self.mcp], None))
                if self.autorizar:
                    definiciones.append(("miku", [sys.executable, "-I", str(cerebro.SERVIDOR_ACCIONES)],
                                         dict(os.environ, VORTEX_MIKU_AUTORIZAR=self.autorizar)))
            for nombre, comando, env in definiciones:
                self._servidores.append(ServidorMCP(nombre, comando, env))
        permitidas = {h.removeprefix("mcp__vortex__") for h in cerebro.HERRAMIENTAS_PERMITIDAS}
        lista, rutas_h = [], {}
        for s in self._servidores:
            for h in s.herramientas:
                if s.nombre == "vortex" and h["name"] not in permitidas:
                    continue                        # portapapeles e historial de la terminal: nunca
                nombre = f"{s.nombre}__{h['name']}"
                rutas_h[nombre] = (s, h["name"], f"mcp__{s.nombre}__{h['name']}")
                lista.append({"type": "function", "function": {
                    "name": nombre, "description": h.get("description", ""),
                    "parameters": h.get("inputSchema") or {"type": "object", "properties": {}}}})
        return lista, rutas_h

    def _correr(self, rutas_h: dict, nombre: str, argumentos: str) -> tuple[str, list]:
        """Corre una herramienta. (texto para la IA, [(mime, base64), …] de imágenes)."""
        if nombre not in rutas_h:
            return f"No existe la herramienta {nombre}.", []
        servidor, herramienta, _ = rutas_h[nombre]
        try:
            args = json.loads(argumentos) if argumentos.strip() else {}
        except ValueError:
            return "Los argumentos no eran JSON válido.", []
        try:
            res = servidor.pedir("tools/call", {"name": herramienta, "arguments": args if isinstance(args, dict) else {}},
                                 timeout=TIEMPO_HERRAMIENTA)
        except RuntimeError as e:
            return f"La herramienta falló: {e}", []
        textos, imagenes = [], []
        for c in res.get("content") or []:
            if c.get("type") == "text":
                textos.append(c.get("text", ""))
            elif c.get("type") == "image" and c.get("data"):
                imagenes.append((c.get("mimeType") or "image/png", c["data"]))
        texto = "\n".join(t for t in textos if t) or ("Listo: te mando la imagen." if imagenes else "Listo.")
        if res.get("isError"):
            texto = "Error: " + texto
        return texto, imagenes

    def _una_vuelta(self, mensajes: list, herramientas: list, avisar, token) -> tuple[str, list]:
        """Una respuesta de la IA en flujo: avisa del texto y devuelve (texto, llamadas a herramientas)."""
        cuerpo = {"messages": mensajes, "stream": True}
        if herramientas:
            cuerpo["tools"] = herramientas

        def guardar(r):
            with self._lock:
                self._respuesta = r
        timeout = TIEMPO_LECTURA.get(self.proveedor["id"], TIEMPO_LECTURA_DEFECTO)
        r = ia.pedir(self.proveedor, cuerpo, timeout, al_abrir=guardar)
        contenido, llamadas = "", {}
        try:
            for linea in r:
                if token.is_set():
                    break
                linea = linea.decode("utf-8", "replace").strip()
                if not linea.startswith("data:"):
                    continue
                dato = linea[5:].strip()
                if dato == "[DONE]":
                    break
                try:
                    ev = json.loads(dato)
                except ValueError:
                    continue
                if ev.get("error"):
                    raise ia.ErrorIA("otro", f"{self.proveedor['nombre']} contestó con un error: "
                                     f"{(ev['error'] or {}).get('message', ev['error'])}")
                delta = ((ev.get("choices") or [{}])[0].get("delta")) or {}
                if delta.get("content"):
                    contenido += delta["content"]
                    avisar("texto", delta=delta["content"])
                for t in delta.get("tool_calls") or []:
                    i = t.get("index", len(llamadas))
                    ll = llamadas.setdefault(i, {"id": "", "type": "function", "function": {"name": "", "arguments": ""}})
                    ll["id"] = t.get("id") or ll["id"]
                    f = t.get("function") or {}
                    ll["function"]["name"] += f.get("name") or ""
                    a = f.get("arguments")
                    ll["function"]["arguments"] += a if isinstance(a, str) else (json.dumps(a) if a else "")
        except (OSError, ValueError) as e:
            if not token.is_set():
                raise ia.ErrorIA("internet", f"Se cortó la respuesta de {self.proveedor['nombre']} ({e}).") from None
        finally:
            with self._lock:
                self._respuesta = None
            try:
                r.close()
            except OSError:
                pass
        if contenido:
            avisar("bloque_fin")
        lista = []
        for i, ll in sorted(llamadas.items()):
            if ll["function"]["name"]:
                ll["id"] = ll["id"] or f"llamada_{i}"
                lista.append(ll)
        return contenido, lista
