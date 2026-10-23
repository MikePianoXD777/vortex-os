"""La voz de Miku: Supertonic 3 (Supertone) sobre onnxruntime, en la propia máquina y sin internet.

Se usa desde voz_worker.py, el proceso aparte que carga el modelo una sola vez. Necesita numpy y
onnxruntime (el paquete trae el suyo en /usr/lib/vortex-miku/python) y los modelos de ~/.local/share/
vortex/miku/voz/, que se bajan la primera vez (descargas.py).

La inferencia es un port compacto de supertonic-py 1.3.1 (licencia MIT, © 2025 Supertone Inc.; el texto
está en licencias/supertonic-MIT.txt). Los pesos del modelo son BigScience OpenRAIL-M (licencias/
BigScience-OpenRAIL-M.txt): se pueden usar y redistribuir con esa licencia, que prohíbe, entre otras cosas,
suplantar a una persona con una voz sintética o engañar sobre que el audio es de una IA. Miku dice
siempre que es una asistente y su voz es una voz preestablecida del modelo, no la de una persona real.
"""

import json
import os
import re
import sys
import unicodedata
import zlib

from . import audio, rutas

IDIOMA = "es"
PASOS = 8                  # pasos de difusión (más = mejor y más lento); el valor de fábrica del SDK
MAX_TROZO = 300            # caracteres por pasada del modelo; una frase más larga se parte
SILENCIO_TROZOS = 0.3      # segundos entre trozos de una misma frase

# Quién habla cuando la ventana es el asistente a secas (sin avatar): la voz F1 tal cual, clara.
ESTILO_NORMAL = {"tono": 0, "velocidad": 1.05, "corte": 0, "estante": 0.0, "nivel": -20.0, "compresion": 2.2}

# Miku Nakano en el anime, según Mike: «apagada y ligeramente grave, medio su voz depre». Es el ajuste
# de fábrica con avatar: más grave que la F1 normal (~196 Hz → ~170 Hz), más lenta, con menos brillo y
# a volumen bajo y parejo. «tono» son centésimas de semitono (negativo = más grave); como se hace
# remuestreando, también la vuelve más lenta (-250 centésimas = 15 % más despacio) y de timbre más
# ancho, que es justo lo que se busca; por eso la velocidad del motor sube un poco (1,05 es la normal)
# para que la voz quede ~10 % más lenta y no arrastrada.
ESTILO_MIKU = {"tono": -250, "velocidad": 1.08, "corte": 7500, "estante": -3.0, "nivel": -26.0, "compresion": 3.0}


def preparar_entorno() -> None:
    """Pone la onnxruntime vendorizada del paquete al alcance (gana a la del sistema, si hay una)."""
    libs = str(rutas.LIBS_PY)
    if os.path.isdir(libs) and libs not in sys.path:
        sys.path.insert(0, libs)


# ---- Texto -----------------------------------------------------------------------------------------

def texto_hablado(texto: str) -> str:
    """Deja el texto listo para el sintetizador: sin markdown, enlaces ni emojis."""
    t = re.sub(r"```.*?```", " ", texto, flags=re.S)
    t = re.sub(r"`([^`]*)`", r"\1", t)
    t = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", t)
    t = re.sub(r"https?://\S+", "un enlace", t)
    t = re.sub(r"[*_#>~|]+", " ", t)
    t = re.sub(r"^\s*[-•]\s+", "", t, flags=re.M)
    # Fuera emojis y símbolos raros (conserva letras, números y la puntuación del español).
    t = "".join(c for c in t if unicodedata.category(c)[0] in "LNPZ" or c in "\n")
    t = t.replace("…", "...")
    return re.sub(r"\s+", " ", t).strip()


_SIMBOLOS = {"–": "-", "‑": "-", "—": "-", "¯": " ", "_": " ", "“": '"', "”": '"',
             "‘": "'", "’": "'", "´": "'", "`": "'", "[": " ", "]": " ", "|": " ", "/": " ", "#": " ",
             "→": " ", "←": " "}
_ESPACIO_PUNTUACION = [(re.compile(r" ,"), ","), (re.compile(r" \."), "."), (re.compile(r" !"), "!"),
                       (re.compile(r" \?"), "?"), (re.compile(r" ;"), ";"), (re.compile(r" :"), ":"), (re.compile(r" '"), "'")]
_COMILLAS_REPETIDAS = re.compile(r'(["\'`])\1+')
_FINAL = re.compile(r"[.!?;:,'\")\]}…。」』】〉》›»]$")
_FIN_FRASE = re.compile(r"(?<=[.!?…])\s+")


def preparar_texto(texto: str) -> str:
    """Normalización del texto que espera el modelo (las mismas reglas que el SDK de Supertone)."""
    t = unicodedata.normalize("NFKD", texto)
    for viejo, nuevo in _SIMBOLOS.items():
        t = t.replace(viejo, nuevo)
    t = re.sub(r"[♥☆♡©\\]", "", t)
    for patron, cambio in _ESPACIO_PUNTUACION:
        t = patron.sub(cambio, t)
    t = _COMILLAS_REPETIDAS.sub(r"\1", t)
    t = re.sub(r"\s+", " ", t).strip()
    if not _FINAL.search(t):
        t += "."
    return f"<{IDIOMA}>{t}</{IDIOMA}>"


