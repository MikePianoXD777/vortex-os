#!/usr/bin/env python3
"""La app de Miku: abre la ventana (con avatar o el panel de solo voz) y se pone a escuchar.

La lanza el comando vortex-miku («vortex-miku» la abre y, otra vez, la cierra del todo). Con el 3D
apagado (miku.json: "avatar": false) no se importa QtWebEngine: queda solo el panel de voz.
"""

import fcntl
import os
import signal
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from miku import config, rutas  # noqa: E402

VERSION = "0.1.0"


def tomar_candado():
    """Una sola Miku por usuario: si otra ya tiene el candado, esta sale sin hacer nada."""
    f = open(rutas.runtime() / "vortex-miku.vivo", "w")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return None
    f.write(str(os.getpid()))
    f.flush()
    return f


def elegir_modo(cfg: dict) -> tuple[bool, str]:
    """(¿con avatar?, nota si hubo que bajar a solo voz). Sin QtWebEngine o sin las bibliotecas 3D: solo voz."""
    if not cfg["avatar"]:
        return False, ""
    if not (rutas.VENDOR / "three.module.min.js").is_file():
        return False, "Faltan las bibliotecas del 3D (three.js), así que sigo como asistente de voz. Reinstala el paquete vortex-miku."
    try:
        from PySide6 import QtWebEngineWidgets  # noqa: F401
    except ImportError:
        return False, "No pude cargar QtWebEngine, así que sigo como asistente de voz. Instala qt6-webengine."
    return True, ""


def crear(cfg: dict, avatar: bool, nota: str, modelo: Path | None):
    """Arma el asistente y su ventana. Devuelve (asistente, ventana, cerrar)."""
    from miku.asistente import Asistente
    from PySide6.QtWidgets import QApplication

    app = QApplication.instance()
    asistente = Asistente(cfg, avatar)
    cerrando = []

    def cerrar():
        if cerrando:
            return
        cerrando.append(True)
        asistente.cerrar_todo()
        app.quit()

    if avatar:
        from miku.ui_avatar import VentanaAvatar
        ventana = VentanaAvatar(asistente, modelo, cerrar)
    else:
        from miku.ui_panel import PanelVoz
        ventana = PanelVoz(asistente, cerrar)
    asistente.cerrar.connect(cerrar)
    if nota:
        asistente.aviso.emit(nota)
    return asistente, ventana, cerrar


def main() -> int:
    candado = tomar_candado()
    if candado is None:
        return 0
    cfg = config.cargar()
    avatar, nota = elegir_modo(cfg)
    if avatar:
        # QtWebEngine tiene que importarse antes de crear la QApplication.
        from PySide6 import QtWebEngineWidgets  # noqa: F401
    from PySide6.QtCore import QTimer
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtWidgets import QApplication

    # El app_id de Wayland sale de aquí: es lo que casan las reglas de Hyprland (class = "vortex-miku").
    QGuiApplication.setDesktopFileName("vortex-miku")
    app = QApplication(sys.argv[:1])
    app.setApplicationName("vortex-miku")
    app.setQuitOnLastWindowClosed(True)

    modelo = rutas.ruta_modelo(cfg) if avatar else None
    asistente, ventana, cerrar = crear(cfg, avatar, nota, modelo)

    # Señales: el launcher manda SIGTERM al cerrar. Python solo las atiende si el intérprete corre, de ahí el reloj.
    signal.signal(signal.SIGTERM, lambda *_: cerrar())
    signal.signal(signal.SIGINT, lambda *_: cerrar())
    reloj = QTimer()
    reloj.start(200)
    reloj.timeout.connect(lambda: None)

    ventana.show()
    asistente.iniciar()           # sin modelo 3D la página avisa sola (sin ?modelo=) con «vortex-miku modelo RUTA.vrm»
    codigo = app.exec()
    asistente.cerrar_todo()
    # Si el comando dejó su PID, solo se quita cuando es el nuestro.
    pidfile = rutas.runtime() / "vortex-miku.pid"
    try:
        if pidfile.read_text().strip() == str(os.getpid()):
            pidfile.unlink()
    except OSError:
        pass
    return codigo


if __name__ == "__main__":
    sys.exit(main())
