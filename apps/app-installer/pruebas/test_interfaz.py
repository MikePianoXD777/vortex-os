"""La ventana carga sin errores ni avisos de QML, en los dos idiomas, y responde a un archivo."""
from __future__ import annotations

import os
import time

import pytest

import muestras

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("QT_QUICK_BACKEND", "software")
os.environ["VORTEX_APP_INSTALLER_COLORES"] = "/nonexistent/qs_colors.json"

from PySide6.QtCore import QEventLoop, QTimer, QtMsgType, qInstallMessageHandler  # noqa: E402

AVISOS: list[str] = []


def _mensajes(tipo, _contexto, texto):
    if tipo in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg) and "propagateSizeHints" not in texto:
        AVISOS.append(texto)


def _esperar(ms):
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


@pytest.fixture(scope="module")
def app():
    from vortex_app_installer.__main__ import crear_aplicacion
    qInstallMessageHandler(_mensajes)
    return crear_aplicacion(["prueba"])


@pytest.mark.parametrize("idioma", ["es", "en"])
def test_ventana_sin_avisos(casa, app, idioma, monkeypatch):
    monkeypatch.setenv("VORTEX_APP_INSTALLER_IDIOMA", idioma)
    from vortex_app_installer.__main__ import cargar_interfaz
    AVISOS.clear()
    interfaz, instalador = cargar_interfaz(app)
    assert interfaz.rootObjects()
    ventana = interfaz.rootObjects()[0]
    archivos = [muestras.zip_(casa.descargas / "a.zip", {"a/b.txt": b"b"}),
                muestras.appimage_falso(casa.descargas / "Falso.AppImage"),
                muestras.mod_fabric(casa.descargas / "sodium.jar")]
    sh = casa.descargas / "x.sh"
    sh.write_text("#!/bin/sh\necho hola\n")
    archivos.append(sh)
    for archivo in archivos:
        instalador.abrir(str(archivo))
        fin = time.time() + 20
        while instalador.property("estado") != "listo" and time.time() < fin:
            _esperar(50)
        assert instalador.property("estado") == "listo", archivo
        assert instalador.property("archivo")["acciones"]
        _esperar(150)
    instalador.abrir(str(archivos[0]))
    while instalador.property("estado") != "listo":
        _esperar(50)
    instalador.ejecutar("descomprimir", {})
    fin = time.time() + 20
    while instalador.property("estado") != "hecho" and time.time() < fin:
        _esperar(50)
    assert instalador.property("estado") == "hecho"
    assert len(instalador.property("instalados")) == 1
    ventana.setProperty("pestana", "instalado")
    _esperar(200)
    assert AVISOS == []
    interfaz.deleteLater()
