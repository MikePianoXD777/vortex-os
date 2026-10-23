"""Tratamiento del audio de la voz de Miku con numpy: tono, brillo, volumen y el movimiento de la boca.

Todo es offline y por frase (nada en tiempo real): se aplica al WAV de cada frase antes de sonar.
"""

import io
import wave

import numpy as np


def centesimas_a_factor(centesimas: float) -> float:
    return 2.0 ** (centesimas / 1200.0)


def cambiar_tono(x: np.ndarray, centesimas: float) -> np.ndarray:
    """Baja o sube el tono remuestreando (como acelerar o frenar una cinta).

    Negativo = más grave Y un poco más lento, y también más "ancho" el timbre, que es justo lo
    que queremos para una voz más apagada. Es limpio mientras el cambio sea de unas decenas de
    centésimas a unos cientos (la voz de Miku usa de -50 a -200).
    """
    if not centesimas or len(x) < 2:
        return x
    r = centesimas_a_factor(centesimas)
    n = int(len(x) / r)
    return np.interp(np.arange(n) * r, np.arange(len(x)), x).astype(np.float32)


def filtrar_agudos(x: np.ndarray, sr: int, corte_hz: float, estante_db: float = 0.0, estante_hz: float = 3500.0) -> np.ndarray:
    """Quita brillo: paso bajo suave (Butterworth de 3.er orden, fase cero) y un estante de agudos."""
    if len(x) < 8:
        return x
    espectro = np.fft.rfft(x)
    f = np.fft.rfftfreq(len(x), 1.0 / sr)
    ganancia = 1.0 / np.sqrt(1.0 + (f / corte_hz) ** 6)
    if estante_db:
        # Baja `estante_db` dB por encima de `estante_hz`, con una transición suave (sigmoide).
        paso = 1.0 / (1.0 + np.exp(-(f - estante_hz) / (estante_hz * 0.25)))
        ganancia = ganancia * 10.0 ** (estante_db * paso / 20.0)
    return np.fft.irfft(espectro * ganancia, len(x)).astype(np.float32)


def _db(v: float) -> float:
    return 20.0 * np.log10(max(v, 1e-9))


