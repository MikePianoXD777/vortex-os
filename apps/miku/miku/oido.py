"""El oído de Miku: graba del micrófono, corta solo cuando dejas de hablar y transcribe en la propia máquina.

- El micrófono se abre SOLO mientras `escuchar()` corre (la ventana está abierta y el indicador visible).
  No hay palabra de activación ni escucha de fondo.
- El audio vive solo en memoria: va de pw-record a este proceso y de aquí a whisper-cli por su
  entrada estándar. No se escribe ningún archivo.
- Transcribe con whisper.cpp (whisper-cli, del paquete whisper-cpp) y el modelo ggml-small-q5_1 que se baja
  la primera vez a ~/.local/share/vortex/miku/oido/, en español y sin internet.

Solo usa la biblioteca estándar. El detector de voz sigue al ruido de fondo (con preénfasis, para
que el zumbido grave de un ventilador no cuente como voz) y corta tras ~1,5 s de silencio.
"""

import io
import math
import os
import re
import select
import shutil
import subprocess
import threading
import time
import wave
from array import array
from collections import deque

from . import rutas

MODELO = "ggml-small-q5_1.bin"
RATE = 16000
TRAMO_MS = 20
TRAMO = RATE * TRAMO_MS // 1000          # muestras por tramo
BYTES_TRAMO = TRAMO * 2

# Whisper, cuando le llega ruido o silencio, inventa frases de sus datos de entrenamiento y marcas
# como «[Música]» o «(aplausos)». Se quitan las marcas, y si lo que queda es solo una de esas frases
# inventadas, es que no dijiste nada.
_MARCAS = re.compile(r"\[[^\]]*\]|\([^)]*\)|[♪♫]+")
_FRASES_FANTASMA = re.compile(
    r"^(gracias por (ver|escuchar)( este| el)?( v[ií]deo)?|suscr[ií]bete( al canal)?|"
    r"subt[ií]tulos? (realizados? )?por.*|.*amara\.org.*|nos vemos en el pr[oó]ximo( v[ií]deo)?)$", re.I)


class ErrorOido(Exception):
    """Algo falló y se puede decir en español (mensaje listo para mostrar)."""


def _nivel_db(tramo: bytes) -> float:
    a = array("h")
    a.frombytes(tramo)
    # Preénfasis y[n] = x[n] - 0,95·x[n-1]: el ruido de fondo es sobre todo grave y su nivel oscila ±8 dB;
    # así queda estable y la voz sigue ~24 dB por encima.
    e = sum((a[i] - 0.95 * a[i - 1]) ** 2 for i in range(1, len(a))) / (len(a) - 1)
    return 10 * math.log10(e / (32768.0 ** 2) + 1e-12)


def encontrar_whisper() -> tuple[str, str]:
    """(whisper-cli, modelo) o ErrorOido."""
    cli = os.environ.get("VORTEX_MIKU_WHISPER") or shutil.which("whisper-cli")
    modelo = rutas.OIDO / MODELO
    if not cli or not os.access(cli, os.X_OK):
        raise ErrorOido("No encuentro el oído (whisper-cli). Reinstala el paquete vortex-miku. Mientras tanto puedes escribirme.")
    if not modelo.is_file():
        raise ErrorOido("Todavía no tengo el oído: se baja la primera vez que me abres, con internet. Mientras tanto puedes escribirme.")
    return cli, str(modelo)


def transcribir(pcm: bytes) -> str:
    """Pasa audio crudo (16 kHz, 16 bits, mono) a texto con whisper.cpp. Nada toca el disco."""
    cli, modelo = encontrar_whisper()
    w = io.BytesIO()
    with wave.open(w, "wb") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(RATE)
        f.writeframes(pcm)
    hilos = os.environ.get("VORTEX_MIKU_HILOS", str(min(8, os.cpu_count() or 4)))
    try:
        # -otxt hace que imprima el texto por stdout cuando la entrada es stdin (-f -).
        # cwd en la carpeta en memoria del usuario: si whisper llegara a escribir algo, no sería en su carpeta de trabajo.
        r = subprocess.run([cli, "-m", modelo, "-l", "es", "-t", hilos, "-bs", "1", "-bo", "1", "-np", "-nt", "-otxt", "-f", "-"],
                           input=w.getvalue(), capture_output=True, timeout=90, cwd=rutas.runtime())
    except (OSError, subprocess.TimeoutExpired) as e:
        raise ErrorOido(f"No pude transcribir lo que dijiste ({type(e).__name__}).") from e
    if r.returncode != 0:
        raise ErrorOido("No pude transcribir lo que dijiste (whisper falló).")
    texto = " ".join(r.stdout.decode("utf-8", "replace").split())
    texto = " ".join(_MARCAS.sub(" ", texto).split())
    limpio = texto.strip(" .,;:-¡!¿?…")
    if len(limpio) < 2 or _FRASES_FANTASMA.match(limpio):
        return ""
    return texto


