"""Prueba de la interfaz de Vortexine's Shop con el motor falso: carga el QML sin errores ni avisos
y saca capturas (sin ventanas visibles: plataforma «offscreen») de cada pantalla a 1180x760 y 900x700.

    python apps/shop/pruebas/ui_capturas.py [carpeta_de_capturas]

Por defecto guarda en ~/Vídeos/Vortex OS previews/tienda/. No usa red, ni flatpak, ni sonido.
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"     # nunca una ventana de verdad en el escritorio
os.environ.setdefault("VORTEX_SHOP_COLORES", "/nonexistent/qs_colors.json")   # respaldo fijo, no el fondo de Mike
os.environ.setdefault("QT_QUICK_BACKEND", "software")
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
sys.path.insert(0, str(Path(__file__).resolve().parent))

from PySide6.QtCore import (Q_ARG, QEventLoop, QMetaObject, Qt, QTimer,  # noqa: E402
                            QtMsgType, qInstallMessageHandler)

import falso_motor  # noqa: E402
from vortex_shop.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402

AVISOS: list[str] = []


def _mensajes(tipo, contexto, texto):
    if tipo in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg):
        AVISOS.append(texto)
    print(f"[qt] {texto}", file=sys.stderr)


def esperar(ms: int) -> None:
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


def llamar(ventana, nombre: str, *args) -> None:
    QMetaObject.invokeMethod(ventana, nombre, Qt.ConnectionType.DirectConnection,
                             *[Q_ARG("QVariant", a) for a in args])


def main() -> int:
    carpeta = Path(sys.argv[1]) if len(sys.argv) > 1 else Path.home() / "Vídeos" / "Vortex OS previews" / "tienda"
    carpeta.mkdir(parents=True, exist_ok=True)
    qInstallMessageHandler(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    datos = tempfile.mkdtemp(prefix="vortex-shop-ui-")
    motor = falso_motor.crear_motor(
        datos, duracion=8.0,
        instaladas=("org.mozilla.firefox", "org.videolan.VLC", "org.audacityteam.Audacity", "org.freedesktop.Platform.Locale",
                    "org.example.Desconocida"),
        actualizaciones=("org.mozilla.firefox", "org.videolan.VLC"))
    ctl = motor.control
    ctl.del_sistema.add("org.audacityteam.Audacity")   # se ve «Instalada por el sistema»
    interfaz, puente = cargar_interfaz(app, motor)
    if not interfaz.rootObjects():
        print("FALLO: el QML no cargó", file=sys.stderr)
        return 1
    v = interfaz.rootObjects()[0]
    puente.iniciar()

    def foto(nombre: str, espera: int = 700) -> None:
        esperar(espera)
        sufijo = "" if v.width() >= 1100 else "-900"
        ruta = carpeta / f"{nombre}{sufijo}.png"
        img = v.grabWindow()
        assert img.save(str(ruta)), ruta
        print("captura", ruta)

    def recorrido(num0: int) -> None:
        n = num0
        ctl.sin_red = False
        llamar(v, "navegar", "inicio", "")
        foto(f"{n:02d}-inicio", 1200); n += 1
        llamar(v, "navegar", "categoria", "Game")
        foto(f"{n:02d}-categoria", 900); n += 1
        llamar(v, "escribirBusqueda", "video")
        foto(f"{n:02d}-busqueda", 900); n += 1
        llamar(v, "verDetalle", "com.valvesoftware.Steam")
        foto(f"{n:02d}-detalle", 1200); n += 1
        # Instalar GIMP desde su detalle y dos más desde el inicio: una en curso y dos en cola.
        puente.instalar("com.valvesoftware.Steam")
        esperar(100)
        puente.instalar("net.lutris.Lutris")   # en el inicio se ven tres tarjetas con su cola
        puente.instalar("org.chromium.Chromium")
        esperar(3800)
        foto(f"{n:02d}-detalle-instalando", 200); n += 1
        llamar(v, "navegar", "inicio", "")
        foto(f"{n:02d}-instalando", 700); n += 1
        llamar(v, "navegar", "instaladas", "")
        foto(f"{n:02d}-instaladas", 900); n += 1
        llamar(v, "navegar", "actualizaciones", "")
        foto(f"{n:02d}-actualizaciones", 900); n += 1
        llamar(v, "escribirBusqueda", "zzzzzz")
        foto(f"{n:02d}-vacio", 900); n += 1
        ctl.sin_red = True
        llamar(v, "navegar", "inicio", "")
        puente.reintentarTodo()
        foto(f"{n:02d}-sin-red", 1500); n += 1
        ctl.sin_red = False
        llamar(v, "navegar", "instaladas", "")
        llamar(v, "pedirQuitar", "org.videolan.VLC", "VLC")
        foto(f"{n:02d}-quitar-confirmar", 500); n += 1
        llamar(v, "cerrarDialogos")
        v.close()   # con instalaciones en curso: pregunta si de verdad quiere salir
        foto(f"{n:02d}-salir-confirmar", 500); n += 1
        llamar(v, "cerrarDialogos")
        puente.aviso.emit("No se pudo instalar Blender: se acabó el espacio en disco. Libera espacio y vuelve a intentarlo.")
        foto(f"{n:02d}-aviso-error", 500); n += 1
        # Esperar a que termine la cola para dejar el estado limpio entre recorridos.
        esperar(1000)

    v.resize(1180, 760)
    esperar(300)
    recorrido(1)
    # Se espera a que acaben las instalaciones de la primera vuelta (3 x 8 s en cola de una en una).
    esperar(20000)
    v.resize(900, 700)
    esperar(400)
    ctl.instaladas = [i for i in ctl.instaladas if i not in ("com.valvesoftware.Steam", "net.lutris.Lutris", "org.chromium.Chromium")]
    puente.reintentarTodo()
    recorrido(1)

    esperar(500)
    if AVISOS:
        print("\nAVISOS DE QT/QML:", *AVISOS, sep="\n  ", file=sys.stderr)
        return 1
    print("OK: sin errores ni avisos de QML")
    return 0


if __name__ == "__main__":
    codigo = main()
    # Quedan instalaciones falsas en la cola: no se espera a que acaben para cerrar.
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)