def nivelar(x: np.ndarray, sr: int, rms_db: float = -24.0, pico_db: float = -3.0, relacion: float = 2.2) -> np.ndarray:
    """Volumen bajo y parejo: comprime suave las partes fuertes y deja la frase en `rms_db`.

    La envolvente se mide cada 5 ms, el compresor ataca en 10 ms y suelta en 120 ms; el umbral
    es el nivel medio de la propia frase. Después se sube o baja todo a `rms_db` (RMS de la frase
    sin los silencios) y el pico no pasa de `pico_db`.
    """
    if len(x) < sr // 50:
        return x
    salto = max(1, sr // 200)
    n = len(x) // salto
    bloques = x[: n * salto].reshape(n, salto)
    env = np.sqrt((bloques ** 2).mean(axis=1) + 1e-12)
    env_db = 20.0 * np.log10(env)
    voz = env_db > (env_db.max() - 35.0)                 # sin los silencios
    umbral = float(np.mean(env_db[voz])) if voz.any() else float(env_db.max())
    objetivo = np.where(env_db > umbral, (env_db - umbral) / relacion + umbral, env_db)
    reduccion = np.minimum(objetivo - env_db, 0.0)
    a_ataque, a_suelta = 1.0 - np.exp(-5.0 / 10.0), 1.0 - np.exp(-5.0 / 120.0)
    suave = np.empty_like(reduccion)
    g = 0.0
    for i, r in enumerate(reduccion):
        g += (r - g) * (a_ataque if r < g else a_suelta)  # baja rápido, sube despacio
        suave[i] = g
    ganancia = np.interp(np.arange(len(x)), (np.arange(n) + 0.5) * salto, 10.0 ** (suave / 20.0))
    y = x * ganancia
    activo = np.abs(y) > 10.0 ** ((_db(float(np.abs(y).max())) - 35.0) / 20.0)
    rms = float(np.sqrt(np.mean(y[activo] ** 2))) if activo.any() else float(np.sqrt(np.mean(y ** 2)))
    y = y * (10.0 ** (rms_db / 20.0) / max(rms, 1e-9))
    pico = float(np.abs(y).max())
    tope = 10.0 ** (pico_db / 20.0)
    if pico > tope:
        y = np.tanh(y / tope * 0.9) * tope / np.tanh(0.9) if pico > 2 * tope else y * (tope / pico)
    return y.astype(np.float32)


def a_wav(x: np.ndarray, sr: int) -> bytes:
    pcm = (np.clip(x, -1.0, 1.0) * 32767.0).astype("<i2")
    b = io.BytesIO()
    with wave.open(b, "wb") as f:
        f.setnchannels(1)
        f.setsampwidth(2)
        f.setframerate(sr)
        f.writeframes(pcm.tobytes())
    return b.getvalue()


def leer_wav(ruta_o_bytes) -> tuple[np.ndarray, int]:
    """Lee un WAV de 16 bits (de un archivo o de bytes) como float32 mono."""
    origen = io.BytesIO(ruta_o_bytes) if isinstance(ruta_o_bytes, (bytes, bytearray)) else ruta_o_bytes
    with wave.open(origen, "rb") as f:
        sr, canales, ancho = f.getframerate(), f.getnchannels(), f.getsampwidth()
        crudo = f.readframes(f.getnframes())
    if ancho != 2:
        raise ValueError("solo se leen WAV de 16 bits")
    x = np.frombuffer(crudo, dtype="<i2").astype(np.float32) / 32768.0
    if canales > 1:
        x = x.reshape(-1, canales).mean(axis=1)
    return x, sr


# ---- Boca: de la voz a las vocales del avatar ----------------------------------------------

# Cuánto se abre la boca para cada vocal cuando suena fuerte (a abre más; i y u, menos).
_APERTURA = {"aa": 1.0, "oh": 0.8, "ee": 0.7, "ih": 0.55, "ou": 0.6}
VOCALES = ("aa", "ih", "ou", "ee", "oh")      # a, i, u, e, o en las expresiones VRM


def _pertenencia(valor: float, centro: float, ancho: float) -> float:
    """Triangular: 1 en `centro`, 0 a `ancho` de distancia."""
    return max(0.0, 1.0 - abs(valor - centro) / ancho)


def visemas(x: np.ndarray, sr: int, hz: int = 30) -> list[list[int]]:
    """Cuánto se abre la boca en cada cuadro (hz por segundo) y con qué vocal.

    Devuelve una lista de [aa, ih, ou, ee, oh] con enteros de 0 a 100. La abertura sale del
    volumen de la voz (normalizado al percentil 95 de la propia frase); la vocal, de dónde está la
    energía del espectro: un pico grave y todo oscuro es o/u, un pico alto de primera formante es a,
    y mucha energía arriba es e/i. Es una aproximación barata (sin texto ni fonemas) pero se nota
    que la boca cambia de forma al hablar y no solo se abre y se cierra.
    """
    paso = max(1, sr // hz)
    n = len(x) // paso
    if n == 0:
        return []
    ventana = np.hanning(2 * paso)
    f = np.fft.rfftfreq(2 * paso, 1.0 / sr)
    banda1 = (f >= 250) & (f <= 1000)     # primera formante: apertura de la boca
    banda2 = (f >= 900) & (f <= 3500)     # segunda formante: adelante o atrás
    relleno = np.concatenate([np.zeros(paso // 2, dtype=np.float32), x, np.zeros(2 * paso, dtype=np.float32)])
    rms = np.array([np.sqrt(np.mean(x[i * paso:(i + 1) * paso] ** 2)) for i in range(n)])
    ref = float(np.percentile(rms, 95)) or 1.0
    salida = []
    for i in range(n):
        v = min(1.0, (rms[i] / ref) ** 0.8) if rms[i] > 0.02 * ref else 0.0
        if v <= 0.02:
            salida.append([0, 0, 0, 0, 0])
            continue
        s = np.abs(np.fft.rfft(relleno[i * paso:i * paso + 2 * paso] * ventana)) ** 2
        e1, e2 = s[banda1], s[banda2]
        f1 = float(f[banda1][int(np.argmax(e1))]) if e1.sum() > 0 else 500.0
        centroide = float((f[banda2] * e2).sum() / e2.sum()) if e2.sum() > 0 else 1500.0
        # f1 bajo = vocal cerrada (i, u); medio = e, o; alto = a. centroide alto = delante (i, e).
        cerrada, media, abierta = (_pertenencia(f1, 330, 220), _pertenencia(f1, 520, 200), max(0.0, min(1.0, (f1 - 560) / 220)))
        delante, atras = max(0.0, min(1.0, (centroide - 1500) / 800)), max(0.0, min(1.0, (1900 - centroide) / 900))
        pesos = {
            "aa": abierta + 0.15,
            "ih": cerrada * delante,
            "ou": cerrada * atras,
            "ee": media * delante,
            "oh": media * atras,
        }
        total = sum(pesos.values()) or 1.0
        salida.append([int(round(100 * v * _APERTURA[k] * pesos[k] / total)) for k in VOCALES])
    return salida
