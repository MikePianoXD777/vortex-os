"""Arranque de Vortexine Task Manager (python -m vortex_task_manager).

Una sola ventana por persona: si ya hay una abierta, esta instancia le pide que se muestre por un socket
local y sale. La clase de ventana (app_id en Wayland) es «vortex-task-manager», que es también el nombre
de su lanzador (.desktop) y lo que busca Ctrl+Shift+Esc (config/hypr/atajos.lua).
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

from PySide6.QtCore import QObject, QSize, Qt, QUrl
from PySide6.QtGui import QFont, QGuiApplication, QIcon, QPixmap
from PySide6.QtNetwork import QLocalServer, QLocalSocket
from PySide6.QtQml import QQmlApplicationEngine
from PySide6.QtQuick import QQuickImageProvider
from PySide6.QtQuickControls2 import QQuickStyle

from . import APP_ID, NOMBRE
from .colores import Colores
from .puente import Puente

QML = Path(__file__).resolve().parent / "qml"


class ProveedorIconos(QQuickImageProvider):
    """image://icono/<nombre o ruta>: el ícono de la app, del tema de íconos que uses."""

    def __init__(self):
        super().__init__(QQuickImageProvider.ImageType.Pixmap)

    def requestPixmap(self, ident: str, size: QSize, requested: QSize) -> QPixmap:
        lado = max(requested.width(), requested.height(), 32) if requested.isValid() else 64
        icono = QIcon(ident) if ident.startswith("/") else QIcon.fromTheme(ident)
        if icono.isNull():
            icono = QIcon.fromTheme(ident.lower())
        pixmap = icono.pixmap(QSize(lado, lado))
        if pixmap.isNull():
            pixmap = QPixmap(lado, lado)
            pixmap.fill(Qt.GlobalColor.transparent)
        return pixmap


def _tema_iconos() -> None:
    """Fuera de KDE, Qt no sabe qué tema de íconos usas ni dónde están: se toma el de GTK (gsettings) o
    Papirus, y se buscan en las carpetas de XDG y en las de Flatpak (ahí están los de tus apps Flatpak)."""
    datos = [os.environ.get("XDG_DATA_HOME") or str(Path.home() / ".local/share")]
    datos += (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":")
    datos += [str(Path.home() / ".local/share/flatpak/exports/share"), "/var/lib/flatpak/exports/share", "/usr/share"]
    rutas = list(QIcon.themeSearchPaths())
    for d in [str(Path.home() / ".icons")] + [str(Path(d) / "icons") for d in datos if d]:
        if d not in rutas and os.path.isdir(d):
            rutas.append(d)
    QIcon.setThemeSearchPaths(rutas)
    if QIcon.themeName() and QIcon.themeName() != "hicolor":
        return
    tema = ""
    if shutil.which("gsettings"):
        try:
            tema = subprocess.run(["gsettings", "get", "org.gnome.desktop.interface", "icon-theme"],
                                  capture_output=True, text=True, timeout=2).stdout.strip().strip("'")
        except (OSError, subprocess.TimeoutExpired):
            tema = ""
    bases = [Path.home() / ".local/share/icons", Path("/usr/share/icons")]
    for candidato in (tema, "Papirus-Dark", "Papirus", "breeze-dark", "Adwaita"):
        if candidato and any((b / candidato).is_dir() for b in bases):
            QIcon.setThemeName(candidato)
            break
    QIcon.setFallbackThemeName("hicolor")
    QIcon.setFallbackSearchPaths([*QIcon.fallbackSearchPaths(), "/usr/share/pixmaps"])


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
    _tema_iconos()
    return app


def cargar_interfaz(app: QGuiApplication, puente: Puente | None = None) -> tuple[QQmlApplicationEngine, Puente]:
    motor_qml = QQmlApplicationEngine(app)
    puente = puente or Puente(app)
    motor_qml.addImageProvider("icono", ProveedorIconos())
    motor_qml.rootContext().setContextProperty("tm", puente)
    motor_qml.rootContext().setContextProperty("colores", Colores(parent=app))
    motor_qml.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
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
