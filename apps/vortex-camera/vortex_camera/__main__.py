"""Arranque de Vortexine's Camera (python -m vortex_camera).

Una sola ventana por persona: si ya hay una abierta, esta instancia le pide que se muestre por un socket
local y sale (dos ventanas abrirían dos veces cada cámara). La clase de ventana (app_id en Wayland) es
«vortex-camera», que es también el nombre de su lanzador (.desktop).
"""
from __future__ import annotations

import ctypes
import ctypes.util
import os
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
AV_LOG_ERROR = 16


def callar_ffmpeg() -> None:
    """Qt Multimedia usa el ffmpeg del sistema, que en nivel «info» escribe la URL de cada cámara (con
    su código de seguridad) en la terminal y en el registro. Se deja solo en errores."""
    nombre = ctypes.util.find_library("avutil")
    if not nombre:
        return
    try:
        ctypes.CDLL(nombre).av_log_set_level(AV_LOG_ERROR)
    except (OSError, AttributeError):
        pass


def _avisar_a_la_abierta(nombre: str) -> bool:
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.write(b"mostrar\n")
    socket.waitForBytesWritten(400)
    socket.disconnectFromServer()
    return True


def _escuchar(nombre: str, puente: Puente, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)
    servidor = QLocalServer(padre)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)

    def conexion() -> None:
        while (socket := servidor.nextPendingConnection()) is not None:
            socket.readyRead.connect(lambda s=socket: (s.readAll(), puente.mostrarVentana.emit()))
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
    callar_ffmpeg()
    return app


def cargar_interfaz(app: QGuiApplication, puente: Puente | None = None) -> tuple[QQmlApplicationEngine, Puente]:
    motor_qml = QQmlApplicationEngine(app)
    puente = puente or Puente(app)
    motor_qml.rootContext().setContextProperty("cam", puente)
    motor_qml.rootContext().setContextProperty("colores", Colores(parent=app))
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    callar_ffmpeg()          # por si el plugin de Qt Multimedia lo cambió al cargarse
    return motor_qml, puente


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    if "--version" in argv:
        from . import VERSION
        print(NOMBRE, VERSION)
        return 0
    app = crear_aplicacion(argv)
    nombre = f"{APP_ID}-{os.getuid()}"
    if _avisar_a_la_abierta(nombre):
        return 0
    interfaz, puente = cargar_interfaz(app)
    if not interfaz.rootObjects():
        return 1
    servidor = _escuchar(nombre, puente, app)
    codigo = app.exec()
    servidor.close()
    puente.cerrar()
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)


if __name__ == "__main__":
    sys.exit(main())
