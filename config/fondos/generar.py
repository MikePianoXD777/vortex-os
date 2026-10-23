#!/usr/bin/env python3
"""Genera los fondos originales de Vortexine's OS (remolinos galácticos), sin imágenes de terceros.

    python3 config/fondos/generar.py            # todos los fondos en config/fondos/
    python3 config/fondos/generar.py --vista    # además, vistas previas pequeñas en /tmp

Cada fondo sale de una semilla y una paleta, así se puede regenerar idéntico.
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ANCHO, ALTO = 2560, 1440
AQUI = Path(__file__).resolve().parent

# nombre: (semilla, color de los brazos, color del núcleo, brazos, torsión, inclinación)
FONDOS = {
    "vortex-azul": (7, (70, 110, 255), (200, 220, 255), 2, 4.2, 0.55),
    "vortex-violeta": (21, (150, 80, 255), (255, 210, 250), 3, 3.6, 0.60),
    "vortex-ambar": (42, (255, 120, 40), (255, 230, 190), 2, 4.8, 0.50),
    "vortex-turquesa": (77, (30, 200, 190), (210, 255, 245), 2, 3.9, 0.58),
}


def ruido(rng, escala, alto, ancho):
    """Ruido suave: valores aleatorios en una rejilla pequeña, ampliados con interpolación."""
    pequeno = rng.random((max(2, alto // escala), max(2, ancho // escala))).astype(np.float32)
    img = Image.fromarray((pequeno * 255).astype(np.uint8)).resize((ancho, alto), Image.BICUBIC)
    return np.asarray(img, dtype=np.float32) / 255.0


def generar(semilla, brazo, nucleo, n_brazos, torsion, inclinacion):
    rng = np.random.default_rng(semilla)
    y, x = np.mgrid[0:ALTO, 0:ANCHO].astype(np.float32)
    # Centro algo desplazado, y el disco «inclinado» (aplastado en vertical).
    cx, cy = ANCHO * 0.5, ALTO * 0.52
    dx = (x - cx) / (ALTO * 0.5)
    dy = (y - cy) / (ALTO * 0.5 * inclinacion)
    r = np.sqrt(dx * dx + dy * dy) + 1e-4
    theta = np.arctan2(dy, dx)

    turb = ruido(rng, 120, ALTO, ANCHO) * 0.5 + ruido(rng, 40, ALTO, ANCHO) * 0.3 + ruido(rng, 12, ALTO, ANCHO) * 0.2
    # Brazos en espiral logarítmica, deformados por el ruido para que no se vean perfectos.
    fase = n_brazos * theta - torsion * np.log(r) + turb * 2.2
    brazos = (0.5 + 0.5 * np.cos(fase)) ** 3
    disco = np.exp(-((r - 0.55) / 0.42) ** 2)          # anillo donde viven los brazos
    luz_brazos = brazos * disco * (0.55 + 0.45 * turb)
    # Suavizado: el ruido ampliado deja bordes en bloque.
    capa = Image.fromarray((np.clip(luz_brazos, 0, 1) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(4))
    luz_brazos = np.asarray(capa, dtype=np.float32) / 255.0

    nucleo_luz = np.exp(-(r / 0.10) ** 2) * 1.4 + np.exp(-(r / 0.28) ** 2) * 0.35
    halo = np.exp(-(r / 1.1) ** 2) * 0.10

    img = np.zeros((ALTO, ANCHO, 3), dtype=np.float32)
    c_brazo = np.array(brazo, dtype=np.float32) / 255.0
    c_nucleo = np.array(nucleo, dtype=np.float32) / 255.0
    img += luz_brazos[..., None] * c_brazo * 1.15
    img += nucleo_luz[..., None] * c_nucleo
    img += halo[..., None] * c_brazo

    # Polvo oscuro que corta los brazos (da profundidad).
    polvo = ruido(rng, 24, ALTO, ANCHO) * 0.6 + ruido(rng, 8, ALTO, ANCHO) * 0.4
    img *= (0.75 + 0.25 * polvo)[..., None]

    # Estrellas: puntos al azar con brillo variable; algunas más grandes y un poco azuladas.
    estrellas = np.zeros((ALTO, ANCHO), dtype=np.float32)
    n = int(ANCHO * ALTO * 0.00035)
    ys, xs = rng.integers(0, ALTO, n), rng.integers(0, ANCHO, n)
    estrellas[ys, xs] = rng.random(n) ** 3
    capa = Image.fromarray((np.clip(estrellas, 0, 1) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.8))
    estrellas = np.asarray(capa, dtype=np.float32) / 255.0 * 3.0
    img += estrellas[..., None] * np.array([0.85, 0.9, 1.0], dtype=np.float32)

    # Viñeta y curva suave para que nada se queme.
    vineta = np.clip(1.25 - 0.55 * np.sqrt(((x - cx) / ANCHO) ** 2 * 4 + ((y - cy) / ALTO) ** 2 * 4), 0, 1)
    img *= vineta[..., None]
    img = 1.0 - np.exp(-img * 1.6)
    return Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8))


def main():
    vista = "--vista" in sys.argv
    for nombre, params in FONDOS.items():
        img = generar(*params)
        destino = AQUI / f"{nombre}.jpg"
        img.save(destino, quality=92, optimize=True)
        print(destino)
        if vista:
            img.resize((640, 360), Image.LANCZOS).save(f"/tmp/claude-1000/vista-{nombre}.jpg", quality=85)


if __name__ == "__main__":
    main()
