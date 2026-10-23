"""Capturas de la interfaz con LOGOS y CAPTURAS REALES de Flathub (necesita red): el motor sigue siendo
el falso (nada se instala), pero las URLs de íconos y capturas son las https de Flathub guardadas por la
prueba del motor en datos/*.json. Espera a que todas las imágenes remotas terminen de cargar y avisa de
las que no.

    python apps/shop/pruebas/ui_capturas_real.py <carpeta_con_los_json_de_flathub> [carpeta_de_capturas]
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("VORTEX_SHOP_COLORES", "/nonexistent/qs_colors.json")   # respaldo fijo, no el fondo de Mike
os.environ.setdefault("QT_QUICK_BACKEND", "software")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from PySide6.QtCore import Q_ARG, QEventLoop, QMetaObject, Qt, QTimer, qInstallMessageHandler  # noqa: E402
from PySide6.QtQml import QQmlProperty  # noqa: E402

import falso_motor  # noqa: E402
from vortex_shop.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402


def _mensajes(tipo, contexto, texto):
    print(f"[qt] {texto}", file=sys.stderr)


def esperar(ms: int) -> None:
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


def llamar(v, nombre, *args):
    QMetaObject.invokeMethod(v, nombre, Qt.ConnectionType.DirectConnection, *[Q_ARG("QVariant", a) for a in args])


def imagenes_remotas(v):
    """({listas, cargando, error}, urls con error): las Image con fuente http(s) que hay ahora en la ventana."""
    cuantas = {"listas": 0, "cargando": 0, "error": 0}
    errores = []
    pendientes = [v.contentItem()]
    todos = []
    while pendientes:   # los hijos visuales de QML no son hijos QObject: se recorre childItems()
        item = pendientes.pop()
        todos.append(item)
        pendientes.extend(item.childItems())
    for item in todos:
        if item.metaObject().className().startswith("QQuickImage"):
            fuente = item.property("source")
            if fuente is not None and fuente.scheme() in ("http", "https"):
                # El enum Image.status no se puede leer desde Python: una imagen lista tiene tamaño pintado.
                if item.property("paintedWidth") > 0 and item.property("progress") == 1:
                    cuantas["listas"] += 1
                else:
                    cuantas["cargando"] += 1
                    errores.append(fuente.toString())
    return cuantas, errores


def cargadas(v, limite_s=40):
    for _ in range(limite_s * 4):
        esperar(250)
        c, e = imagenes_remotas(v)
        if c["cargando"] == 0 and c["listas"] > 0:
            return c, e
    c, e = imagenes_remotas(v)   # lo que siga sin cargar tras el límite cuenta como error
    return {"listas": c["listas"], "cargando": 0, "error": c["cargando"]}, e


def main() -> int:
    datos = Path(sys.argv[1])
    carpeta = Path(sys.argv[2]) if len(sys.argv) > 2 else Path.home() / "Vídeos" / "Vortex OS previews" / "tienda"
    carpeta.mkdir(parents=True, exist_ok=True)
    from PySide6.QtCore import qInstallMessageHandler as instalar
    instalar(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    reales = falso_motor.cargar_datos_reales(datos)
    print(f"{len(reales)} apps con datos reales")
    motor = falso_motor.crear_motor(tempfile.mkdtemp(prefix="vortex-shop-real-"), apps_reales=reales, duracion=8.0,
                                    latencia=0.1, instaladas=("org.videolan.VLC",), actualizaciones=("org.videolan.VLC",))
    interfaz, puente = cargar_interfaz(app, motor)
    v = interfaz.rootObjects()[0]
    v.resize(1180, 760)
    puente.iniciar()
    malas = 0

    def foto(nombre):
        nonlocal malas
        c, e = cargadas(v)
        malas += len(e)
        ruta = carpeta / f"{nombre}-real.png"
        v.grabWindow().save(str(ruta))
        print("captura", ruta, c, e)

    foto("01-inicio")
    llamar(v, "verDetalle", "com.valvesoftware.Steam")
    foto("04-detalle")
    puente.instalar("com.valvesoftware.Steam")
    puente.instalar("com.spotify.Client")
    llamar(v, "navegar", "inicio", "")
    esperar(3500)
    foto("06-instalando")
    print("imágenes con error:", malas)
    return 1 if malas else 0


if __name__ == "__main__":
    codigo = main()
    sys.stdout.flush()
    os._exit(codigo)
