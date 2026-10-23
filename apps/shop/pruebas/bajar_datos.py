#!/usr/bin/env python3
"""Baja de la API de Flathub las respuestas de muestra que usa el servidor falso de las pruebas
(apps/shop/pruebas/datos/). Se corre una vez (o para refrescarlas); las pruebas nunca lo corren.

    python3 apps/shop/pruebas/bajar_datos.py

Son respuestas reales, recortadas: las listas a unas pocas apps, y las fichas sin su historial largo
de versiones. Todo lo demás queda tal como lo manda Flathub.
"""
import json
import os
import sys
import urllib.request

API = "https://flathub.org/api/v2"
AQUI = os.path.dirname(os.path.abspath(__file__))
DESTINO = os.path.join(AQUI, "datos")
CABECERAS = {"User-Agent": "vortexines-shop/1.0", "Content-Type": "application/json"}

# Fichas que sirve el servidor falso (las demás recomendadas devuelven 404, como si ya no existieran).
FICHAS = ["org.prismlauncher.PrismLauncher", "com.valvesoftware.Steam", "org.mozilla.firefox",
          "org.videolan.VLC", "com.spotify.Client", "org.vinegarhq.Sober"]
RESUMENES = ["org.prismlauncher.PrismLauncher"]


def pedir(ruta, cuerpo=None):
    datos = json.dumps(cuerpo).encode() if cuerpo is not None else None
    with urllib.request.urlopen(urllib.request.Request(API + ruta, data=datos, headers=CABECERAS), timeout=20) as r:
        return json.load(r)


def guardar(nombre, datos):
    with open(os.path.join(DESTINO, nombre), "w", encoding="utf-8") as f:
        json.dump(datos, f, ensure_ascii=False, indent=1, sort_keys=True)
        f.write("\n")
    print("  ", nombre)


def recortar(datos, cantidad):
    datos["hits"] = datos["hits"][:cantidad]
    return datos


def main():
    os.makedirs(DESTINO, exist_ok=True)
    guardar("categorias.json", pedir("/collection/category"))
    guardar("trending.json", recortar(pedir("/collection/trending?page=1&per_page=12&locale=es"), 12))
    guardar("categoria_game.json",
            recortar(pedir("/collection/category/game?page=1&per_page=30&sort_by=installs_last_month&locale=es"), 8))
    guardar("busqueda_minecraft.json", recortar(pedir("/search?locale=es", {"query": "minecraft", "hits_per_page": 30,
                                                                             "page": 1}), 8))
    hoy = __import__("time").strftime("%Y-%m-%d", __import__("time").gmtime())
    guardar("app_del_dia.json", pedir("/app-picks/app-of-the-day/" + hoy))
    guardar("apps_de_la_semana.json", pedir("/app-picks/apps-of-the-week/" + hoy))
    for app_id in FICHAS:
        ficha = pedir("/appstream/%s?locale=es" % app_id)
        ficha["releases"] = ficha.get("releases", [])[:3]
        guardar("appstream_%s.json" % app_id, ficha)
    for app_id in RESUMENES:
        resumen = pedir("/summary/" + app_id)
        guardar("resumen_%s.json" % app_id, resumen)
    return 0


if __name__ == "__main__":
    sys.exit(main())
