"""La ventana: una sola por persona; si ya hay una abierta, se le pasan los archivos por un socket local."""
from __future__ import annotations

import json
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


def _avisar_a_la_abierta(nombre: str, archivos: list[str]) -> bool:
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.write((json.dumps(archivos) + "\n").encode())
    socket.waitForBytesWritten(400)
    socket.disconnectFromServer()
    return True


def _escuchar(nombre: str, puente: Puente, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)
    servidor = QLocalServer(padre)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)

    def leer(socket: QLocalSocket) -> None:
        try:
            puente.agregarRutas(json.loads(bytes(socket.readAll()).decode()))
        except (ValueError, UnicodeDecodeError):
            pass
        puente.mostrarVentana.emit()

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


def cargar_interfaz(app: QGuiApplication, puente: Puente | None = None) -> tuple[QQmlApplicationEngine, Puente]:
    motor_qml = QQmlApplicationEngine(app)
    puente = puente or Puente(app)
    motor_qml.rootContext().setContextProperty("vs", puente)
    motor_qml.rootContext().setContextProperty("colores", Colores(parent=app))
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    return motor_qml, puente


def main(archivos: list[str]) -> int:
    rutas = [str(Path(a).expanduser().resolve()) for a in archivos]
    app = crear_aplicacion(sys.argv[:1])
    nombre = f"{APP_ID}-{os.getuid()}"
    if _avisar_a_la_abierta(nombre, rutas):
        return 0
    interfaz, puente = cargar_interfaz(app)
    if not interfaz.rootObjects():
        return 1
    puente.agregarRutas(rutas)
    servidor = _escuchar(nombre, puente, app)
    codigo = app.exec()
    servidor.close()
    puente.cerrar()
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)
