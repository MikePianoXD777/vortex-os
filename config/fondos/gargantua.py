#!/usr/bin/env python3
"""Genera el fondo animado «Gargantua» de Vortexine's OS: un agujero negro con su disco de acreción, por código.

No es la imagen de la película: es un agujero negro de Schwarzschild de verdad. Cada rayo de luz se sigue hacia
atrás desde la cámara (la órbita de un fotón, d²x/dt² = -3/2·h²·x/r⁵ con el radio del horizonte = 1), y donde
choca con el disco se toma su color. Por eso se ve el disco de detrás doblado por encima y por debajo del
agujero, y las estrellas deformadas alrededor.

El disco gira como en una órbita de Kepler (más rápido por dentro). Cada anillo da un número entero de vueltas
en lo que dura el video, así que el bucle no tiene costura.

    python3 config/fondos/gargantua.py                   # config/fondos/gargantua.mp4 (1920x1080, 24 s)
    python3 config/fondos/gargantua.py --vista /tmp/g.png   # solo un cuadro, para ver cómo queda
    (--cache ARCHIVO.npz guarda los rayos ya seguidos: así probar colores no repite lo lento)

Necesita numpy, Pillow y ffmpeg. Tarda unos minutos (casi todo es seguir los ~8 millones de rayos una vez).
"""

import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageFilter

ANCHO, ALTO = 1920, 1080
SUPER = 2                       # rayos por lado de cada píxel (2x2), para bordes finos
FPS, SEGUNDOS = 30, 24
AQUI = Path(__file__).resolve().parent

# Escena (radio del horizonte = 1). La órbita estable más interna está en r = 3.
CAMARA_DIST, CAMARA_ALTURA = 26.0, 1.6      # un poco por encima del disco, como en las fotos clásicas
FOV_HORIZONTAL = np.radians(62)
R_IN, R_OUT = 3.0, 15.0
SEMILLA = 2014


def rayos(ancho, alto):
    """Posición y dirección inicial de cada rayo, desde la cámara hacia el agujero (en el origen)."""
    cam = np.array([0.0, CAMARA_ALTURA, -CAMARA_DIST], dtype=np.float64)
    frente = -cam / np.linalg.norm(cam)
    derecha = np.cross(np.array([0.0, 1.0, 0.0]), frente)
    derecha /= np.linalg.norm(derecha)
    arriba = np.cross(frente, derecha)
    tan_x = np.tan(FOV_HORIZONTAL / 2)
    tan_y = tan_x * alto / ancho
    u = (np.arange(ancho) + 0.5) / ancho * 2 - 1
    v = 1 - (np.arange(alto) + 0.5) / alto * 2
    uu, vv = np.meshgrid(u * tan_x, v * tan_y)
    d = frente[None, None, :] + uu[..., None] * derecha + vv[..., None] * arriba
    d /= np.linalg.norm(d, axis=-1, keepdims=True)
    d = d.reshape(-1, 3)
    x = np.broadcast_to(cam, d.shape).copy()
    return x, d


def trazar(ancho, alto):
    """Sigue cada rayo. Devuelve, por rayo: radio y ángulo donde chocó con el disco (o NaN), si cayó al
    horizonte, y la dirección final de los que escaparon (para las estrellas)."""
    x, v = rayos(ancho, alto)
    n = len(x)
    r_disco = np.full(n, np.nan, np.float32)
    phi_disco = np.full(n, np.nan, np.float32)
    horizonte = np.zeros(n, bool)
    salida = np.zeros((n, 3), np.float32)
    h2 = np.sum(np.cross(x, v) ** 2, axis=1)            # se conserva a lo largo de la órbita
    vivos = np.arange(n)
    paso = 0
    while len(vivos) and paso < 4000:
        paso += 1
        r = np.linalg.norm(x, axis=1)
        dt = np.clip(0.06 * r, 0.015, 0.8)[:, None]
        a = -1.5 * h2[:, None] * x / r[:, None] ** 5
        v_med = v + 0.5 * dt * a
        x_nuevo = x + dt * v_med
        r_n = np.linalg.norm(x_nuevo, axis=1)
        a_n = -1.5 * h2[:, None] * x_nuevo / r_n[:, None] ** 5
        v = v_med + 0.5 * dt * a_n

        # ¿Cruzó el plano del disco (y = 0) en este paso?
        cruza = (x[:, 1] * x_nuevo[:, 1]) < 0
        fin = np.zeros(len(vivos), bool)
        if cruza.any():
            i = np.nonzero(cruza)[0]
            f = x[i, 1] / (x[i, 1] - x_nuevo[i, 1])
            p = x[i] + f[:, None] * (x_nuevo[i] - x[i])
            rr = np.hypot(p[:, 0], p[:, 2])
            toca = (rr >= R_IN) & (rr <= R_OUT)
            j = i[toca]
            r_disco[vivos[j]] = rr[toca]
            phi_disco[vivos[j]] = np.arctan2(p[toca, 2], p[toca, 0])
            fin[j] = True
        cae = (r_n < 1.0) & ~fin
        horizonte[vivos[cae]] = True
        fin |= cae
        escapa = (r_n > CAMARA_DIST * 1.6) & (np.sum(x_nuevo * v, axis=1) > 0) & ~fin
        salida[vivos[escapa]] = v[escapa]
        fin |= escapa

        x, v, h2, vivos = x_nuevo[~fin], v[~fin], h2[~fin], vivos[~fin]
        if paso % 200 == 0:
            print(f"  paso {paso}: quedan {len(vivos)} rayos", file=sys.stderr, flush=True)
    horizonte[vivos] = True                          # los que siguen dando vueltas: sombra
    return r_disco, phi_disco, horizonte, salida


