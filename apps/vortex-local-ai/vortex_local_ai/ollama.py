"""Cliente de Ollama: la IA que corre en tu PC. Solo con la biblioteca estándar (sin requests).

Los modelos viven en ~/.ollama y se comparten con Miku: si Ollama no está corriendo, primero se arranca
la unidad de usuario de Miku (vortex-ollama.service, del paquete vortex-miku) y, si no existe,
«ollama serve» como proceso aparte con la misma configuración.

Lo que va en streaming (bajar, chat) usa http.client para poder cortar la conexión al cancelar aunque el
servidor esté callado (si no, un read() bloqueado no se entera del cancelar hasta el siguiente trozo).
"""
from __future__ import annotations

import http.client
import json
import os
import shutil
import socket
import subprocess
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path
from typing import Callable, Iterator

URL_POR_DEFECTO = "http://127.0.0.1:11434"
UNIDAD = "vortex-ollama.service"   # la de Miku: apps/miku/systemd/vortex-ollama.service

# Igual que la unidad de Miku, para que los dos arranques se porten igual.
ENTORNO_SERVIDOR = {
    "OLLAMA_VULKAN": "1",
    "OLLAMA_KEEP_ALIVE": "10m",
    "OLLAMA_CONTEXT_LENGTH": "8192",
    "OLLAMA_NUM_PARALLEL": "1",
    "OLLAMA_MAX_LOADED_MODELS": "1",
}

# Dónde puede estar instalada la unidad de usuario.
_DIRS_UNIDADES = ("/usr/lib/systemd/user", "/etc/systemd/user", "~/.config/systemd/user",
                  "~/.local/share/systemd/user")

ESTADOS = {
    "pulling manifest": "Buscando el modelo",
    "verifying sha256 digest": "Revisando la descarga",
    "writing manifest": "Guardando",
    "removing any unused layers": "Guardando",
    "removing unused layers": "Guardando",
    "success": "Listo",
}


class ErrorOllama(Exception):
    """Un fallo de Ollama con un mensaje en español para enseñárselo a la persona.

    clave: "conexion" (no contesta), "no_instalado", "modelo" (no existe o no lo tienes), "red" (sin
    internet al bajar), "cancelado" u "otro".
    """

    def __init__(self, mensaje: str, clave: str = "otro"):
        super().__init__(mensaje)
        self.clave = clave


class Cancelado(ErrorOllama):
    """Se canceló a medias (con el threading.Event de cancelar)."""

    def __init__(self, mensaje: str = "Se canceló."):
        super().__init__(mensaje, "cancelado")


def _traducir(error: str, modelo: str = "") -> ErrorOllama:
    """Pasa un error de Ollama (en inglés) a un ErrorOllama claro."""
    bajo = error.lower()
    nombre = f"«{modelo}»" if modelo else "ese modelo"
    if "file does not exist" in bajo or "manifest unknown" in bajo or "pull model manifest" in bajo and "not found" in bajo:
        return ErrorOllama(f"No encontré {nombre} en la biblioteca de Ollama. Revisa que el nombre esté bien escrito.", "modelo")
    if "not found" in bajo and ("model" in bajo or modelo):
        return ErrorOllama(f"No tienes {nombre} en tu PC. Bájalo primero en «Modelos».", "modelo")
    if "does not support" in bajo and "embed" in bajo:
        return ErrorOllama(f"{nombre.capitalize()} no sirve para embeddings: elige uno para el vault, como nomic-embed-text.", "modelo")
    if "does not support thinking" in bajo:
        return ErrorOllama(f"{nombre.capitalize()} no sabe «pensar»: apaga esa opción.", "modelo")
    if "does not support" in bajo and ("image" in bajo or "vision" in bajo):
        return ErrorOllama(f"{nombre.capitalize()} no puede ver imágenes: elige un modelo de visión.", "modelo")
    if any(p in bajo for p in ("no such host", "dial tcp", "lookup", "network is unreachable", "tls", "i/o timeout",
                               "connection reset", "eof")):
        return ErrorOllama("No pude conectarme a internet para bajar el modelo. Revisa tu conexión y vuelve a intentar; "
                           "lo que ya se bajó no se pierde.", "red")
    if "no space left" in bajo:
        return ErrorOllama("Ya no hay espacio en el disco para el modelo. Borra algún modelo o libera espacio.", "otro")
    if "out of memory" in bajo or "cudamalloc" in bajo or "requires more system memory" in bajo:
        return ErrorOllama(f"{nombre.capitalize()} no cabe en la memoria de tu PC. Prueba con uno más chico.", "modelo")
    return ErrorOllama(f"Ollama dijo: {error}", "otro")


