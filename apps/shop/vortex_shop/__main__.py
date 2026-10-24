"""Arranque de Vortexine's Shop (python -m vortex_shop).

Una sola ventana por usuario: si ya hay una tienda abierta, esta instancia le avisa por un socket
local para que se traiga al frente y sale. La clase de ventana (app_id en Wayland) es
«vortexines-shop», que es también el nombre del lanzador (.desktop).
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

from PySide6.QtCore import QObject, QUrl
from PySide6.QtGui import QFont, QGuiApplication, QIcon
from PySide6.QtNetwork import QLocalServer, QLocalSocket
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuickControls2 import QQuickStyle

from .colores import Colores
from .puente import Motor, Puente

APP_ID = "vortexines-shop"
TITULO = "Vortexine's Shop"
QML = Path(__file__).resolve().parent / "qml"


def _motor_real() -> Motor:
    """Los módulos del motor (catálogo de Flathub y operaciones de flatpak)."""
    from . import catalogo, errores, flatpak
    return Motor(catalogo=catalogo.Catalogo(), flatpak=flatpak, errores=errores)


def _avisar_a_la_abierta(nombre: str) -> bool:
    """True si ya había una tienda abierta (y se le pidió mostrarse)."""
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.write(b"mostrar\n")
    socket.waitForBytesWritten(400)
    socket.disconnectFromServer()
    return True


def _escuchar(nombre: str, puente: Puente, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)   # resto de una sesión anterior que no cerró bien
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
    """QGuiApplication con la identidad, la fuente y el estilo de Vortexine's."""
    QQuickStyle.setStyle("Basic")   # sin el aspecto nativo: todo se dibuja con los colores de la tienda
    app = QGuiApplication(argv)
    app.setApplicationName(APP_ID)
    app.setApplicationDisplayName(TITULO)
    app.setOrganizationName("Vortexine's")
    app.setDesktopFileName(APP_ID)   # en Wayland esto es el app_id de la ventana
    # Ícono del tema («vortexines-shop», lo instala el paquete); si no está, el logo de Vortexine's.
    app.setWindowIcon(QIcon.fromTheme(APP_ID, QIcon(str(QML / "vortex-os.svg"))))
    fuente = QFont("Adwaita Sans")
    fuente.setFamilies(["Adwaita Sans", "Noto Sans"])
    fuente.setPixelSize(14)
    app.setFont(fuente)
    return app


def cargar_interfaz(app: QGuiApplication, motor: Motor) -> tuple[QQmlApplicationEngine, Puente]:
    """Crea el puente y carga Main.qml (la ventana). Lo usan también las pruebas."""
    # Primero el motor QML y después el puente: al cerrar, Qt destruye en este orden y la interfaz
    # se va antes de que desaparezca `tienda` (si no, salen errores de «null» al salir).
    motor_qml = QQmlApplicationEngine(app)
    puente = Puente(motor, app)
    colores = Colores(parent=app)   # paleta del fondo de pantalla (matugen), con respaldo fijo
    motor_qml.rootContext().setContextProperty("tienda", puente)
    motor_qml.rootContext().setContextProperty("colores", colores)
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    return motor_qml, puente


def main(argv: list[str] | None = None, motor: Motor | None = None, instancia: str | None = None) -> int:
    argv = sys.argv if argv is None else argv
    app = crear_aplicacion(argv)
    nombre = instancia or f"{APP_ID}-{os.getuid()}"
    if _avisar_a_la_abierta(nombre):
        return 0
    if motor is None:
        try:
            motor = _motor_real()
        except ImportError as e:
            print(f"Vortexine's Shop: falta el motor de la tienda ({e}).", file=sys.stderr)
            return 1
    interfaz, puente = cargar_interfaz(app, motor)
    if not interfaz.rootObjects():
        return 1
    servidor = _escuchar(nombre, puente, app)   # vive mientras viva la aplicación
    puente.iniciar()
    codigo = app.exec()
    # Cerrada la ventana, la tienda sale ya, sin esperar a los hilos de fondo: si se cerró con algo en
    # la cola (y el usuario lo confirmó), Qt esperaría a todas las instalaciones; y un hilo del catálogo
    # o de los íconos colgado (sin red) dejaba el proceso vivo sin ventana, con el socket de «una sola
    # tienda» abierto, así que abrirla otra vez no hacía nada.
    servidor.close()
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)


if __name__ == "__main__":
    sys.exit(main())