def estrellas(salida, escapo, rng):
    """Cielo de estrellas según hacia dónde salió cada rayo (la lente las deforma sola). En cada celda de una
    rejilla del cielo puede haber una estrella, en un punto al azar de la celda: se pinta como un puntito
    (gaussiana de ~1 píxel), así cerca del agujero se estira en arcos."""
    d = salida[escapo]
    lon = np.arctan2(d[:, 0], d[:, 2])
    lat = np.arcsin(np.clip(d[:, 1], -1, 1))
    celdas_lon, celdas_lat = 3000, 1500
    fx = (lon / (2 * np.pi) + 0.5) * celdas_lon
    fy = np.clip((lat / np.pi + 0.5) * celdas_lat, 0, celdas_lat - 1e-3)
    ci, cj = fx.astype(np.int64) % celdas_lon, fy.astype(np.int64)
    hay = rng.random((celdas_lat, celdas_lon)) > 0.97
    brillo = np.where(hay, rng.pareto(2.2, (celdas_lat, celdas_lon)) * 0.35 + 0.15, 0.0).astype(np.float32)
    ox = rng.random((celdas_lat, celdas_lon)).astype(np.float32)
    oy = rng.random((celdas_lat, celdas_lon)).astype(np.float32)
    tono = rng.random((celdas_lat, celdas_lon)).astype(np.float32)
    celda = 2 * np.pi / celdas_lon                       # radianes por celda
    sigma = FOV_HORIZONTAL / (ANCHO * SUPER) * 1.3       # ~1 píxel
    dx = (fx - np.floor(fx) - ox[cj, ci]) * celda * np.cos(lat)
    dy = (fy - np.floor(fy) - oy[cj, ci]) * celda
    b = brillo[cj, ci] * np.exp(-(dx * dx + dy * dy) / (2 * sigma * sigma))
    t = tono[cj, ci][:, None]
    color = (1 - t) * np.array([0.75, 0.85, 1.0]) + t * np.array([1.0, 0.9, 0.75])
    cielo = np.zeros((len(salida), 3), np.float32)
    cielo[escapo] = color * b[:, None] * 0.9
    return cielo


class Disco:
    """Textura del disco: anillos finos, cada uno una franja de ruido periódica en el ángulo que gira un número
    entero de vueltas por ciclo (más por dentro). Entre anillos se interpola: no hay cortes."""

    def __init__(self, rng, anillos=480, muestras=4096):
        self.anillos, self.muestras = anillos, muestras
        self.r_anillo = np.linspace(R_IN, R_OUT, anillos)
        # Vueltas por ciclo: Kepler (ω ∝ r^-1.5), con 4 vueltas en el borde interno.
        self.vueltas = np.maximum(1, np.round(4 * (R_IN / self.r_anillo) ** 1.5)).astype(np.float64)
        ang = np.linspace(0, 2 * np.pi, muestras, endpoint=False)
        m = np.arange(1, 97)
        amp = rng.normal(size=(anillos, len(m))) / m ** 0.9
        fase = rng.random((anillos, len(m))) * 2 * np.pi
        g = np.einsum("am,amk->ak", amp, np.cos(m[None, :, None] * ang[None, None, :] + fase[..., None]))
        g = (g - g.mean(axis=1, keepdims=True)) / (g.std(axis=1, keepdims=True) + 1e-6)
        self.franjas = (0.62 + 0.38 * np.tanh(0.8 * g)).astype(np.float32)
        # Anillos más y menos densos (fijos), para que tenga estructura.
        radial = rng.normal(size=anillos)
        radial = np.convolve(radial, np.ones(9) / 9, mode="same")
        self.densidad = (0.75 + 0.25 * np.tanh(radial * 2.5)).astype(np.float32)

    def brillo(self, r, phi, t):
        """t de 0 a 1 (un ciclo)."""
        s = (r - R_IN) / (R_OUT - R_IN) * (self.anillos - 1)
        j0 = np.clip(np.floor(s).astype(np.int64), 0, self.anillos - 2)
        f = (s - j0).astype(np.float32)
        out = np.zeros_like(r)
        for j, w in ((j0, 1 - f), (j0 + 1, f)):
            pos = (phi / (2 * np.pi) - self.vueltas[j] * t) % 1.0 * self.muestras
            k0 = pos.astype(np.int64) % self.muestras
            fk = (pos - np.floor(pos)).astype(np.float32)
            val = self.franjas[j, k0] * (1 - fk) + self.franjas[j, (k0 + 1) % self.muestras] * fk
            out += w * val * self.densidad[j]
        # Perfil: brillante junto al borde interno, se apaga hacia fuera; bordes suaves.
        perfil = (R_IN / r) ** 1.35
        perfil *= np.clip((r - R_IN) / 0.35, 0, 1) * np.clip((R_OUT - r) / 4.0, 0, 1)
        return out * perfil


