#!/usr/bin/env python3
"""Prueba sin pantalla del panel de solo voz (sin avatar): tamaño, indicador de escucha, respuesta,
cuadro de texto, micrófono y Esc. Además comprueba que NO se cargó QtWebEngine (el modo sin 3D no lo usa).

    QT_QPA_PLATFORM=offscreen python3 -I pruebas/panel.py CARPETA_SALIDA
"""

import os
import sys
import time
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from PySide6.QtCore import Qt  # noqa: E402
from PySide6.QtTest import QTest  # noqa: E402
from PySide6.QtGui import QGuiApplication  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402,F401

from PySide6.QtWidgets import QPushButton  # noqa: E402

from miku import app as miku_app  # noqa: E402
from miku import colores  # noqa: E402
from miku.ui_panel import PanelVoz  # noqa: E402
from pruebas.falso import AsistenteFalso  # noqa: E402


def main() -> int:
    salida = Path(sys.argv[1])
    salida.mkdir(parents=True, exist_ok=True)
    fallos = []

    def comprobar(nombre, cond, extra=""):
        print(("  ok  " if cond else "  MAL ") + nombre + (f"  {extra}" if extra else ""), flush=True)
        if not cond:
            fallos.append(nombre)

    app = miku_app.crear_aplicacion()
    comprobar("el app_id de Wayland es «vortex-miku» (lo que casan las reglas de Hyprland)", QGuiApplication.desktopFileName() == "vortex-miku", QGuiApplication.desktopFileName())
    asistente = AsistenteFalso()
    cerrado = []
    panel = PanelVoz(asistente, lambda: cerrado.append(1))
    panel.show()
    app.processEvents()

    comprobar("los colores salen de la paleta del fondo (o del respaldo de Vortexine's si no hay)",
              panel.P == colores.leer(), f"fondo {panel.P['fondo']}")
    comprobar("sin QtWebEngine cargado (no hay 3D)", not any(m.startswith("PySide6.QtWebEngine") for m in sys.modules))
    comprobar("tamaño ~420x240", (panel.width(), panel.height()) == (420, 240))
    comprobar("título «Asistente de Vortex»", panel.windowTitle() == "Asistente de Vortex")

    # Primera vez: la descarga de los modelos se ve con su barra.
    asistente.progreso.emit("Preparando la voz de Miku (una sola vez, ~560 MB)…", "Bajando el oído 90 MB de 590 MB", 0.15)
    app.processEvents()
    comprobar("muestra el progreso de la descarga con el texto pedido",
              not panel.prep.isHidden() and "~560 MB" in panel.prep_titulo.text() and abs(panel.prep_barra.fraccion - 0.15) < 1e-6)
    panel.grab().save(str(salida / "panel-preparando.png"))
    asistente.progreso.emit("", "", -1.0)
    comprobar("al terminar la descarga se esconde", panel.prep.isHidden())

    # Claude apagado: el globo dice qué hacer y trae su botón.
    asistente.respuesta.emit("Para hablar conmigo necesito tu cuenta de Claude.", True)
    asistente.acciones.emit([["activar", "Activar Claude"]])
    app.processEvents()
    botones = panel.botones.findChildren(QPushButton)
    comprobar("botón «Activar Claude» visible", len(botones) == 1 and botones[0].text() == "Activar Claude" and not panel.botones.isHidden())
    panel.grab().save(str(salida / "panel-activar-claude.png"))
    botones[0].click()
    comprobar("el botón pide la acción «activar»", ("accion", "activar") in asistente.registro)
    asistente.acciones.emit([["sesion", "Iniciar sesión"]])
    app.processEvents()
    botones = panel.botones.findChildren(QPushButton)
    comprobar("botón «Iniciar sesión»", len(botones) == 1 and botones[0].text() == "Iniciar sesión")
    asistente.acciones.emit([])
    app.processEvents()
    comprobar("sin botones se esconde", panel.botones.isHidden())

    asistente.estado.emit("escuchando", "Escuchando...")
    asistente.nivel.emit(0.7)
    app.processEvents()
    comprobar("indicador de escucha visible", panel.estado.text() == "Escuchando..." and panel.mic.activo and panel.punto.reloj.isActive())
    panel.grab().save(str(salida / "panel-escuchando.png"))

    asistente.usuario.emit("¿Qué hora es en Tokio?")
    asistente.estado.emit("pensando", "Pensando...")
    asistente.respuesta.emit("", False)
    app.processEvents()
    panel.grab().save(str(salida / "panel-pensando.png"))
    asistente.estado.emit("hablando", "")
    asistente.respuesta.emit("En Tokio son las nueve y cuarto de la mañana. Si quieres, te ayudo a poner un reloj con esa zona horaria en el escritorio.", True)
    app.processEvents()
    comprobar("muestra la respuesta en texto", "Tokio" in panel.respuesta.text() and "Tú:" in panel.tu.text())
    panel.grab().save(str(salida / "panel-respuesta.png"))

    asistente.aviso.emit("No tengo internet, así que no puedo pensar ahora mismo.")
    app.processEvents()
    comprobar("muestra los avisos en español", "internet" in panel.respuesta.text())
    panel.grab().save(str(salida / "panel-aviso.png"))

    panel.entrada.setText("abre la terminal")
    QTest.keyClick(panel.entrada, Qt.Key.Key_Return)
    comprobar("escribir y Enter manda el texto", ("enviar", "abre la terminal") in asistente.registro and panel.entrada.text() == "")
    comprobar("escribir cuenta como tocarlo (no se cierra solo)", ("tocada",) in asistente.registro)

    QTest.mouseClick(panel.mic, Qt.MouseButton.LeftButton)
    comprobar("clic en el micrófono pide escuchar", ("escuchar",) in asistente.registro)
    asistente.estado_actual = "escuchando"
    QTest.mouseClick(panel.mic, Qt.MouseButton.LeftButton)
    comprobar("otro clic corta la escucha", ("cortar",) in asistente.registro)

    QTest.keyClick(panel, Qt.Key.Key_Escape)
    comprobar("Esc cierra todo", bool(cerrado))
    panel.deleteLater()

    # Escala de la interfaz (general.uiScale de Ajustes) y paleta de respaldo.
    chico = PanelVoz(AsistenteFalso(), lambda: None, escala=0.85, paleta=dict(colores.RESPALDO))
    chico.show()
    app.processEvents()
    comprobar("con uiScale 0.85 el panel mide 357x204", (chico.width(), chico.height()) == (357, 204), f"{chico.width()}x{chico.height()}")
    chico.grab().save(str(salida / "panel-escala-085.png"))
    comprobar("paleta de respaldo de Vortexine's", chico.P["fondo"] == "#0b0d1a")
    chico.deleteLater()
    print("FALLOS:", len(fallos), fallos if fallos else "")
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