def _bytes(valor) -> int:
    try:
        return int(valor or 0)
    except (TypeError, ValueError):
        return 0


class _SeparadorPensar:
    """Separa lo que va dentro de <think>…</think> del resto, aunque las etiquetas lleguen partidas."""

    ABRE, CIERRA = "<think>", "</think>"

    def __init__(self):
        self.dentro = False
        self.pendiente = ""

    def meter(self, texto: str) -> list[tuple[bool, str]]:
        """Regresa [(es_pensamiento, texto), …] con lo que ya se puede soltar."""
        salida: list[tuple[bool, str]] = []
        buf = self.pendiente + texto
        self.pendiente = ""
        while buf:
            etiqueta = self.CIERRA if self.dentro else self.ABRE
            i = buf.find(etiqueta)
            if i >= 0:
                if i:
                    salida.append((self.dentro, buf[:i]))
                buf = buf[i + len(etiqueta):]
                self.dentro = not self.dentro
                continue
            # ¿El final podría ser el principio de la etiqueta? Guárdalo para el siguiente trozo.
            corte = len(buf)
            for n in range(min(len(etiqueta) - 1, len(buf)), 0, -1):
                if etiqueta.startswith(buf[-n:]):
                    corte = len(buf) - n
                    break
            if corte:
                salida.append((self.dentro, buf[:corte]))
            self.pendiente = buf[corte:]
            break
        return salida

    def terminar(self) -> list[tuple[bool, str]]:
        resto, self.pendiente = self.pendiente, ""
        return [(self.dentro, resto)] if resto else []


