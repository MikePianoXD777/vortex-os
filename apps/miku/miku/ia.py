"""Con qué IA piensa Miku («ia» en miku.json): Claude (claude -p, cerebro.py) o cualquiera compatible con OpenAI.

  claude   Claude Code con tu cuenta (como siempre).
  local    Ollama en tu PC, sin cuenta ni internet: el modelo (qwen3.5:4b, ~3,3 GB) se baja una vez a
           ~/.ollama. Usa la GPU por Vulkan (NVIDIA, AMD o Intel) y, si no hay, la CPU.
  gemini   Gemini de Google con tu clave de AI Studio (tiene capa gratis).
  openai   ChatGPT con tu clave de la API de OpenAI.
  otra     Cualquier servidor compatible con OpenAI (OpenRouter, Groq, LM Studio…): «ia_url» y «ia_modelo».

La clave va en el llavero (secret-tool), o si no hay llavero en ~/.config/vortex/ia-<proveedor>.clave (0600),
como la de ElevenLabs. Nunca va en miku.json.
"""

import json
import os
import shutil
import subprocess
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path

from . import rutas

OLLAMA = os.environ.get("VORTEX_MIKU_OLLAMA", "http://127.0.0.1:11434")   # la variable es para pruebas
MODELO_LOCAL = "qwen3.5:4b"
TAMANO_LOCAL = "3,3 GB"
# 4096 (lo que pone Ollama solo) se llena en una plática corta con las herramientas y una captura, y al llenarse
# Ollama corta el principio: las instrucciones de Miku. 8192 ocupa 3,4 GB de GPU con qwen3.5:4b (cabe en 4 GB);
# 16384, 3,7 GB. Va también en systemd/vortex-ollama.service.
CONTEXTO_LOCAL = "8192"
UNIDAD_OLLAMA = "vortex-ollama.service"     # unidad de usuario del paquete vortex-miku (no se activa sola)

PROVEEDORES = {
    "claude": {"nombre": "Claude"},
    "local": {"nombre": "IA local", "url": OLLAMA + "/v1", "modelo": MODELO_LOCAL, "clave": False,
              # Sin «pensar» en voz baja antes de contestar: en una GPU de 4 GB eso son segundos de silencio. Sin
              # pensar, Qwen recomienda temperatura 0.7 y top_p 0.8 (el modelo trae 1 y 0.95, las de pensar): con
              # esas mezclaba idiomas («Tóquio», «estábamosUsing») y leía mal los números de las herramientas.
              "extra": {"reasoning_effort": "none", "temperature": 0.7, "top_p": 0.8}},
    "gemini": {"nombre": "Gemini", "url": "https://generativelanguage.googleapis.com/v1beta/openai",
               "modelo": "gemini-flash-latest", "clave": True, "donde": "aistudio.google.com/apikey"},
    "openai": {"nombre": "ChatGPT", "url": "https://api.openai.com/v1", "modelo": "gpt-5-mini", "clave": True,
               "donde": "platform.openai.com/api-keys"},
    "otra": {"nombre": "tu IA", "url": "", "modelo": "", "clave": None},     # None = clave opcional
}


def elegido(cfg: dict) -> dict:
    """El proveedor de la config, con «url» y «modelo» ya resueltos (ia_url / ia_modelo mandan si vienen)."""
    nombre = cfg.get("ia") if cfg.get("ia") in PROVEEDORES else "claude"
    p = dict(PROVEEDORES[nombre], id=nombre)
    if cfg.get("ia_url") and nombre == "otra":
        p["url"] = cfg["ia_url"]
    if cfg.get("ia_modelo") and nombre != "claude":
        p["modelo"] = cfg["ia_modelo"]
    p["url"] = (p.get("url") or "").rstrip("/")
    return p


# ---- La clave -------------------------------------------------------------------------------------

def _llavero(proveedor: str) -> list[str]:
    return ["service", "vortex-miku", "cuenta", f"ia-{proveedor}"]


