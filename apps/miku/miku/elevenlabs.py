"""Voz natural con ElevenLabs (opcional, en la nube): emociones de verdad a cambio de internet y una cuenta.

La voz la eliges tú en elevenlabs.io (Voice Library o Voice Design) y se usa por su «Voice ID». La clave
de la API se guarda en el llavero (secret-tool; el mismo que abre greetd al entrar) y, si no hay
llavero, en ~/.config/vortex/elevenlabs.clave con permisos 0600. Nunca va en miku.json.

Lo que Miku dice (el texto de cada frase, no tu voz) sale a ElevenLabs para convertirlo en audio. Con
el modelo eleven_v4_turbo (o eleven_v3) las etiquetas entre corchetes que pone Claude, como [sighs] o
[whispers], cambian cómo lo dice; para mostrarlo en pantalla se quitan (quitar_etiquetas).
Solo biblioteca estándar (y numpy en `sintetizar`): lo usa también el comando vortex-miku.
"""

import json
import os
import re
import shutil
import subprocess
import urllib.error
import urllib.request

from . import rutas

API = os.environ.get("VORTEX_MIKU_ELEVENLABS_API", "https://api.elevenlabs.io/v1")   # la variable es para pruebas
MODELO_DEFECTO = "eleven_v4_turbo"
MODELO_RESPALDO = "eleven_v3"            # por si la cuenta o la región aún no tiene el v4
TASA = 24000                             # pcm_24000: PCM de 16 bits mono, sin decodificar nada (44,1 kHz pide plan Pro)
ARCHIVO_CLAVE = rutas.CONFIG_DIR / "elevenlabs.clave"
_LLAVERO = ["service", "vortex-miku", "cuenta", "elevenlabs"]

# Etiquetas de audio que se le enseñan a Claude (en inglés, que es como mejor las entiende el modelo).
ETIQUETAS = ("sighs", "whispers", "sad", "softly", "laughs softly", "giggles", "excited", "curious",
             "nervous", "hesitates", "exhales", "happy", "surprised", "sarcastic")
# Lo que se le suma a la persona de Claude cuando la voz es ElevenLabs.
PERSONA = (
    "Tu voz es natural y expresiva: puedes poner etiquetas de emoción en inglés entre corchetes justo antes "
    "de la parte que afectan, por ejemplo [sighs], [whispers], [softly], [sad], [laughs softly], [nervous], "
    "[hesitates], [curious] o [excited]. Usa como mucho una o dos por respuesta y solo cuando de verdad se "
    "sienta esa emoción; no las expliques ni las escribas de otra forma. No se muestran en pantalla: solo "
    "cambian cómo suena tu voz."
)
_RE_ETIQUETA = re.compile(r"\[\s*([a-zA-Z][a-zA-Z ,'-]{1,30})\s*\](?!\()\s*")


class ErrorVoz(Exception):
    """Un problema con ElevenLabs, ya explicado para la persona."""


def quitar_etiquetas(texto: str) -> str:
    """El texto sin [etiquetas] de emoción (para el globo, el oído y Supertonic)."""
    return re.sub(r"\s{2,}", " ", _RE_ETIQUETA.sub("", texto)).strip()


def etiquetas_de(texto: str) -> list[str]:
    return [m.group(1).strip().lower() for m in _RE_ETIQUETA.finditer(texto)]


# ---- La clave -------------------------------------------------------------------------------------

def leer_clave() -> str:
    if os.environ.get("ELEVENLABS_API_KEY"):
        return os.environ["ELEVENLABS_API_KEY"].strip()
    if shutil.which("secret-tool"):
        try:
            r = subprocess.run(["secret-tool", "lookup", *_LLAVERO], capture_output=True, text=True, timeout=5)
            if r.returncode == 0 and r.stdout.strip():
                return r.stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            pass
    try:
        return ARCHIVO_CLAVE.read_text(encoding="utf-8").strip()
    except OSError:
        return ""