def color_disco(b):
    """De naranja oscuro a blanco cálido según el brillo (como un cuerpo que se calienta)."""
    b = b[:, None]
    oscuro = np.array([0.55, 0.16, 0.02])
    medio = np.array([1.0, 0.55, 0.18])
    claro = np.array([1.0, 0.93, 0.80])
    c = np.where(b < 0.5, oscuro + (medio - oscuro) * (b / 0.5), medio + (claro - medio) * np.clip((b - 0.5) / 0.5, 0, 1))
    return c * (b * 3.6)


def armar_cuadro(t, datos):
    r_d, phi_d, toca, cielo, disco = datos
    luz = cielo.copy()
    luz[toca] = color_disco(disco.brillo(r_d[toca], phi_d[toca], t))
    img = luz.reshape(ALTO * SUPER, ANCHO * SUPER, 3)
    img = img.reshape(ALTO, SUPER, ANCHO, SUPER, 3).mean(axis=(1, 3))
    # Resplandor: lo brillante se difumina alrededor (más barato a un cuarto de tamaño).
    brillante = np.clip(img - 0.55, 0, None)
    peq = Image.fromarray((np.clip(brillante, 0, 1) * 255).astype(np.uint8)).resize((ANCHO // 4, ALTO // 4), Image.BILINEAR)
    glow = np.asarray(peq.filter(ImageFilter.GaussianBlur(10)).resize((ANCHO, ALTO), Image.BILINEAR), np.float32) / 255
    img = img + glow * 0.9
    img = 1 - np.exp(-img * 1.15)                    # tono: sin quemar los blancos
    return (np.clip(img, 0, 1) ** (1 / 1.1) * 255).astype(np.uint8)


def main() -> int:
    vista = sys.argv[sys.argv.index("--vista") + 1] if "--vista" in sys.argv else None
    cache = Path(sys.argv[sys.argv.index("--cache") + 1]) if "--cache" in sys.argv else None   # para afinar el color
    rng = np.random.default_rng(SEMILLA)
    if cache and cache.is_file():
        z = np.load(cache)
        r_d, phi_d, horizonte, salida = z["r"], z["phi"], z["h"], z["s"]
    else:
        print("Siguiendo los rayos…", file=sys.stderr, flush=True)
        r_d, phi_d, horizonte, salida = trazar(ANCHO * SUPER, ALTO * SUPER)
        if cache:
            np.savez(cache, r=r_d, phi=phi_d, h=horizonte, s=salida)
    toca = ~np.isnan(r_d)
    escapo = ~toca & ~horizonte
    cielo = estrellas(salida, escapo, rng)
    datos = (r_d, phi_d, toca, cielo, Disco(rng))
    if vista:
        Image.fromarray(armar_cuadro(0.0, datos)).save(vista)
        print(vista)
        return 0
    destino = AQUI / "gargantua.mp4"
    cuadros = FPS * SEGUNDOS
    ff = subprocess.Popen(["ffmpeg", "-y", "-hide_banner", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24",
                           "-s", f"{ANCHO}x{ALTO}", "-r", str(FPS), "-i", "-", "-c:v", "libx264", "-preset", "slow",
                           "-crf", "22", "-tune", "film", "-g", str(FPS * 2), "-pix_fmt", "yuv420p",
                           "-movflags", "+faststart", str(destino)], stdin=subprocess.PIPE)
    for i in range(cuadros):
        ff.stdin.write(armar_cuadro(i / cuadros, datos).tobytes())
        if i % 60 == 0:
            print(f"  cuadro {i}/{cuadros}", file=sys.stderr, flush=True)
    ff.stdin.close()
    if ff.wait() != 0:
        return 1
    print(destino)
    return 0


if __name__ == "__main__":
    sys.exit(main())