def trozos(texto: str, maximo: int = MAX_TROZO) -> list[str]:
    """Parte un texto largo en trozos de hasta `maximo` caracteres, cortando en el final de una frase."""
    salida, actual = [], ""
    for frase in _FIN_FRASE.split(texto):
        while len(frase) > maximo:                  # una frase interminable: se corta en una coma o un espacio
            corte = max(frase.rfind(",", 0, maximo), frase.rfind(" ", 0, maximo))
            corte = corte if corte > 0 else maximo
            salida.append(frase[:corte + 1].strip())
            frase = frase[corte + 1:].strip()
        if actual and len(actual) + 1 + len(frase) > maximo:
            salida.append(actual)
            actual = ""
        actual = f"{actual} {frase}".strip()
    if actual:
        salida.append(actual)
    return [s for s in salida if s]


# ---- Motor -----------------------------------------------------------------------------------------

class MotorSupertonic:
    """Supertonic 3: duración → texto → difusión (vector_estimator) → vocoder. 44,1 kHz, mono."""

    def __init__(self, voz: dict):
        preparar_entorno()
        import numpy as np
        import onnxruntime as ort
        self.np = np
        base = rutas.VOZ
        cfg = json.loads((base / "onnx" / "tts.json").read_text(encoding="utf-8"))
        self.indice = json.loads((base / "onnx" / "unicode_indexer.json").read_text(encoding="utf-8"))
        self.sr = int(cfg["ae"]["sample_rate"])
        self.trozo_base = int(cfg["ae"]["base_chunk_size"])
        self.compresion = int(cfg["ttl"]["chunk_compress_factor"])
        self.latente = int(cfg["ttl"]["latent_dim"])
        so = ort.SessionOptions()
        so.intra_op_num_threads = int(os.environ.get("VORTEX_MIKU_HILOS", min(8, os.cpu_count() or 4)))
        so.log_severity_level = 3

        def sesion(nombre):
            return ort.InferenceSession(str(base / "onnx" / f"{nombre}.onnx"), sess_options=so, providers=["CPUExecutionProvider"])
        self.duracion = sesion("duration_predictor")
        self.texto = sesion("text_encoder")
        self.vector = sesion("vector_estimator")
        self.vocoder = sesion("vocoder")
        estilo = json.loads((base / "voice_styles" / f"{voz.get('estilo_voz', 'F1')}.json").read_text(encoding="utf-8"))
        self.estilo_ttl = np.array(estilo["style_ttl"]["data"], dtype=np.float32).reshape(*estilo["style_ttl"]["dims"])
        self.estilo_dp = np.array(estilo["style_dp"]["data"], dtype=np.float32).reshape(*estilo["style_dp"]["dims"])

    def _mascara(self, largos, maximo=None):
        np = self.np
        maximo = int(maximo or largos.max())
        return (np.arange(maximo) < np.expand_dims(largos, 1)).astype(np.float32).reshape(-1, 1, maximo)

    def _trozo(self, texto: str, velocidad: float, rng):
        np = self.np
        t = preparar_texto(texto)
        valores = [ord(c) for c in t]
        ids = np.array([[self.indice[v] if v < len(self.indice) else -1 for v in valores]], dtype=np.int64)
        ids = np.where(ids < 0, 0, ids)             # un carácter que el modelo no conoce se lee como relleno
        mascara = self._mascara(np.array([ids.shape[1]], dtype=np.int64))
        dur = self.duracion.run(None, {"text_ids": ids, "style_dp": self.estilo_dp, "text_mask": mascara})[0] / velocidad
        emb = self.texto.run(None, {"text_ids": ids, "style_ttl": self.estilo_ttl, "text_mask": mascara})[0]
        largo_wav = int(dur.max() * self.sr)
        paso_lat = self.trozo_base * self.compresion
        n_lat = int((largo_wav + paso_lat - 1) // paso_lat)
        lat_mask = self._mascara(((dur * self.sr).astype(np.int64) + paso_lat - 1) // paso_lat, n_lat)
        xt = rng.standard_normal((1, self.latente * self.compresion, n_lat)).astype(np.float32) * lat_mask
        total = np.array([PASOS], dtype=np.float32)
        for i in range(PASOS):
            xt = self.vector.run(None, {"noisy_latent": xt, "text_emb": emb, "style_ttl": self.estilo_ttl, "text_mask": mascara,
                                        "latent_mask": lat_mask, "current_step": np.array([i], dtype=np.float32),
                                        "total_step": total})[0]
        wav = self.vocoder.run(None, {"latent": xt})[0].reshape(-1)
        return wav[:int(float(dur[0]) * self.sr)].astype(np.float32)

    def sintetizar(self, texto: str, velocidad: float):
        np = self.np
        rng = np.random.default_rng(zlib.crc32(texto.encode("utf-8")))     # misma frase, misma voz
        partes = []
        for t in trozos(texto):
            if partes:
                partes.append(np.zeros(int(SILENCIO_TROZOS * self.sr), dtype=np.float32))
            partes.append(self._trozo(t, min(max(velocidad, 0.7), 2.0), rng))
        return np.concatenate(partes), self.sr


def cargar(voz: dict):
    return MotorSupertonic(voz)


def estilo_de(voz: dict, miku: bool) -> dict:
    """El ajuste de la voz: normal, o el de Miku (el de fábrica con lo que la voz traiga en "estilo")."""
    if not miku:
        return dict(ESTILO_NORMAL)
    return {**ESTILO_MIKU, **voz.get("estilo", {})}


def procesar(x, sr: int, estilo: dict):
    """Del audio crudo del motor al que suena: tono, brillo y volumen según el estilo."""
    x = audio.cambiar_tono(x, estilo.get("tono", 0))
    if estilo.get("corte"):
        x = audio.filtrar_agudos(x, sr, estilo["corte"], estilo.get("estante", 0.0))
    return audio.nivelar(x, sr, rms_db=estilo.get("nivel", -22.0), relacion=estilo.get("compresion", 2.2))