class Ollama:
    """Habla con el servidor de Ollama (por defecto http://127.0.0.1:11434)."""

    def __init__(self, url: str | None = None, *, puede_arrancar: bool | None = None):
        self.url = (url or os.environ.get("VORTEX_LOCAL_AI_OLLAMA") or URL_POR_DEFECTO).rstrip("/")
        if "://" not in self.url:
            self.url = "http://" + self.url
        if puede_arrancar is None:
            puede_arrancar = os.environ.get("VORTEX_LOCAL_AI_NO_ARRANCAR", "") in ("", "0")
        self.puede_arrancar = puede_arrancar
        self.ultimo_error = ""
        self.ultimas_estadisticas: dict = {}
        self._arranque: dict = {}
        partes = urllib.parse.urlsplit(self.url)
        self._https = partes.scheme == "https"
        self._host = partes.hostname or "127.0.0.1"
        self._puerto = partes.port or (443 if self._https else 11434)
        self._base = partes.path.rstrip("/")

    # ---------------------------------------------------------------- HTTP

    def _error_conexion(self) -> ErrorOllama:
        if not self.instalado() and self._es_local():
            return ErrorOllama("Ollama no está instalado. Actualiza Vortexine's con: vortex actualizar", "no_instalado")
        return ErrorOllama(f"No me pude conectar con Ollama ({self.url}). Puede que esté apagado: ábrelo de nuevo "
                           "o mira qué pasa con: systemctl --user status vortex-ollama", "conexion")

    def _json(self, ruta: str, datos: dict | None = None, metodo: str | None = None, timeout: float = 10.0,
              modelo: str = "") -> dict:
        cuerpo = json.dumps(datos).encode() if datos is not None else None
        req = urllib.request.Request(self.url + ruta, data=cuerpo, method=metodo,
                                     headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=timeout) as r:
                crudo = r.read()
        except urllib.error.HTTPError as e:
            raise self._error_http(e.code, e.read(), modelo) from None
        except (urllib.error.URLError, ConnectionError, socket.timeout, TimeoutError, OSError):
            raise self._error_conexion() from None
        try:
            return json.loads(crudo or b"{}")
        except ValueError:
            raise ErrorOllama("Ollama contestó algo que no entiendo.") from None

    def _error_http(self, codigo: int, cuerpo: bytes, modelo: str) -> ErrorOllama:
        try:
            mensaje = json.loads(cuerpo or b"{}").get("error") or ""
        except ValueError:
            mensaje = (cuerpo or b"").decode("utf-8", "replace").strip()
        if not mensaje:
            if codigo == 404:
                return _traducir("model not found", modelo)
            mensaje = f"error {codigo}"
        return _traducir(mensaje, modelo)

    def _stream(self, ruta: str, datos: dict, cancelar: threading.Event | None, timeout: float,
                modelo: str = "") -> Iterator[dict]:
        """POST que regresa una línea JSON por evento. Si `cancelar` se activa, corta la conexión."""
        clase = http.client.HTTPSConnection if self._https else http.client.HTTPConnection
        conexion = clase(self._host, self._puerto, timeout=timeout)
        fin = threading.Event()

        def vigilar():
            while not fin.is_set():
                if cancelar.wait(0.1):
                    try:
                        if conexion.sock is not None:
                            conexion.sock.shutdown(socket.SHUT_RDWR)
                    except OSError:
                        pass
                    return

        try:
            try:
                conexion.request("POST", self._base + ruta, body=json.dumps(datos).encode(),
                                 headers={"Content-Type": "application/json"})
                if cancelar is not None:
                    threading.Thread(target=vigilar, daemon=True).start()
                respuesta = conexion.getresponse()
            except (ConnectionError, socket.timeout, TimeoutError, OSError, http.client.HTTPException):
                if cancelar is not None and cancelar.is_set():
                    raise Cancelado() from None
                raise self._error_conexion() from None
            if respuesta.status >= 400:
                raise self._error_http(respuesta.status, respuesta.read(), modelo)
            while True:
                if cancelar is not None and cancelar.is_set():
                    raise Cancelado()
                try:
                    linea = respuesta.readline()
                except (ConnectionError, socket.timeout, TimeoutError, OSError, http.client.HTTPException,
                        ValueError):
                    if cancelar is not None and cancelar.is_set():
                        raise Cancelado() from None
                    raise ErrorOllama("Se cortó la conexión con Ollama a medias. Vuelve a intentar.", "conexion") from None
                if not linea:
                    if cancelar is not None and cancelar.is_set():
                        raise Cancelado()
                    return
                linea = linea.strip()
                if not linea:
                    continue
                try:
                    evento = json.loads(linea)
                except ValueError:
                    continue
                if evento.get("error"):
                    raise _traducir(str(evento["error"]), modelo)
                yield evento
        finally:
            fin.set()
            conexion.close()

    # ---------------------------------------------------------------- servidor

    def instalado(self) -> bool:
        """¿Está el comando ollama?"""
        return bool(shutil.which("ollama"))

    def _es_local(self) -> bool:
        return self._host in ("127.0.0.1", "localhost", "::1", "0.0.0.0")

    def disponible(self) -> bool:
        try:
            self._json("/api/version", timeout=1.5)
            return True
        except ErrorOllama:
            return False

    def version(self) -> str | None:
        try:
            return self._json("/api/version", timeout=1.5).get("version")
        except ErrorOllama:
            return None

    @staticmethod
    def _hay_unidad() -> bool:
        for carpeta in _DIRS_UNIDADES:
            if (Path(carpeta).expanduser() / UNIDAD).is_file():
                return True
        return False

    def asegurar_servidor(self, timeout: float = 20) -> bool:
        """Deja Ollama contestando. True si ya contesta o si lo pude arrancar a tiempo.

        Primero la unidad de usuario de Miku (vortex-ollama.service, mismo ~/.ollama); si no existe o falla,
        «ollama serve» como proceso aparte (sesión propia, sin atarse a esta app). Solo arranca algo si la
        URL es de esta PC. Si no se puede, `ultimo_error` dice por qué.
        """
        self.ultimo_error = ""
        if self.disponible():
            return True
        if not self.puede_arrancar:
            self.ultimo_error = "Ollama no contesta (y esta instancia no tiene permiso de arrancarlo)."
            return False
        if not self._es_local():
            self.ultimo_error = f"Ollama no contesta en {self.url} y no está en esta PC: no lo puedo arrancar."
            return False
        if not self.instalado():
            self.ultimo_error = "Ollama no está instalado. Actualiza Vortexine's con: vortex actualizar"
            return False
        arrancado = False
        # La unidad escucha en 127.0.0.1:11434: solo sirve si esa es nuestra URL.
        if self._puerto == 11434 and self._hay_unidad():
            try:
                r = subprocess.run(["systemctl", "--user", "start", UNIDAD], capture_output=True, timeout=15)
                arrancado = r.returncode == 0
            except (OSError, subprocess.TimeoutExpired):
                arrancado = False
            if arrancado:
                self._arranque["unidad"] = True
        if not arrancado:
            entorno = dict(os.environ, OLLAMA_HOST=f"{self._host}:{self._puerto}", **ENTORNO_SERVIDOR)
            try:
                self._arranque["proceso"] = subprocess.Popen(
                    ["ollama", "serve"], env=entorno, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL, start_new_session=True)
            except OSError as e:
                self.ultimo_error = f"No pude arrancar Ollama: {e}"
                return False
        limite = time.monotonic() + timeout
        while time.monotonic() < limite:
            if self.disponible():
                return True
            proceso = self._arranque.get("proceso")
            if proceso is not None and proceso.poll() is not None:
                self.ultimo_error = "Ollama se cerró al arrancar. Mira qué pasa con: ollama serve"
                return False
            time.sleep(0.3)
        self.ultimo_error = "Ollama tardó demasiado en arrancar. Mira qué pasa con: systemctl --user status vortex-ollama"
        return False

    def parar_propio(self) -> None:
        """Para el Ollama que arrancó esta instancia. Si ya estaba corriendo antes, no lo toca."""
        if self._arranque.pop("unidad", False):
            try:
                subprocess.run(["systemctl", "--user", "stop", "--no-block", UNIDAD], capture_output=True, timeout=5)
            except (OSError, subprocess.TimeoutExpired):
                pass
        proceso = self._arranque.pop("proceso", None)
        if proceso is not None and proceso.poll() is None:
            proceso.terminate()

    # ---------------------------------------------------------------- modelos

    def modelos(self) -> list[dict]:
        """Los modelos instalados (/api/tags)."""
        salida = []
        for m in self._json("/api/tags").get("models") or []:
            detalles = m.get("details") or {}
            nombre = m.get("name") or m.get("model") or ""
            familias = [str(f).lower() for f in (detalles.get("families") or [detalles.get("family") or ""])]
            salida.append({
                "nombre": nombre,
                "bytes": _bytes(m.get("size")),
                "familia": detalles.get("family") or "",
                "parametros": detalles.get("parameter_size") or "",
                "cuantizacion": detalles.get("quantization_level") or "",
                "modificado": m.get("modified_at") or "",
                "embeddings": "embed" in nombre.lower() or any("bert" in f for f in familias),
            })
        salida.sort(key=lambda m: m["nombre"])
        return salida

    def cargados(self) -> list[dict]:
        """Los modelos que están en memoria ahora (/api/ps)."""
        salida = []
        for m in self._json("/api/ps").get("models") or []:
            total, vram = _bytes(m.get("size")), _bytes(m.get("size_vram"))
            salida.append({
                "nombre": m.get("name") or m.get("model") or "",
                "bytes": total,
                "bytes_vram": vram,
                "en_gpu_pct": round(100 * vram / total) if total else 0,
                "expira": m.get("expires_at") or "",
            })
        return salida

    def bajar(self, nombre: str, al_progreso: Callable[[int, int, str], None] | None = None,
              cancelar: threading.Event | None = None) -> None:
        """Baja un modelo (/api/pull). al_progreso(completado, total, estado) con los bytes de todas las capas."""
        nombre = (nombre or "").strip()
        if not nombre:
            raise ErrorOllama("Falta el nombre del modelo.", "modelo")
        capas: dict[str, tuple[int, int]] = {}
        listo = False
        for evento in self._stream("/api/pull", {"model": nombre, "stream": True}, cancelar, timeout=120, modelo=nombre):
            estado_en = str(evento.get("status") or "")
            digest = evento.get("digest")
            if digest:
                total = _bytes(evento.get("total"))
                completado = _bytes(evento.get("completed"))
                anterior = capas.get(digest, (0, 0))
                capas[digest] = (max(completado, anterior[0]), max(total, anterior[1]))
                estado = "Bajando"
            else:
                estado = ESTADOS.get(estado_en, estado_en.capitalize() or "Bajando")
            if estado_en == "success":
                listo = True
            if al_progreso is not None:
                completado = sum(c for c, _ in capas.values())
                total = sum(t for _, t in capas.values())
                if listo:
                    completado = total
                al_progreso(completado, total, estado)
        if not listo:
            raise ErrorOllama(f"La descarga de «{nombre}» se cortó antes de terminar. Vuelve a intentar: "
                              "sigue donde se quedó.", "red")

    def borrar(self, nombre: str) -> None:
        """Borra un modelo instalado (/api/delete)."""
        self._json("/api/delete", {"model": nombre}, metodo="DELETE", modelo=nombre)

    def descargar_de_gpu(self, modelo: str) -> None:
        """Saca el modelo de la memoria ya (keep_alive=0), sin borrarlo."""
        try:
            self._json("/api/generate", {"model": modelo, "keep_alive": 0}, timeout=5, modelo=modelo)
        except ErrorOllama:
            pass

    # ---------------------------------------------------------------- chat y embeddings

    def chat(self, modelo: str, mensajes: list[dict], al_trozo: Callable[[str], None] | None = None,
             cancelar: threading.Event | None = None, opciones: dict | None = None,
             al_pensar: Callable[[str], None] | None = None) -> str:
        """Platica con un modelo (/api/chat) en streaming. Regresa la respuesta sin el pensamiento.

        al_trozo(texto) recibe cada trozo de la respuesta; al_pensar(texto) cada trozo del pensamiento
        (<think>…</think> o el campo «thinking»). Si se cancela, regresa lo que llevaba.
        """
        datos: dict = {"model": modelo, "messages": mensajes, "stream": True}
        if opciones:
            datos["options"] = dict(opciones)
        respuesta: list[str] = []
        separador = _SeparadorPensar()
        self.ultimas_estadisticas = {}

        def soltar(partes):
            for es_pensamiento, texto in partes:
                if not texto:
                    continue
                if es_pensamiento:
                    if al_pensar is not None:
                        al_pensar(texto)
                else:
                    respuesta.append(texto)
                    if al_trozo is not None:
                        al_trozo(texto)

        try:
            for evento in self._stream("/api/chat", datos, cancelar, timeout=600, modelo=modelo):
                mensaje = evento.get("message") or {}
                pensamiento = mensaje.get("thinking")
                if pensamiento and al_pensar is not None:
                    al_pensar(pensamiento)
                if mensaje.get("content"):
                    soltar(separador.meter(mensaje["content"]))
                if evento.get("done"):
                    tokens = _bytes(evento.get("eval_count"))
                    segundos = _bytes(evento.get("eval_duration")) / 1e9
                    self.ultimas_estadisticas = {
                        "tokens": tokens, "segundos": round(segundos, 2),
                        "tokens_por_segundo": round(tokens / segundos, 1) if segundos else 0.0,
                    }
        except Cancelado:
            pass
        soltar(separador.terminar())
        return "".join(respuesta).strip()

    def embeber(self, modelo: str, textos: list[str] | str) -> list[list[float]]:
        """Los vectores de cada texto (/api/embed; si el Ollama es viejo, /api/embeddings uno por uno)."""
        if isinstance(textos, str):
            textos = [textos]
        if not textos:
            return []
        try:
            datos = self._json("/api/embed", {"model": modelo, "input": list(textos)}, timeout=300, modelo=modelo)
            vectores = datos.get("embeddings")
            if isinstance(vectores, list) and len(vectores) == len(textos):
                return [[float(x) for x in v] for v in vectores]
            raise ErrorOllama("Ollama no regresó los embeddings.")
        except ErrorOllama as e:
            if e.clave != "modelo" or "embeddings" in str(e).lower():
                raise
            # Un 404 puede ser «no tienes el modelo» o un Ollama sin /api/embed: prueba la ruta vieja.
            try:
                return [[float(x) for x in self._json("/api/embeddings", {"model": modelo, "prompt": t}, timeout=300,
                                                       modelo=modelo).get("embedding") or []]
                        for t in textos]
            except ErrorOllama:
                raise e from None
