"""Arranque de Vortex Player (python -m vortex_player [ENLACE | ARCHIVO…]).

Una sola ventana por persona: si ya hay una abierta, esta instancia le pasa lo que recibió (un enlace
para bajar o archivos para agregar) por un socket local y sale. La clase de ventana (app_id en Wayland)
es «vortex-player», que es también el nombre de su lanzador (.desktop).
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from PySide6.QtCore import QObject, QUrl
from PySide6.QtGui import QFont, QGuiApplication, QIcon
from PySide6.QtNetwork import QLocalServer, QLocalSocket
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle

from . import APP_ID, NOMBRE
from .colores import Colores
from .puente import Puente

QML = Path(__file__).resolve().parent / "qml"


def _avisar_a_la_abierta(nombre: str, args: list[str]) -> bool:
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.write(("\n".join(["abrir"] + args) + "\n\n").encode("utf-8", "surrogateescape"))
    socket.waitForBytesWritten(400)
    socket.disconnectFromServer()
    return True


def _enfocar_en_hyprland() -> None:
    hyprctl = shutil.which("hyprctl")
    if not hyprctl or not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        return
    ventana = f"pid:{os.getpid()}"
    for orden in (f'hl.dsp.focus({{ window = "{ventana}" }})', f"focuswindow {ventana}"):
        try:
            if subprocess.run([hyprctl, "dispatch", orden], capture_output=True, timeout=3).returncode == 0:
                return
        except (OSError, subprocess.TimeoutExpired):
            return


def recibir(puente: Puente, args: list[str]) -> None:
    """Enlaces → a bajar; archivos o carpetas → a la biblioteca."""
    archivos = []
    for a in args:
        if a.startswith(("http://", "https://", "spotify:")) or a.startswith(("music.youtube", "youtu", "open.spotify")):
            puente.bajar(a)
        elif a:
            archivos.append(a)
    if archivos:
        puente.agregarArchivos(archivos)


def _escuchar(nombre: str, puente: Puente, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)
    servidor = QLocalServer(padre)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)

    def leer(socket: QLocalSocket) -> None:
        lineas = bytes(socket.readAll()).decode("utf-8", "surrogateescape").splitlines()
        if lineas and lineas[0] == "abrir":
            recibir(puente, [x for x in lineas[1:] if x])
            puente.mostrarVentana.emit()
            _enfocar_en_hyprland()

    def conexion() -> None:
        while (socket := servidor.nextPendingConnection()) is not None:
            socket.readyRead.connect(lambda s=socket: leer(s))
            socket.disconnected.connect(socket.deleteLater)

    servidor.newConnection.connect(conexion)
    servidor.listen(nombre)
    return servidor


def crear_aplicacion(argv: list[str]) -> QGuiApplication:
    QQuickStyle.setStyle("Basic")
    app = QGuiApplication(argv)
    app.setApplicationName(APP_ID)
    app.setApplicationDisplayName(NOMBRE)
    app.setOrganizationName("Vortexine's")
    app.setDesktopFileName(APP_ID)
    app.setWindowIcon(QIcon.fromTheme(APP_ID, QIcon(str(QML / "logo.svg"))))
    fuente = QFont("Adwaita Sans")
    fuente.setFamilies(["Adwaita Sans", "Noto Sans"])
    fuente.setPixelSize(14)
    app.setFont(fuente)
    return app


def cargar_interfaz(app: QGuiApplication) -> tuple[QQmlApplicationEngine, Puente]:
    motor_qml = QQmlApplicationEngine(app)
    puente = Puente(app)
    colores = Colores(parent=app)
    motor_qml.rootContext().setContextProperty("vp", puente)
    motor_qml.rootContext().setContextProperty("rep", puente.r)
    motor_qml.rootContext().setContextProperty("colores", colores)
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    return motor_qml, puente


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    if "--version" in argv:
        from . import VERSION
        print(NOMBRE, VERSION)
        return 0
    app = crear_aplicacion(argv)
    args = [a for a in argv[1:] if not a.startswith("-")]
    nombre = f"{APP_ID}-{os.getuid()}"
    if _avisar_a_la_abierta(nombre, args):
        return 0
    interfaz, puente = cargar_interfaz(app)
    if not interfaz.rootObjects():
        return 1
    servidor = _escuchar(nombre, puente, app)
    mpris = None
    try:
        from .mpris import Mpris
        if Mpris is not None:
            mpris = Mpris(puente.r, puente.portada_de, puente.mostrarVentana.emit)
    except Exception as e:  # sin D-Bus (por ejemplo, en una sesión rara): funciona igual sin MPRIS
        print("Vortex Player: sin MPRIS:", e, file=sys.stderr)
    recibir(puente, args)
    codigo = app.exec()
    servidor.close()
    puente.pararAvance()
    puente.r.cerrar()
    del mpris
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)


if __name__ == "__main__":
    sys.exit(main())
