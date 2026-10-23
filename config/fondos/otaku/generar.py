#!/usr/bin/env python3
"""Fondo ORIGINAL de respaldo del «Modo otaku» de Vortex OS (bin/vortex-otaku).

Se usa cuando Wallhaven no responde o no hay red. No dibuja ningún personaje ni usa
imágenes de terceros: solo unos audífonos estilizados, ondas de sonido y notas, con la
paleta que recuerda a Miku Nakano (el azul de sus audífonos, el castaño de su pelo y un
verde azulado). Todo sale de este código, así que no hay derechos de terceros en el jpg.

    python3 config/fondos/otaku/generar.py                    # deja otaku-respaldo.jpg aquí al lado
    python3 config/fondos/otaku/generar.py --salida RUTA.jpg  # lo deja en otra parte
    python3 config/fondos/otaku/generar.py --vista            # además, una vista pequeña en /tmp

Sale siempre igual (semilla fija). Necesita python-pillow y python-numpy; el jpg ya generado
está en el repo, así que en el sistema instalado no hace falta ninguno de los dos.
"""

import argparse
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

ANCHO, ALTO = 2560, 1440
AQUI = Path(__file__).resolve().parent
SEMILLA = 39

# Paleta (RGB 0-255).
AZUL_NOCHE = (11, 18, 46)
AZUL_FONDO = (28, 48, 108)
AZUL_AUDIFONOS = (52, 84, 168)
AZUL_CLARO = (112, 148, 226)
CASTANO = (132, 84, 56)
CASTANO_OSCURO = (56, 34, 30)
VERDE_AZULADO = (60, 184, 174)
CREMA = (248, 234, 214)


def c(rgb):
    return np.array(rgb, dtype=np.float32) / 255.0


