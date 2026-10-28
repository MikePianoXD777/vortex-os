"""Arranque de Vortex App Installer (python -m vortex_app_installer [ARCHIVO]).

Una sola ventana por usuario: si ya hay una abierta, esta instancia le pasa el archivo por un socket
local (para que lo abra y se traiga al frente) y sale. La clase de ventana (app_id en Wayland) es
«vortex-app-installer», que es también el nombre de su lanzador (.desktop).
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
from .puente import Instalador, ruta_de

QML = Path(__file__).resolve().parent / "qml"


def _archivo_de(argv: list[str]) -> str:
    """El primer argumento que no es una opción (lo que pasa el explorador al dar doble clic: %f o %u)."""
    for a in argv[1:]:
        if not a.startswith("-") or a.startswith("file:"):
            return ruta_de(a)
    return ""


def _avisar_a_la_abierta(nombre: str, archivo: str) -> bool:
    """True si ya había una ventana abierta (y se le pasó el archivo)."""
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if not socket.waitForConnected(400):
        return False
    socket.write(("abrir\t%s\n" % archivo).encode("utf-8", "surrogateescape"))
    socket.waitForBytesWritten(400)
    socket.disconnectFromServer()
    return True


def _enfocar_en_hyprland() -> None:
    """Hyprland ignora requestActivate() (misc:focus_on_activate apagado): se le pide directo.
    Primero la sintaxis de la config en Lua (Hyprland 0.56+); si falla, la de antes."""
    hyprctl = shutil.which("hyprctl")
    if not hyprctl or not os.environ.get("HYPRLAND_INSTANCE_SIGNATURE"):
        return
    ventana = f"pid:{os.getpid()}"
    for orden in (f'hl.dsp.focus({{ window = "{ventana}" }})', f"focuswindow {ventana}"):
        try:
            hecho = subprocess.run([hyprctl, "dispatch", orden], capture_output=True, timeout=3)
        except (OSError, subprocess.TimeoutExpired):
            return
        if hecho.returncode == 0:
            return


def _escuchar(nombre: str, instalador: Instalador, padre: QObject) -> QLocalServer:
    QLocalServer.removeServer(nombre)   # resto de una sesión anterior que no cerró bien
    servidor = QLocalServer(padre)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)

    def recibir(socket: QLocalSocket) -> None:
        for linea in bytes(socket.readAll()).decode("utf-8", "surrogateescape").splitlines():
            orden, _, archivo = linea.partition("\t")
            if orden == "abrir":
                if archivo:
                    instalador.abrir(archivo)
                instalador.mostrarVentana.emit()
                _enfocar_en_hyprland()

    def conexion() -> None:
        while (socket := servidor.nextPendingConnection()) is not None:
            socket.readyRead.connect(lambda s=socket: recibir(s))
            socket.disconnected.connect(socket.deleteLater)

    servidor.newConnection.connect(conexion)
    servidor.listen(nombre)
    return servidor


def crear_aplicacion(argv: list[str]) -> QGuiApplication:
    """QGuiApplication con la identidad, la fuente y el estilo de Vortexine's."""
    QQuickStyle.setStyle("Basic")   # sin el aspecto nativo: todo se dibuja con los colores de Vortexine's
    app = QGuiApplication(argv)
    app.setApplicationName(APP_ID)
    app.setApplicationDisplayName(NOMBRE)
    app.setOrganizationName("Vortexine's")
    app.setDesktopFileName(APP_ID)   # en Wayland esto es el app_id de la ventana
    app.setWindowIcon(QIcon.fromTheme(APP_ID, QIcon(str(QML / "logo.svg"))))
    fuente = QFont("Adwaita Sans")
    fuente.setFamilies(["Adwaita Sans", "Noto Sans"])
    fuente.setPixelSize(14)
    app.setFont(fuente)
    return app


def cargar_interfaz(app: QGuiApplication) -> tuple[QQmlApplicationEngine, Instalador]:
    """Crea el puente y carga Main.qml (la ventana). Lo usan también las pruebas."""
    motor_qml = QQmlApplicationEngine(app)   # primero el motor QML: al cerrar se va antes que el puente
    instalador = Instalador(app)
    colores = Colores(parent=app)
    motor_qml.rootContext().setContextProperty("instalador", instalador)
    motor_qml.rootContext().setContextProperty("colores", colores)
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    return motor_qml, instalador


def main(argv: list[str] | None = None, instancia: str | None = None) -> int:
    argv = sys.argv if argv is None else argv
    if "--version" in argv:
        from . import VERSION
        print(NOMBRE, VERSION)
        return 0
    app = crear_aplicacion(argv)
    archivo = _archivo_de(argv)
    nombre = instancia or f"{APP_ID}-{os.getuid()}"
    if _avisar_a_la_abierta(nombre, archivo):
        return 0
    interfaz, instalador = cargar_interfaz(app)
    if not interfaz.rootObjects():
        return 1
    servidor = _escuchar(nombre, instalador, app)
    if archivo:
        instalador.abrir(archivo)
    codigo = app.exec()
    # Cerrada la ventana se sale ya, sin esperar a los hilos de fondo (una búsqueda en Flathub sin red).
    servidor.close()
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)


if __name__ == "__main__":
    sys.exit(main())
