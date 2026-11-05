"""La ventana: una sola por persona; si ya hay una abierta, se le pide que se muestre."""
from __future__ import annotations

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

QML = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent.parent)) / "vortex_connect" / "qml"


def _ya_abierta(nombre: str) -> bool:
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.disconnectFromServer()
    return True


def _escuchar(nombre: str, puente: Puente, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)
    servidor = QLocalServer(padre)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)
    servidor.newConnection.connect(puente.mostrarVentana.emit)
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
    fuente.setFamilies(["Adwaita Sans", "Segoe UI", "Noto Sans"])
    fuente.setPixelSize(14)
    app.setFont(fuente)
    return app


def cargar_interfaz(app: QGuiApplication, puente: Puente | None = None) -> tuple[QQmlApplicationEngine, Puente]:
    motor_qml = QQmlApplicationEngine(app)
    puente = puente or Puente(app)
    motor_qml.rootContext().setContextProperty("vc", puente)
    motor_qml.rootContext().setContextProperty("colores", Colores(parent=app))
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    return motor_qml, puente


def prueba_humo() -> int:
    """Para el .exe recién armado: la ventana carga y se cierra. 0 si cargó."""
    from PySide6.QtCore import QTimer
    app = crear_aplicacion(sys.argv[:1])
    interfaz, puente = cargar_interfaz(app)
    if not interfaz.rootObjects():
        return 1
    QTimer.singleShot(3000, app.quit)
    codigo = app.exec()
    puente.cerrar()
    sys.stdout and sys.stdout.flush()
    os._exit(codigo)


def main() -> int:
    app = crear_aplicacion(sys.argv[:1])
    nombre = f"{APP_ID}-{os.getlogin() if os.name == 'nt' else os.getuid()}"
    if _ya_abierta(nombre):
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