def suave(t):
    t = np.clip(t, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


def mezcla(a, b, t):
    """Mezcla de colores a -> b con t (matriz HxW o escalar) entre 0 y 1."""
    t = np.asarray(t, dtype=np.float32)
    if t.ndim:
        t = t[..., None]
    return a * (1.0 - t) + b * t


def borde(d):
    """Distancia con signo (negativa dentro) -> cobertura 0..1 con un píxel de antialiasing."""
    return np.clip(0.5 - d, 0.0, 1.0).astype(np.float32)


def caja_redondeada(x, y, cx, cy, mx, my, radio):
    """Distancia con signo a una caja de bordes redondeados centrada en (cx, cy)."""
    qx = np.abs(x - cx) - (mx - radio)
    qy = np.abs(y - cy) - (my - radio)
    fuera = np.sqrt(np.maximum(qx, 0.0) ** 2 + np.maximum(qy, 0.0) ** 2)
    dentro = np.minimum(np.maximum(qx, qy), 0.0)
    return fuera + dentro - radio


def sobre(img, mascara, color, alfa=1.0):
    """Pone `color` sobre `img` donde `mascara` (HxW) vale algo."""
    m = (mascara * alfa)[..., None]
    return img * (1.0 - m) + color * m


def brillo(img, mascara, color, k):
    """Suma luz (resplandor)."""
    return img + (mascara * k)[..., None] * color


def difuminar(m, radio):
    capa = Image.fromarray((np.clip(m, 0, 1) * 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(radio))
    return np.asarray(capa, dtype=np.float32) / 255.0


def mascara_dibujada(dibujar, ss=2):
    """Máscara 0..1 dibujada al doble de tamaño y reducida, para bordes suaves."""
    capa = Image.new("L", (ANCHO * ss, ALTO * ss), 0)
    dibujar(ImageDraw.Draw(capa), ss)
    capa = capa.resize((ANCHO, ALTO), Image.LANCZOS)
    return np.asarray(capa, dtype=np.float32) / 255.0


def nota(d, ss, x, y, s):
    """Corchea: cabeza, plica y bandera. (x, y) es el centro de la cabeza; s, el tamaño."""
    x, y, s = x * ss, y * ss, s * ss
    d.ellipse((x - 0.62 * s, y - 0.42 * s, x + 0.62 * s, y + 0.42 * s), fill=255)
    px = x + 0.5 * s
    d.rectangle((px - 0.09 * s, y - 3.0 * s, px + 0.09 * s, y), fill=255)
    pts = []
    for i in range(21):
        t = i / 20.0
        pts.append((px + 0.09 * s + 1.15 * s * math.sin(t * 1.5) * t ** 0.5, y - 3.0 * s + 1.9 * s * t * (1.0 - 0.25 * t)))
    d.line(pts, fill=255, width=max(1, int(0.34 * s)))


def generar():
    rng = np.random.default_rng(SEMILLA)
    y, x = np.mgrid[0:ALTO, 0:ANCHO].astype(np.float32)
    cx, cy = ANCHO * 0.5, ALTO * 0.56

    # --- Fondo: noche azul arriba que se vuelve castaño abajo, con luz verde azulada al centro.
    t = y / ALTO
    img = mezcla(c(AZUL_NOCHE), c(AZUL_FONDO), suave(t * 1.5))
    img = mezcla(img, c(CASTANO_OSCURO), suave((t - 0.55) * 2.0) * 0.85)
    r = np.sqrt((x - cx) ** 2 + (y - cy) ** 2)
    img = brillo(img, np.exp(-(r / 760.0) ** 2), c(VERDE_AZULADO), 0.32)
    esq = np.sqrt((x - ANCHO * 0.9) ** 2 + (y - ALTO * 1.0) ** 2)
    img = brillo(img, np.exp(-(esq / 700.0) ** 2), c(CASTANO), 0.40)

    # --- Ondas de sonido que salen de los audífonos (anillos que se apagan).
    def ondas(d, ss):
        for i in range(13):
            rad = (420 + i * 92) * ss
            v = int(255 * (1.0 - i / 13.0) ** 1.6)
            d.ellipse((cx * ss - rad, cy * ss - rad, cx * ss + rad, cy * ss + rad), outline=v, width=3 * ss)

    img = brillo(img, mascara_dibujada(ondas), c(VERDE_AZULADO), 0.55)

    # --- Bokeh: círculos suaves de luz, cada uno con un color de la paleta.
    paleta = [c(VERDE_AZULADO), c(CREMA), c(CASTANO), c(AZUL_CLARO)]
    circulos = [(rng.uniform(0, ANCHO), rng.uniform(0, ALTO), rng.uniform(18, 95), int(rng.uniform(70, 255)), int(rng.integers(0, len(paleta)))) for _ in range(46)]
    for i, color in enumerate(paleta):
        def bokeh(d, ss, i=i):
            for px, py, rad, v, k in circulos:
                if k == i:
                    d.ellipse(((px - rad) * ss, (py - rad) * ss, (px + rad) * ss, (py + rad) * ss), fill=v)

        img = brillo(img, difuminar(mascara_dibujada(bokeh), 5), color, 0.16)

    # --- Audífonos: diadema (arco), almohadillas castañas y dos auriculares.
    radio_banda = 470.0
    grosor = 58.0
    by = cy - 20
    d_centro = np.sqrt((x - cx) ** 2 + (y - by) ** 2)
    angulo = np.arctan2(y - by, x - cx)  # arriba = -pi/2
    arriba = ((angulo < -0.12) & (angulo > -np.pi + 0.12)).astype(np.float32)

    banda = borde(np.abs(d_centro - radio_banda) - grosor / 2) * arriba
    # Sombra de la diadema.
    sombra_banda = difuminar(np.roll(banda, (26, 10), axis=(0, 1)), 18)
    img = img * (1.0 - 0.45 * sombra_banda[..., None])
    # Relleno de la diadema: más claro por fuera, con una franja verde azulada al centro.
    pos = np.clip((d_centro - (radio_banda - grosor / 2)) / grosor, 0, 1)
    img = sobre(img, banda, mezcla(c(AZUL_AUDIFONOS), c(AZUL_CLARO), suave(pos) * 0.85))
    img = sobre(img, borde(np.abs(d_centro - radio_banda) - 3.0) * arriba, c(VERDE_AZULADO), 0.9)
    # Almohadilla castaña por dentro de la diadema.
    img = sobre(img, borde(np.abs(d_centro - (radio_banda - grosor / 2 - 17)) - 12) * arriba, c(CASTANO))

    for lado in (-1, 1):
        ox = cx + lado * radio_banda
        oy = cy + 20
        # Sombra del auricular.
        sombra = borde(caja_redondeada(x, y, ox + 18, oy + 30, 100, 190, 84))
        img = img * (1.0 - 0.5 * difuminar(sombra, 20)[..., None])
        # Almohadilla (mira hacia el centro).
        pad = borde(caja_redondeada(x, y, ox - lado * 78, oy, 52, 158, 46))
        img = sobre(img, pad, mezcla(c(CASTANO), c(CASTANO_OSCURO), suave((y - oy) / 320.0 + 0.4)))
        # Cuerpo del auricular, con degradado vertical.
        cuerpo = borde(caja_redondeada(x, y, ox, oy, 98, 188, 84))
        vert = suave((y - (oy - 188)) / 376.0)
        img = sobre(img, cuerpo, mezcla(c(AZUL_CLARO) * 0.82, c(AZUL_AUDIFONOS) * 0.72, vert))
        # Marco interior más oscuro y disco del altavoz con anillos.
        marco = borde(caja_redondeada(x, y, ox, oy, 74, 164, 62))
        img = sobre(img, marco, c(AZUL_NOCHE) * 1.3, 0.92)
        dd = np.sqrt((x - ox) ** 2 + (y - oy) ** 2)
        anillos_disco = 0.5 + 0.5 * np.cos(dd * 0.55)
        img = sobre(img, borde(dd - 62), mezcla(c(VERDE_AZULADO) * 0.8, c(AZUL_CLARO), anillos_disco * 0.55))
        img = sobre(img, borde(dd - 15), c(CREMA), 0.9)
        img = brillo(img, np.exp(-(dd / 120.0) ** 2), c(VERDE_AZULADO), 0.35)
        # Brillo en el borde superior del auricular.
        reflejo = borde(caja_redondeada(x, y, ox - lado * 18, oy - 140, 38, 12, 12))
        img = sobre(img, reflejo, c(CREMA), 0.55)

        # Arcos de sonido hacia afuera de cada auricular.
        ang = np.arctan2(y - oy, (x - ox) * lado)  # 0 = hacia afuera
        dentro_ang = suave((0.62 - np.abs(ang)) * 6.0)
        for i, rad in enumerate((232, 278, 324)):
            arco = borde(np.abs(dd - rad) - 4.5) * dentro_ang
            img = sobre(img, arco, c(VERDE_AZULADO), 0.85 - 0.22 * i)

    # --- Notas musicales flotando.
    def notas(d, ss):
        for px, py, s in ((330, 330, 30), (520, 560, 20), (2230, 300, 26), (2060, 520, 18), (2330, 640, 22), (230, 760, 16), (1180, 190, 18), (1450, 130, 24)):
            nota(d, ss, px, py, s)

    capa_notas = mascara_dibujada(notas)
    img = sobre(img, capa_notas, c(CREMA), 0.55)
    img = brillo(img, difuminar(capa_notas, 10), c(VERDE_AZULADO), 0.35)

    # --- Destellos en forma de cruz.
    def destellos(d, ss):
        for _ in range(26):
            px, py = rng.uniform(80, ANCHO - 80), rng.uniform(60, ALTO - 60)
            largo = rng.uniform(9, 30) * ss
            fino = max(1, int(rng.uniform(1.2, 2.4) * ss))
            d.line((px * ss - largo, py * ss, px * ss + largo, py * ss), fill=255, width=fino)
            d.line((px * ss, py * ss - largo, px * ss, py * ss + largo), fill=255, width=fino)

    img = brillo(img, mascara_dibujada(destellos), c(CREMA), 0.8)

    # --- Viñeta y grano muy fino (evita las bandas del degradado).
    vineta = np.clip(1.28 - 0.62 * np.sqrt(((x - ANCHO / 2) / ANCHO) ** 2 * 4 + ((y - ALTO / 2) / ALTO) ** 2 * 4), 0, 1)
    img *= vineta[..., None] * 0.92 + 0.08
    img += rng.normal(0.0, 0.007, img.shape).astype(np.float32)
    return Image.fromarray((np.clip(img, 0, 1) * 255).astype(np.uint8))


def main():
    ap = argparse.ArgumentParser(description="Genera el fondo de respaldo del modo otaku")
    ap.add_argument("--salida", type=Path, default=AQUI / "otaku-respaldo.jpg")
    ap.add_argument("--vista", action="store_true", help="además, una vista pequeña en /tmp")
    args = ap.parse_args()
    img = generar()
    args.salida.parent.mkdir(parents=True, exist_ok=True)
    img.save(args.salida, quality=90, optimize=True, progressive=True)
    print(args.salida)
    if args.vista:
        img.resize((1024, 576), Image.LANCZOS).save("/tmp/otaku-respaldo-vista.jpg", quality=85)


if __name__ == "__main__":
    main()