class Oido:
    """Una escucha a la vez. `escuchar()` bloquea hasta tener tu frase (córrelo en un hilo)."""

    def __init__(self, silencio: float = 1.5, espera: float = 8.0, maximo: float = 30.0, margen_db: float = 10.0):
        self.silencio, self.espera, self.maximo, self.margen_db = silencio, espera, maximo, margen_db
        self._cortar = threading.Event()      # «ya acabé» (clic en el micrófono)
        self._cancelar = threading.Event()    # descarta lo grabado
        self._pw: subprocess.Popen | None = None
        self._lock = threading.Lock()

    def cortar_ahora(self) -> None:
        self._cortar.set()

    def cancelar(self) -> None:
        self._cancelar.set()
        self._cerrar_grabador()

    def _cerrar_grabador(self) -> None:
        with self._lock:
            pw, self._pw = self._pw, None
        if pw and pw.poll() is None:
            pw.terminate()
            try:
                pw.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pw.kill()

    def _abrir_fuente(self):
        """Descriptor de donde llega el PCM. VORTEX_MIKU_ENTRADA=archivo.wav sirve para pruebas (sin micrófono)."""
        prueba = os.environ.get("VORTEX_MIKU_ENTRADA")
        if prueba:
            cmd = ["ffmpeg", "-v", "quiet", "-re", "-i", prueba, "-f", "s16le", "-ar", str(RATE), "-ac", "1", "-"]
        elif shutil.which("pw-record"):
            cmd = ["pw-record", "--raw", "--rate", str(RATE), "--channels", "1", "--format", "s16", "--latency", "20ms",
                   "-n", str(int((self.maximo + self.espera + 2) * RATE)), "-"]
        else:
            raise ErrorOido("No encuentro pw-record (PipeWire) para usar el micrófono. Puedes escribirme.")
        try:
            pw = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, stdin=subprocess.DEVNULL)
        except OSError as e:
            raise ErrorOido("No pude abrir el micrófono. Puedes escribirme.") from e
        with self._lock:
            self._pw = pw
        return pw

    def escuchar(self, al_nivel=None, al_hablar=None) -> bytes | None:
        """Graba hasta que dejes de hablar. Devuelve el PCM de tu frase, o None si nadie habló o se canceló.

        `al_nivel(0..1)` se llama ~25 veces por segundo con el volumen del micrófono (para el indicador);
        `al_hablar()` una vez, cuando empieza a oír voz.
        """
        self._cortar.clear()
        self._cancelar.clear()
        pw = self._abrir_fuente()
        fd = pw.stdout.fileno()
        buf = b""
        t = 0                                   # tramos leídos: el reloj es el propio audio
        previo = deque(maxlen=20)               # 0,4 s anteriores al inicio de la voz
        captura: list[bytes] = []
        hablando = False
        seguidos_voz = seguidos_silencio = sobre = 0
        suelo = None
        calib: list[float] = []
        inicio_t = 0
        t_pared = time.monotonic()
        try:
            while not self._cancelar.is_set():
                if self._cortar.is_set():
                    break
                r, _, _ = select.select([fd], [], [], 0.2)
                if not r:
                    if time.monotonic() - t_pared > self.espera + self.maximo + 5:
                        break                   # el flujo se paró
                    continue
                d = os.read(fd, 4096)
                if not d:
                    break                       # fin del flujo (la fuente de pruebas o un micrófono que se fue)
                t_pared = time.monotonic()
                buf += d
                while len(buf) >= BYTES_TRAMO:
                    tramo, buf = buf[:BYTES_TRAMO], buf[BYTES_TRAMO:]
                    t += 1
                    n = _nivel_db(tramo)
                    if al_nivel and t % 2 == 0:
                        al_nivel(max(0.0, min(1.0, (n + 62.0) / 40.0)))
                    if suelo is None:           # ruido de fondo = mediana de los primeros 0,3 s (acotada)
                        calib.append(n)
                        previo.append(tramo)
                        if len(calib) >= 15:
                            suelo = min(-46.0, max(-70.0, sorted(calib)[len(calib) // 2]))
                        continue
                    voz = n > suelo + self.margen_db
                    if not hablando:
                        if not voz and -90 < n < suelo + 6:         # un silencio digital (cero exacto) no cuenta como ruido de fondo
                            suelo = 0.98 * suelo + 0.02 * n         # sigue el ruido lento
                        suelo = min(-46.0, max(-70.0, suelo))
                        previo.append(tramo)
                        seguidos_voz = seguidos_voz + 1 if voz else 0
                        if seguidos_voz >= 5:                       # 100 ms seguidos de voz
                            hablando = True
                            inicio_t = t
                            captura = list(previo)
                            seguidos_silencio = 0
                            if al_hablar:
                                al_hablar()
                        elif t * TRAMO_MS / 1000 > self.espera:
                            return None
                    else:
                        captura.append(tramo)
                        # Un tramo suelto por encima del umbral (clic de teclado, golpe) no cuenta como voz:
                        # para reiniciar la cuenta del silencio hacen falta 3 seguidos (60 ms).
                        if n > suelo + self.margen_db - 4:
                            sobre += 1
                            seguidos_silencio = 0 if sobre >= 3 else seguidos_silencio + 1
                        else:
                            sobre = 0
                            seguidos_silencio += 1
                        if seguidos_silencio * TRAMO_MS / 1000 >= self.silencio or (t - inicio_t) * TRAMO_MS / 1000 >= self.maximo:
                            raise StopIteration
        except StopIteration:
            pass
        finally:
            self._cerrar_grabador()
            try:                                # no se vuelve hasta que el grabador haya muerto de verdad
                pw.wait(timeout=3)
            except subprocess.TimeoutExpired:
                pw.kill()
                pw.wait()
            pw.stdout.close()
        if self._cancelar.is_set() or not hablando:
            return None
        if seguidos_silencio:                   # quita casi todo el silencio final (deja 0,3 s)
            captura = captura[:len(captura) - max(0, seguidos_silencio - 15)]
        return b"".join(captura)
