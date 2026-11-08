"""Arranque de Vortexine's Runner (python -m vortex_runner).

    vortex-runner                 abre la biblioteca
    vortex-runner ARCHIVO.exe     lo abre si ya está en tu biblioteca; si no, pregunta (doble clic en Thunar)
    vortex-runner --correr ID     abre ese programa sin ventana de Runner (los accesos del lanzador)
    vortex-runner --version

Una sola ventana por persona: si ya hay una abierta, esta le pasa el archivo (o le pide que se muestre)
por un socket local y sale. La clase de ventana es «vortex-runner», como su .desktop.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

QML = Path(__file__).resolve().parent / "qml"


def _correr(pid: str) -> int:
    from . import biblioteca, correr
    e = biblioteca.por_id(pid)
    if e is None:
        print(f"vortex-runner: no hay ningún programa «{pid}» en tu biblioteca", file=sys.stderr)
        return 1
    if correr.corriendo(e):
        return 0
    try:
        return correr.correr_y_esperar(e)
    except FileNotFoundError as err:
        print(f"vortex-runner: {err} Ábrelo desde Vortexine's Runner para instalarlo.", file=sys.stderr)
        return 1


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv if argv is None else argv
    from . import APP_ID, NOMBRE, VERSION
    if "--version" in argv:
        print(NOMBRE, VERSION)
        return 0
    if "--correr" in argv:
        i = argv.index("--correr")
        return _correr(argv[i + 1]) if i + 1 < len(argv) else 2

    from PySide6.QtCore import QUrl
    from PySide6.QtGui import QFont, QGuiApplication, QIcon
    from PySide6.QtNetwork import QLocalServer, QLocalSocket
    from PySide6.QtQml import QQmlApplicationEngine
    from PySide6.QtQuickControls2 import QQuickStyle

    from .colores import Colores
    from .puente import Runner

    archivos = [a for a in argv[1:] if not a.startswith("-")]
    nombre = f"{APP_ID}-{os.getuid()}"

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

    # ¿Ya hay una abierta? Se le pasa el archivo (o «mostrar») y listo.
    socket = QLocalSocket()
    socket.connectToServer(nombre)
    if socket.waitForConnected(400):
        mensaje = ("abrir\t" + str(Path(archivos[0]).resolve())) if archivos else "mostrar"
        socket.write(mensaje.encode() + b"\n")
        socket.waitForBytesWritten(400)
        socket.disconnectFromServer()
        return 0

    motor = QQmlApplicationEngine(app)
    runner = Runner(app)
    motor.rootContext().setContextProperty("runner", runner)
    motor.rootContext().setContextProperty("colores", Colores(parent=app))
    motor.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    if not motor.rootObjects():
        return 1

    QLocalServer.removeServer(nombre)
    servidor = QLocalServer(app)
    servidor.setSocketOptions(QLocalServer.SocketOption.UserAccessOption)

    def conexion() -> None:
        while (s := servidor.nextPendingConnection()) is not None:
            def leer(s=s):
                for linea in bytes(s.readAll()).decode("utf-8", "replace").splitlines():
                    runner.mostrarVentana.emit()
                    if linea.startswith("abrir\t"):
                        runner.abrirArchivo(linea[6:])
            s.readyRead.connect(leer)
            s.disconnected.connect(s.deleteLater)

    servidor.newConnection.connect(conexion)
    servidor.listen(nombre)
    if archivos:
        runner.abrirArchivo(archivos[0])
    codigo = app.exec()
    servidor.close()
    runner.cerrar()
    sys.stdout.flush()
    sys.stderr.flush()
    os._exit(codigo)


if __name__ == "__main__":
    sys.exit(main())