def archivo_clave(proveedor: str) -> Path:
    return rutas.CONFIG_DIR / f"ia-{proveedor}.clave"


def leer_clave(proveedor: str) -> str:
    if os.environ.get("VORTEX_MIKU_IA_CLAVE"):
        return os.environ["VORTEX_MIKU_IA_CLAVE"].strip()
    if shutil.which("secret-tool"):
        try:
            r = subprocess.run(["secret-tool", "lookup", *_llavero(proveedor)], capture_output=True, text=True, timeout=5)
            if r.returncode == 0 and r.stdout.strip():
                return r.stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            pass
    try:
        return archivo_clave(proveedor).read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def guardar_clave(proveedor: str, clave: str) -> str:
    """Guarda la clave y dice dónde quedó («llavero» o la ruta del archivo)."""
    clave = clave.strip()
    archivo = archivo_clave(proveedor)
    if shutil.which("secret-tool"):
        try:
            r = subprocess.run(["secret-tool", "store", "--label", f"{PROVEEDORES[proveedor]['nombre']} (Miku de Vortexine's OS)",
                                *_llavero(proveedor)], input=clave, capture_output=True, text=True, timeout=10)
            if r.returncode == 0:
                archivo.unlink(missing_ok=True)
                return "llavero"
        except (OSError, subprocess.TimeoutExpired):
            pass
    archivo.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(archivo, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(clave + "\n")
    return str(archivo)


def borrar_clave(proveedor: str) -> None:
    if shutil.which("secret-tool"):
        try:
            subprocess.run(["secret-tool", "clear", *_llavero(proveedor)], capture_output=True, timeout=5)
        except (OSError, subprocess.TimeoutExpired):
            pass
    archivo_clave(proveedor).unlink(missing_ok=True)


# ---- Ollama (la IA local) -------------------------------------------------------------------------

class ErrorIA(Exception):
    """Algo falló con la IA; el texto ya está en español y dice qué hacer. `clave` es para la ventana."""

    def __init__(self, clave: str, mensaje: str):
        super().__init__(mensaje)
        self.clave = clave


def _ollama_json(ruta: str, datos: dict | None = None, timeout: float = 5.0) -> dict:
    req = urllib.request.Request(OLLAMA + ruta, data=json.dumps(datos).encode() if datos is not None else None,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read() or b"{}")


def ollama_instalado() -> bool:
    return bool(shutil.which("ollama"))


def ollama_corriendo() -> bool:
    try:
        _ollama_json("/api/version", timeout=1.5)
        return True
    except (OSError, ValueError):
        return False


def arrancar_ollama(espera: float = 20.0) -> bool:
    """Deja corriendo Ollama para tu usuario (los modelos quedan en ~/.ollama). True si ya contesta.

    Primero la unidad de usuario del paquete (systemd la cuida y se puede parar); si no está, «ollama serve»
    aparte de Miku. Al rato sin usarse, Ollama suelta el modelo de la GPU solo (OLLAMA_KEEP_ALIVE).
    """
    if ollama_corriendo():
        return True
    if not ollama_instalado():
        return False
    try:
        r = subprocess.run(["systemctl", "--user", "start", UNIDAD_OLLAMA], capture_output=True, timeout=15)
        arrancada = r.returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        arrancada = False
    if not arrancada:
        env = dict(os.environ, OLLAMA_HOST=OLLAMA.split("://", 1)[-1], OLLAMA_VULKAN="1", OLLAMA_KEEP_ALIVE="10m",
                   OLLAMA_CONTEXT_LENGTH=CONTEXTO_LOCAL)
        try:
            subprocess.Popen(["ollama", "serve"], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                             stderr=subprocess.DEVNULL, start_new_session=True)
        except OSError:
            return False
    limite = time.monotonic() + espera
    while time.monotonic() < limite:
        if ollama_corriendo():
            return True
        time.sleep(0.5)
    return False


def modelo_local_listo(modelo: str = MODELO_LOCAL) -> bool:
    try:
        _ollama_json("/api/show", {"model": modelo})
        return True
    except (OSError, ValueError):
        return False


_precargado = 0.0


def precargar(modelo: str = MODELO_LOCAL) -> None:
    """Sube el modelo a la GPU aparte (la primera respuesta tardaba ~20 s en cargarlo). Si ya está, no tarda nada.

    La ventana lo llama cuando su voz ya cargó: la voz natural (PyTorch) necesita su memoria de GPU de una pieza y
    Ollama no (lo que no cabe lo deja en la CPU). Al revés, en una RTX 3080 la voz natural ya no cabía.
    """
    global _precargado
    if time.monotonic() - _precargado < 300:
        return
    _precargado = time.monotonic()

    def cargar():
        try:
            if arrancar_ollama() and modelo_local_listo(modelo):
                _ollama_json("/api/generate", {"model": modelo, "keep_alive": "10m"}, timeout=300)
        except (OSError, ValueError):
            pass
    threading.Thread(target=cargar, daemon=True).start()


def soltar(modelo: str = MODELO_LOCAL) -> None:
    """Saca el modelo de la GPU ya (al cerrar Miku: si no, sigue ahí 10 min y la voz natural no cabe al reabrirla)."""
    global _precargado
    _precargado = 0.0
    try:
        _ollama_json("/api/generate", {"model": modelo, "keep_alive": 0}, timeout=3)
    except (OSError, ValueError):
        pass


def preparar_local(modelo: str = MODELO_LOCAL) -> None:
    """Revisa que la IA local pueda contestar ya. Si no, ErrorIA con qué hacer («modelo»: falta bajarlo)."""
    if not ollama_instalado():
        raise ErrorIA("otro", "Me falta Ollama para pensar en tu PC. Actualiza Vortexine's en una terminal con: vortex actualizar")
    if not arrancar_ollama():
        raise ErrorIA("otro", "No pude arrancar la IA local (Ollama). Mira qué pasa con: systemctl --user status vortex-ollama")
    if not modelo_local_listo(modelo):
        raise ErrorIA("modelo", f"Para pensar sin internet necesito bajar mi IA local una sola vez ({TAMANO_LOCAL}).")


def bajar_modelo(modelo: str = MODELO_LOCAL, progreso=None, cancelar=None) -> None:
    """Baja el modelo con Ollama. `progreso(detalle, fraccion)`; `cancelar` es un threading.Event."""
    if not arrancar_ollama():
        raise ErrorIA("otro", "No pude arrancar la IA local (Ollama) para bajar el modelo.")
    req = urllib.request.Request(OLLAMA + "/api/pull", data=json.dumps({"model": modelo, "stream": True}).encode(),
                                 headers={"Content-Type": "application/json"})
    capas: dict[str, tuple[int, int]] = {}
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            for linea in r:
                if cancelar is not None and cancelar.is_set():
                    raise ErrorIA("cancelado", "Cancelé la descarga de la IA local.")
                try:
                    ev = json.loads(linea)
                except ValueError:
                    continue
                if ev.get("error"):
                    raise ErrorIA("otro", f"No pude bajar la IA local: {ev['error']}")
                if ev.get("digest") and ev.get("total"):
                    capas[ev["digest"]] = (int(ev.get("completed") or 0), int(ev["total"]))
                hecho = sum(c for c, _ in capas.values())
                total = sum(t for _, t in capas.values())
                if progreso and total:
                    progreso(f"{hecho / 1e9:.1f} de {total / 1e9:.1f} GB", min(hecho / total, 1.0))
                if ev.get("status") == "success":
                    if progreso:
                        progreso("Lista", 1.0)
                    return
    except urllib.error.HTTPError as e:
        raise ErrorIA("otro", f"Ollama no quiso bajar «{modelo}» (error {e.code}).") from None
    except (OSError, TimeoutError) as e:
        raise ErrorIA("internet", f"Se cortó la descarga de la IA local ({getattr(e, 'reason', e)}). Mira tu internet y vuelve a intentarlo.") from None
    raise ErrorIA("otro", "La descarga de la IA local terminó a medias. Vuelve a intentarlo.")


# ---- La API compatible con OpenAI -----------------------------------------------------------------

def _explicar(p: dict, codigo: int, cuerpo: str) -> ErrorIA:
    nombre = p["nombre"]
    if codigo in (401, 403):
        return ErrorIA("clave", f"{nombre} no aceptó tu clave. Ponla otra vez en una terminal con: vortex-miku ia clave")
    if codigo == 404 and p["id"] == "local":
        return ErrorIA("modelo", f"Para pensar sin internet necesito bajar mi IA local una sola vez ({TAMANO_LOCAL}).")
    if codigo == 404:
        return ErrorIA("otro", f"{nombre} no tiene el modelo «{p['modelo']}». Cámbialo con: vortex-miku ia {p['id']} MODELO")
    if codigo == 429:
        return ErrorIA("cuota", f"Ya usé todo el límite de {nombre} por ahora. Prueba otra vez en un rato.")
    if codigo >= 500:
        return ErrorIA("saturado", f"Hay demasiada gente usando {nombre} en este momento. Prueba otra vez en un rato.")
    return ErrorIA("otro", f"{nombre} contestó con un error ({codigo}): {cuerpo[:160]}")


def pedir(p: dict, cuerpo: dict, timeout: float, al_abrir=None):
    """POST /chat/completions. Devuelve la respuesta abierta (el que llama la lee y la cierra).

    `al_abrir(respuesta)` se llama en cuanto existe, para poder cerrarla desde otro hilo y cancelar.
    """
    cabeceras = {"Content-Type": "application/json", "User-Agent": "vortex-miku"}
    if p.get("clave") is not False:
        clave = leer_clave(p["id"])
        if clave:
            cabeceras["Authorization"] = f"Bearer {clave}"
    datos = dict(cuerpo, model=p["modelo"], **p.get("extra", {}))
    req = urllib.request.Request(p["url"] + "/chat/completions", data=json.dumps(datos).encode(), headers=cabeceras)
    try:
        r = urllib.request.urlopen(req, timeout=timeout)
    except urllib.error.HTTPError as e:
        try:
            texto = e.read().decode("utf-8", "replace")
        except OSError:
            texto = ""
        raise _explicar(p, e.code, texto) from None
    except (OSError, TimeoutError) as e:
        if p["id"] == "local":
            raise ErrorIA("otro", "La IA local no contesta. Mira que Ollama esté corriendo: systemctl --user status vortex-ollama") from None
        raise ErrorIA("internet", f"No pude llegar a {p['nombre']} ({getattr(e, 'reason', e)}). Mira tu internet.") from None
    if al_abrir:
        al_abrir(r)
    return r


def completar(p: dict, mensajes: list, timeout: float = 240.0) -> str:
    """Una respuesta entera, sin flujo ni herramientas (la usa momentos.py)."""
    with pedir(p, {"messages": mensajes, "stream": False}, timeout) as r:
        datos = json.loads(r.read())
    return ((datos.get("choices") or [{}])[0].get("message") or {}).get("content") or ""


def comprobar(p: dict) -> tuple[str, str]:
    """(«ok», "") si se puede hablar con este proveedor; si no, (clave, mensaje en español)."""
    if not p.get("url") or not p.get("modelo"):
        return "otro", "Me falta la dirección o el modelo de tu IA. Ponlos con: vortex-miku ia otra URL MODELO"
    if p.get("clave") and not leer_clave(p["id"]):
        return "clave", (f"Para hablar conmigo con {p['nombre']} necesito tu clave ({p.get('donde', '')}). "
                         "Ponla en una terminal con: vortex-miku ia clave")
    if p["id"] == "local":
        try:
            preparar_local(p["modelo"])
        except ErrorIA as e:
            return e.clave, str(e)
    return "ok", ""
