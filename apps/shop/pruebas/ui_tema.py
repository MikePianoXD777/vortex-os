"""Prueba de los colores que se adaptan al fondo: con dos paletas falsas de matugen (A: azul oscuro,
B: clara y rosa) y con el respaldo (archivo roto), comprueba el contraste y el cambio en vivo (el archivo
se reemplaza como lo hace matugen) y saca 01-inicio-tema-A.png, -B.png y -respaldo.png.

    python apps/shop/pruebas/ui_tema.py [carpeta_de_capturas]

No toca el archivo real de colores del shell: usa VORTEX_SHOP_COLORES con archivos temporales.
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("QT_QUICK_BACKEND", "software")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from PySide6.QtCore import QEventLoop, QTimer, QtMsgType, qInstallMessageHandler  # noqa: E402

import falso_motor  # noqa: E402
from vortex_shop import colores as col  # noqa: E402
from vortex_shop.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402

# Paletas con las claves de serpantinum_matugen_colors.json.template (valores tipo Material You).
PALETA_A = {  # azul oscuro
    "base": "#0a0f1f", "mantle": "#121829", "crust": "#0e1322", "text": "#dfe2f3", "subtext0": "#c3c6d6",
    "subtext1": "#8d90a0", "surface0": "#1a1f31", "surface1": "#242a3c", "surface2": "#2f3547",
    "overlay0": "#dfe2f3", "overlay1": "#dfe2f3", "overlay2": "#dfe2f3", "blue": "#b2c5ff", "sapphire": "#304578",
    "peach": "#e0bbdd", "green": "#c3c6dc", "red": "#ffb4ab", "mauve": "#b2c5ff", "pink": "#5a3d5a",
    "yellow": "#434659", "maroon": "#93000a", "teal": "#c3c6dc"}
PALETA_B = {  # fondo claro rosado
    "base": "#ffffff", "mantle": "#fff0f4", "crust": "#fff8f9", "text": "#26181c", "subtext0": "#534347",
    "subtext1": "#857377", "surface0": "#fbe9ee", "surface1": "#f5e3e8", "surface2": "#efdde2",
    "overlay0": "#3c2d31", "overlay1": "#3c2d31", "overlay2": "#3c2d31", "blue": "#8c4a60", "sapphire": "#ffd9e2",
    "peach": "#7a5732", "green": "#745a60", "red": "#ba1a1a", "mauve": "#8c4a60", "pink": "#ffddb5",
    "yellow": "#ffd9e2", "maroon": "#ffdad6", "teal": "#745a60"}

FALLOS: list[str] = []
AVISOS_QT: list[str] = []


def comprobar(cond: bool, texto: str) -> None:
    print(("ok    " if cond else "FALLA ") + texto)
    if not cond:
        FALLOS.append(texto)


def _mensajes(tipo, contexto, texto):
    if tipo in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg):
        AVISOS_QT.append(texto)
        print(f"[qt] {texto}", file=sys.stderr)


def esperar(ms: int) -> None:
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


def escribir_como_matugen(ruta: Path, contenido: str) -> None:
    """Reemplaza el archivo (archivo temporal + rename): el inodo cambia y el vigilante lo pierde."""
    tmp = ruta.with_suffix(".tmp")
    tmp.write_text(contenido, encoding="utf-8")
    os.replace(tmp, ruta)


def revisar_contraste(nombre: str, p: dict[str, str]) -> None:
    for fondo in ("fondo", "barra", "tarjeta", "campo"):
        comprobar(col.contraste(p["texto"], p[fondo]) >= 4.5, f"{nombre}: texto sobre {fondo} >= 4.5")
        comprobar(col.contraste(p["suave"], p[fondo]) >= 4.5, f"{nombre}: texto suave sobre {fondo} >= 4.5")
        comprobar(col.contraste(p["tenue"], p[fondo]) >= 4.5, f"{nombre}: texto tenue sobre {fondo} >= 4.5")
    for estado in ("azul", "azulHover", "azulPresion"):
        comprobar(col.contraste(p["sobreAcento"], p[estado]) >= 4.5, f"{nombre}: texto sobre el botón ({estado}) >= 4.5")
    comprobar(col.contraste(p["azul"], p["fondo"]) >= 4.5 and col.contraste(p["violeta"], p["tarjeta"]) >= 4.5,
              f"{nombre}: acentos legibles como texto")


def main() -> int:
    carpeta = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "Vídeos" / "Vortex OS previews" / "tienda"
    carpeta.mkdir(parents=True, exist_ok=True)
    qInstallMessageHandler(_mensajes)
    tmp = Path(tempfile.mkdtemp(prefix="vortex-shop-tema-"))
    archivo = tmp / "qs_colors.json"
    archivo.write_text(json.dumps(PALETA_A), encoding="utf-8")
    os.environ["VORTEX_SHOP_COLORES"] = str(archivo)

    # Lógica pura: contraste de las dos paletas y el respaldo.
    a, b = col.desde_matugen(PALETA_A), col.desde_matugen(PALETA_B)
    comprobar(a is not None and b is not None, "las paletas de matugen se convierten")
    revisar_contraste("A", a)
    revisar_contraste("B", b)
    revisar_contraste("respaldo", col.RESPALDO)
    comprobar(col.desde_matugen({"base": "#000000"}) is None, "una paleta incompleta se rechaza (cae al respaldo)")
    mala = dict(PALETA_A, blue="#3a4a80")   # un primario que no daría contraste con texto oscuro ni con el fondo
    revisar_contraste("A con primario flojo", col.desde_matugen(mala))

    app = crear_aplicacion(sys.argv[:1])
    motor = falso_motor.crear_motor(tmp / "datos", duracion=3.0, latencia=0.1,
                                    instaladas=("org.mozilla.firefox", "org.videolan.VLC"), actualizaciones=("org.mozilla.firefox",))
    interfaz, puente = cargar_interfaz(app, motor)
    v = interfaz.rootObjects()[0]
    v.resize(1180, 760)
    puente.iniciar()
    colores = app.findChild(col.Colores)

    def foto(nombre: str) -> None:
        esperar(900)
        ruta = carpeta / f"01-inicio-tema-{nombre}.png"
        v.grabWindow().save(str(ruta))
        print("captura", ruta)

    esperar(1500)
    comprobar(colores.paleta["fondo"] == a["fondo"], "al arrancar toma la paleta A del archivo")
    foto("A")

    # Matugen reescribe el archivo (reemplazo) al cambiar el fondo: la tienda lo ve y cambia en vivo.
    escribir_como_matugen(archivo, json.dumps(PALETA_B))
    esperar(700)
    comprobar(colores.paleta["fondo"] == b["fondo"], "al reemplazar el archivo cambia a la paleta B (en vivo)")
    foto("B")
    escribir_como_matugen(archivo, json.dumps(PALETA_A))   # otro cambio: sigue vigilando tras el primero
    esperar(700)
    comprobar(colores.paleta["fondo"] == a["fondo"], "tras el reemplazo sigue vigilando: vuelve a la paleta A")

    # Archivo roto: respaldo fijo de Vortexine's.
    escribir_como_matugen(archivo, "{esto no es json")
    esperar(700)
    comprobar(colores.paleta["fondo"] == col.RESPALDO["fondo"], "archivo roto: usa la paleta de respaldo")
    foto("respaldo")
    # Archivo borrado y vuelto a crear.
    archivo.unlink()
    esperar(500)
    comprobar(colores.paleta["fondo"] == col.RESPALDO["fondo"], "archivo borrado: sigue con el respaldo")
    archivo.write_text(json.dumps(PALETA_B), encoding="utf-8")
    esperar(700)
    comprobar(colores.paleta["fondo"] == b["fondo"], "archivo creado de nuevo: se toma la paleta")

    comprobar(not AVISOS_QT, "el QML no dejó errores ni avisos")
    print("\nTODO BIEN" if not FALLOS else f"\nFALLARON {len(FALLOS)}: " + "; ".join(FALLOS))
    return 1 if FALLOS else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)
