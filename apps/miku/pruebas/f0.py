"""Mide el tono (F0) de una voz con autocorrelación, sin dependencias fuera de numpy. Para las pruebas.

    python3 -I pruebas/f0.py audio.wav [otro.wav ...]
"""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import audio  # noqa: E402


def tono(x: np.ndarray, sr: int, minimo: float = 75.0, maximo: float = 400.0) -> dict:
    """Tono mediano (Hz) y desviación de los tramos con voz. Ventanas de 50 ms cada 10 ms."""
    paso, ventana = int(sr * 0.010), int(sr * 0.050)
    lag_min, lag_max = int(sr / maximo), int(sr / minimo)
    energia = np.array([np.sqrt(np.mean(x[i:i + ventana] ** 2)) for i in range(0, len(x) - ventana, paso)])
    umbral = 0.25 * float(np.percentile(energia, 90))
    valores = []
    for k, i in enumerate(range(0, len(x) - ventana, paso)):
        if energia[k] < umbral:
            continue
        v = x[i:i + ventana] - np.mean(x[i:i + ventana])
        n = len(v)
        f = np.fft.rfft(v, 2 * n)
        ac = np.fft.irfft(f * np.conj(f))[:n]
        if ac[0] <= 0:
            continue
        ac = ac / ac[0]
        seg = ac[lag_min:lag_max]
        j = int(np.argmax(seg))
        if seg[j] > 0.5:                        # periodicidad clara = tramo con voz
            valores.append(sr / (lag_min + j))
    if not valores:
        return {"mediana": 0.0, "desviacion": 0.0, "tramos": 0}
    v = np.array(valores)
    return {"mediana": float(np.median(v)), "desviacion": float(np.std(v)), "tramos": len(v)}


def main() -> int:
    for ruta in sys.argv[1:]:
        x, sr = audio.leer_wav(ruta)
        t = tono(x, sr)
        print(f"{Path(ruta).name}: {len(x) / sr:.2f} s, tono mediano {t['mediana']:.0f} Hz (±{t['desviacion']:.0f}), "
              f"rms {20 * np.log10(np.sqrt(np.mean(x ** 2)) + 1e-9):.1f} dBFS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
