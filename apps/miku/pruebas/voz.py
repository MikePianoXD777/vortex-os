#!/usr/bin/env python3
"""Prueba de la voz (sin reproducir nada): genera la frase de referencia con la voz normal y con tres
variantes del estilo de Miku, las guarda como .ogg, mide tono, ritmo y volumen, y comprueba con el oído
(whisper) que se entienden.

    python3 -I pruebas/voz.py CARPETA_DE_AUDIOS [CARPETA_DE_TRABAJO]

Necesita los modelos de voz y de oído en ~/.local/share/vortex/miku y whisper-cli (VORTEX_MIKU_WHISPER).
"""

import os
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import audio, motor_voz, oido, rutas, voces  # noqa: E402
from miku.voz_worker import Voz  # noqa: E402
from pruebas.f0 import tono  # noqa: E402

FRASE = "Hola... soy Miku. Si quieres, te ayudo con lo que necesites. ¿Te cuento algo del período Sengoku?"

# Las tres variantes del estilo de Miku (la b es la de fábrica, ESTILO_MIKU).
VARIANTES = {
    "a": {"nombre": "suave", "tono": -150, "velocidad": 1.05, "corte": 8500, "estante": -2.0, "nivel": -24.0, "compresion": 2.6},
    "b": {"nombre": "intermedia (la de fábrica)", **motor_voz.ESTILO_MIKU},
    "c": {"nombre": "la más apagada y grave", "tono": -360, "velocidad": 1.04, "corte": 6000, "estante": -5.0, "nivel": -28.0, "compresion": 3.4},
}


def a_ogg(x: np.ndarray, sr: int, destino: Path) -> None:
    wav = audio.a_wav(x, sr)
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "wav", "-i", "-", "-c:a", "libvorbis", "-q:a", "5", str(destino)],
                   input=wav, check=True)


def entendido(x: np.ndarray, sr: int) -> str:
    """Pasa el audio por el oído de Miku (16 kHz, 16 bits, mono) y devuelve lo que transcribe."""
    r = subprocess.run(["ffmpeg", "-v", "error", "-f", "wav", "-i", "-", "-f", "s16le", "-ar", "16000", "-ac", "1", "-"],
                       input=audio.a_wav(x, sr), capture_output=True, check=True)
    return oido.transcribir(r.stdout)


def palabras(t: str) -> list[str]:
    import re
    import unicodedata
    t = unicodedata.normalize("NFD", t.lower())
    t = "".join(c for c in t if unicodedata.category(c) != "Mn")
    return re.sub(r"[^a-z0-9 ]", " ", t).split()


def wer(ref: str, hip: str) -> float:
    a, b = palabras(ref), palabras(hip)
    d = list(range(len(b) + 1))
    for i, pa in enumerate(a, 1):
        previo, d[0] = d[0], i
        for j, pb in enumerate(b, 1):
            previo, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, previo + (pa != pb))
    return d[len(b)] / max(1, len(a))


def main() -> int:
    salida = Path(sys.argv[1])
    salida.mkdir(parents=True, exist_ok=True)
    voz = Voz(voces.VOZ_DEFECTO, False)
    fallos = 0

    def informe(etiqueta: str, x: np.ndarray, sr: int, ruta: Path, tarda: float, ajustes: dict | None) -> None:
        nonlocal fallos
        t = tono(x, sr)
        texto = entendido(x, sr)
        error = wer(FRASE, texto)
        rms = 20 * np.log10(np.sqrt(np.mean(x ** 2)) + 1e-9)
        # Volumen parejo: desviación (dB) del volumen de cada 0,25 s con voz.
        n = int(sr * 0.25)
        niveles = [20 * np.log10(np.sqrt(np.mean(x[i:i + n] ** 2)) + 1e-9) for i in range(0, len(x) - n, n)]
        niveles = [v for v in niveles if v > rms - 20]
        bien = error <= 0.25
        fallos += 0 if bien else 1
        print(f"{'  ok  ' if bien else '  MAL '}{etiqueta}: {ruta.name}  {len(x) / sr:.2f} s  tono {t['mediana']:.0f} Hz (±{t['desviacion']:.0f})  "
              f"rms {rms:.1f} dBFS  parejo ±{np.std(niveles):.1f} dB  sintetizó en {tarda:.1f} s  errores de palabra {error:.0%}")
        print(f"        el oído entendió: «{texto}»")
        if ajustes:
            print(f"        valores: {ajustes}")

    voz.sintetizar("Prueba.")                           # calienta
    t0 = time.monotonic()
    x, sr = voz.sintetizar(FRASE)
    informe("voz normal (sin avatar)", x, sr, salida / "10-f1-normal.ogg", time.monotonic() - t0, voz.estilo)
    a_ogg(x, sr, salida / "10-f1-normal.ogg")
    for letra, ajustes in VARIANTES.items():
        v = Voz(voces.VOZ_DEFECTO, True, {k: val for k, val in ajustes.items() if k != "nombre"})
        t0 = time.monotonic()
        x, sr = v.sintetizar(FRASE)
        ruta = salida / f"11-miku-{letra}.ogg"
        a_ogg(x, sr, ruta)
        informe(f"Miku {letra} ({ajustes['nombre']})", x, sr, ruta, time.monotonic() - t0, {k: val for k, val in ajustes.items() if k != "nombre"})
    print("FALLOS:", fallos)
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
