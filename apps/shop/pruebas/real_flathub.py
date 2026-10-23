#!/usr/bin/env python3
"""Prueba REAL y de solo lectura contra la API de Flathub: busca «minecraft», pide la ficha de Prism
Launcher, las categorías y las recomendadas. No instala nada ni toca el sistema (la caché va a una
carpeta temporal). La corre pruebas/tienda.sh si hay internet.

    python3 -I apps/shop/pruebas/real_flathub.py

Sale con 0 si todo bien, 1 si algo falla y 2 si no hay internet (la prueba se salta).
"""
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from vortex_shop.catalogo import RECOMENDADAS, Catalogo        # noqa: E402
from vortex_shop.errores import SinRed                          # noqa: E402

PRISM = "org.prismlauncher.PrismLauncher"
FALLOS = []


def revisar(nombre, condicion, detalle=""):
    print("  %s %s%s" % ("ok " if condicion else "MAL", nombre, (" (%s)" % detalle) if detalle and not condicion else ""))
    if not condicion:
        FALLOS.append(nombre)


def main():
    cache = tempfile.mkdtemp(prefix="vortex-tienda-real.")
    try:
        os.environ.pop("VORTEX_SHOP_API", None)
        cat = Catalogo(cache_dir=cache)
        try:
            encontradas = cat.buscar("minecraft")
        except SinRed:
            print("  --  sin internet: se salta la prueba real")
            return 2
        revisar("buscar «minecraft» devuelve apps", len(encontradas) >= 5, len(encontradas))
        revisar("Prism Launcher está en la búsqueda", PRISM in [a.id for a in encontradas])
        revisar("las apps traen nombre, ícono y descargas", all(a.nombre and a.icono_url.startswith("https://")
                                                              for a in encontradas) and
                any(a.descargas_mes for a in encontradas))

        d = cat.detalle(PRISM)
        revisar("detalle: nombre", d.nombre == "Prism Launcher", d.nombre)
        revisar("detalle: descripción en texto plano con viñetas", "• " in d.descripcion and "<" not in d.descripcion)
        revisar("detalle: capturas", len(d.capturas) >= 1 and d.capturas[0].startswith("https://"))
        revisar("detalle: licencia, web, versión", bool(d.licencia and d.web and d.version),
                (d.licencia, d.web, d.version))
        revisar("detalle: tamaño de descarga", bool(d.tamano_descarga and d.tamano_descarga > 1_000_000), d.tamano_descarga)
        revisar("detalle: verificada y con categorías", d.verificado and "Juegos" in d.categorias, d.categorias)

        cats = cat.categorias()
        revisar("categorías", len(cats) >= 8 and cats[0].id == "game" and cats[0].nombre == "Juegos", [c.id for c in cats])
        juegos = cat.por_categoria("game")
        revisar("juegos: primera página llena", len(juegos) == Catalogo.POR_PAGINA, len(juegos))
        revisar("juegos: segunda página distinta", cat.por_categoria("game", 2)[0].id != juegos[0].id)

        recomendadas = cat.recomendadas()
        faltan = [i for i in RECOMENDADAS if i not in [a.id for a in recomendadas]]
        revisar("todas las recomendadas existen en Flathub", not faltan, faltan)
        revisar("destacados", len(cat.destacados()) >= 6)
    finally:
        shutil.rmtree(cache, ignore_errors=True)
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