def guardar_clave(clave: str) -> str:
    """Guarda la clave y dice dónde quedó («llavero» o la ruta del archivo)."""
    clave = clave.strip()
    if shutil.which("secret-tool"):
        try:
            r = subprocess.run(["secret-tool", "store", "--label", "ElevenLabs (Miku de Vortexine's OS)", *_LLAVERO],
                               input=clave, capture_output=True, text=True, timeout=10)
            if r.returncode == 0:
                ARCHIVO_CLAVE.unlink(missing_ok=True)
                return "llavero"
        except (OSError, subprocess.TimeoutExpired):
            pass
    ARCHIVO_CLAVE.parent.mkdir(parents=True, exist_ok=True)
    fd = os.open(ARCHIVO_CLAVE, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        f.write(clave + "\n")
    return str(ARCHIVO_CLAVE)


def borrar_clave() -> None:
    if shutil.which("secret-tool"):
        try:
            subprocess.run(["secret-tool", "clear", *_LLAVERO], capture_output=True, timeout=5)
        except (OSError, subprocess.TimeoutExpired):
            pass
    ARCHIVO_CLAVE.unlink(missing_ok=True)


# ---- La API ---------------------------------------------------------------------------------------

def _explicar(codigo: int, cuerpo: bytes) -> str:
    try:
        detalle = json.loads(cuerpo or b"{}").get("detail", "")
        if isinstance(detalle, dict):
            detalle = detalle.get("message") or detalle.get("status") or ""
    except ValueError:
        detalle = ""
    detalle = str(detalle)[:200]
    if codigo == 401:
        return "ElevenLabs no aceptó la clave (¿la copiaste completa y tiene permiso de «Text to Speech»?). Ponla otra vez con: vortex-miku elevenlabs"
    if codigo == 402 or "quota" in detalle.lower() or "credits" in detalle.lower():
        return "Se acabaron los créditos de ElevenLabs de este mes."
    if codigo == 404:
        return "ElevenLabs no encuentra esa voz (revisa el Voice ID, y que la voz esté en «My Voices»)."
    if codigo == 429:
        return "ElevenLabs está ocupado o pediste demasiado seguido; prueba en un momento."
    return f"ElevenLabs respondió {codigo}" + (f": {detalle}" if detalle else "")


def pedir_audio(texto: str, voz_id: str, modelo: str, clave: str, timeout: float = 20.0) -> bytes:
    """PCM de 16 bits mono a 24 kHz de la frase. Lanza ErrorVoz con la explicación."""
    cuerpo = json.dumps({
        "text": texto,
        "model_id": modelo,
        "voice_settings": {"stability": 0.5, "similarity_boost": 0.75, "style": 0.0, "use_speaker_boost": True},
    }).encode("utf-8")
    req = urllib.request.Request(f"{API}/text-to-speech/{urllib.request.quote(voz_id, safe='')}?output_format=pcm_{TASA}",
                                 data=cuerpo, method="POST",
                                 headers={"xi-api-key": clave, "Content-Type": "application/json", "Accept": "audio/pcm"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        raise ErrorVoz(_explicar(e.code, e.read())) from None
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        raise ErrorVoz(f"No pude llegar a ElevenLabs (¿sin internet?): {getattr(e, 'reason', e)}") from None


def modelo_no_disponible(error: ErrorVoz) -> bool:
    t = str(error).lower()
    return "model" in t and ("not" in t or "invalid" in t or "access" in t)


class Voz:
    """La voz de ElevenLabs para voz_worker: misma forma que la de Supertonic (sintetizar → audio, tasa)."""

    def __init__(self, cfg: dict):
        self.voz_id = cfg.get("elevenlabs_voz", "").strip()
        self.modelo = cfg.get("elevenlabs_modelo", "").strip() or MODELO_DEFECTO
        self.clave = leer_clave()
        if not self.voz_id:
            raise ErrorVoz("Falta el Voice ID de ElevenLabs. Ponlo con: vortex-miku elevenlabs VOICE_ID")
        if not self.clave:
            raise ErrorVoz("Falta la clave de ElevenLabs. Ponla con: vortex-miku elevenlabs")

    def sintetizar(self, texto: str):
        import numpy as np
        texto = re.sub(r"\s+", " ", texto).strip()
        if not quitar_etiquetas(texto):
            raise ValueError("no hay nada que decir")
        try:
            pcm = pedir_audio(texto, self.voz_id, self.modelo, self.clave)
        except ErrorVoz as e:
            if self.modelo == MODELO_DEFECTO and modelo_no_disponible(e):
                self.modelo = MODELO_RESPALDO
                pcm = pedir_audio(texto, self.voz_id, self.modelo, self.clave)
            else:
                raise
        x = np.frombuffer(pcm[: len(pcm) // 2 * 2], dtype="<i2").astype(np.float32) / 32768.0
        if len(x) == 0:
            raise ErrorVoz("ElevenLabs devolvió un audio vacío.")
        return x, TASA
