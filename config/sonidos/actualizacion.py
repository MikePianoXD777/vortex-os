#!/usr/bin/env python3
"""Genera actualizacion.wav, el sonido del aviso «Hay actualizaciones» de Vortexine's OS.

Distinto del de las notificaciones normales, para que se sepa sin mirar que es una actualización:
un arpegio que sube (Do, Mi, Sol y Do agudo) con timbre de campanita, sintetizado aquí mismo (sin
grabaciones de nadie). Solo usa la biblioteca estándar.

    python3 config/sonidos/actualizacion.py      # reescribe config/sonidos/actualizacion.wav
"""
import array
import math
import wave
from pathlib import Path

TASA = 48000
NOTAS = (1046.50, 1318.51, 1567.98, 2093.00)   # Do6, Mi6, Sol6, Do7
SEPARACION = 0.11                              # s entre una nota y la siguiente
DURACION_NOTA = 0.9
VOLUMEN = 0.32
# Parciales de campanita: (múltiplo de la frecuencia, fuerza, cuánto más rápido se apaga).
PARCIALES = ((1.0, 1.0, 1.0), (2.0, 0.35, 1.8), (3.01, 0.12, 2.6), (4.2, 0.05, 3.5))


def main() -> None:
    total = int(TASA * (SEPARACION * (len(NOTAS) - 1) + DURACION_NOTA))
    mezcla = [0.0] * total
    for i, f in enumerate(NOTAS):
        inicio = int(TASA * SEPARACION * i)
        fuerza = 0.8 + 0.2 * i / (len(NOTAS) - 1)   # la última, un poco más
        for n in range(int(TASA * DURACION_NOTA)):
            t = n / TASA
            ataque = min(1.0, t / 0.004)             # sin «clic» al empezar
            s = sum(a * math.exp(-t * 5.0 * d) * math.sin(2 * math.pi * f * m * t) for m, a, d in PARCIALES)
            if inicio + n < total:
                mezcla[inicio + n] += fuerza * ataque * s
    pico = max(abs(x) for x in mezcla) or 1.0
    caida = int(TASA * 0.05)                         # se apaga suave al final
    muestras = array.array("h", (
        int(32767 * VOLUMEN * x / pico * (min(1.0, (total - k) / caida))) for k, x in enumerate(mezcla)))
    destino = Path(__file__).with_name("actualizacion.wav")
    with wave.open(str(destino), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(TASA)
        w.writeframes(muestras.tobytes())
    print(f"{destino} ({len(muestras) / TASA:.2f} s)")


if __name__ == "__main__":
    main()
