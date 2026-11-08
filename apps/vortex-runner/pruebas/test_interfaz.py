"""Pruebas de la ventana de Vortexine's Runner, sin pantalla (offscreen), con un umu-run y Protons falsos y
una casa temporal (VORTEX_RUNNER_HOME): no corre nada de Windows, no baja nada y no toca tu biblioteca.

Cubre: la QML carga sin errores ni advertencias (con la biblioteca vacía, con programas y sin el motor),
agregar, buscar, «Configurar» (cambiar nombre, Proton y argumentos), jugar y ver que está abierto,
detener, la pregunta de un .exe abierto desde Thunar, la de un instalador que terminó y quitar.

    cd apps/vortex-runner && QT_QPA_PLATFORM=offscreen python3 -m unittest discover -s pruebas -p 'test_interfaz.py'

Con VORTEX_RUNNER_CAPTURAS=/carpeta guarda capturas (QQuickWindow.grabWindow).
"""
from __future__ import annotations

import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

UMU_FALSO = "#!/bin/sh\nexec sleep \"${UMU_DURA:-0}\"\n"

INICIO = textwrap.dedent("""
    import os, sys, json, time
    sys.path.insert(0, %r)
    from PySide6.QtCore import QTimer, QObject, QUrl
    from PySide6.QtGui import QGuiApplication
    from PySide6.QtQml import QQmlApplicationEngine
    from PySide6.QtQuickControls2 import QQuickStyle
    from vortex_runner.colores import Colores
    from vortex_runner.puente import Runner
    from vortex_runner.__main__ import QML
    QQuickStyle.setStyle("Basic")
    app = QGuiApplication(["vortex-runner"])
    motor = QQmlApplicationEngine(app)
    runner = Runner(app)
    motor.rootContext().setContextProperty("runner", runner)
    motor.rootContext().setContextProperty("colores", Colores(parent=app))
    motor.load(QUrl.fromLocalFile(str(QML / "Main.qml")))
    assert motor.rootObjects(), "la ventana no cargó"
    ventana = motor.rootObjects()[0]
    avisos = []
    runner.aviso.connect(lambda t, k: avisos.append([t, k]))
    capturas = os.environ.get("VORTEX_RUNNER_CAPTURAS", "")
    def capturar(nombre):
        if capturas:
            ventana.grabWindow().save(os.path.join(capturas, nombre + ".png"))
    def hijo(nombre):
        return ventana.findChild(QObject, nombre)
    def abierto(nombre):
        return bool(hijo(nombre).property("visible"))
    tiempo = [0]
    def paso(ms, f):
        tiempo[0] += ms
        QTimer.singleShot(tiempo[0], f)
    datos = {}
    def fin():
        datos["avisos"] = avisos
        print("RESULTADO " + json.dumps(datos, ensure_ascii=False), flush=True)
        runner.cerrar()
        sys.stdout.flush(); sys.stderr.flush()
        os._exit(0)
""")


class Interfaz(unittest.TestCase):
    def setUp(self) -> None:
        self.t = Path(tempfile.mkdtemp(prefix="vortex-runner-qml-"))
        self.addCleanup(shutil.rmtree, self.t, True)
        self.umu = self.t / "umu-run"
        self.umu.write_text(UMU_FALSO)
        self.umu.chmod(self.umu.stat().st_mode | stat.S_IEXEC)
        self.protones = self.t / "protones"
        for nombre in ("proton-cachyos-slr", "GE-Proton10-3"):
            (self.protones / nombre).mkdir(parents=True)
            (self.protones / nombre / "proton").write_text("#!/bin/sh\n")
        self.juegos = self.t / "Mis juegos"
        self.juegos.mkdir()
        for n in ("super_juego-x64.exe", "Otro Programa.exe", "setup_cosa.exe"):
            (self.juegos / n).write_bytes(b"MZ" + b"\0" * 200)

    def correr(self, cuerpo: str, motor: bool = True) -> dict:
        guion = self.t / "guion.py"
        guion.write_text(INICIO % str(RAIZ) + textwrap.dedent(cuerpo) + "\napp.exec()\n", encoding="utf-8")
        entorno = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "DISPLAY")}
        # QT_FORCE_STDERR_LOGGING: sin terminal, Qt manda las advertencias al journal y no se verían aquí.
        entorno.update({"QT_QPA_PLATFORM": "offscreen", "QT_FORCE_STDERR_LOGGING": "1",
                        "VORTEX_RUNNER_HOME": str(self.t / "casa"), "VORTEX_RUNNER_COLORES": "/no/existe",
                        "VORTEX_RUNNER_UMU": str(self.umu) if motor else str(self.t / "no-hay-umu"),
                        "VORTEX_RUNNER_PROTONES": str(self.protones), "UMU_DURA": "30",
                        "XDG_RUNTIME_DIR": str(self.t), "XDG_CONFIG_HOME": str(self.t / "config"),
                        "JUEGOS": str(self.juegos)})
        r = subprocess.run([sys.executable, str(guion)], env=entorno, capture_output=True, text=True, timeout=90)
        linea = next((x for x in r.stdout.splitlines() if x.startswith("RESULTADO ")), None)
        self.assertIsNotNone(linea, r.stdout[-3000:] + r.stderr[-4000:])
        quejas = [x for x in r.stderr.splitlines() if x.strip()]
        self.assertEqual(quejas, [], "La QML o Qt se quejaron")
        return json.loads(linea[len("RESULTADO "):])

    def test_biblioteca(self) -> None:
        d = self.correr("""
            J = os.environ["JUEGOS"]
            def vacia():
                datos["vacia"] = len(runner.programas)
                datos["falta"] = list(runner.faltaMotor)
                capturar("vacia")
                datos["id1"] = runner.agregar(QUrl.fromLocalFile(J + "/super_juego-x64.exe").toString())
                datos["id2"] = runner.agregar(J + "/Otro Programa.exe")
                datos["repetido"] = runner.agregar(J + "/Otro Programa.exe")
            def llena():
                datos["nombres"] = sorted(p["nombre"] for p in runner.programas)
                datos["celdas"] = hijo("rejilla").property("count")
                hijo("busqueda").setProperty("text", "otro")
            def buscada():
                datos["buscada"] = hijo("rejilla").property("count")
                capturar("buscada")
                hijo("busqueda").setProperty("text", "")
            def configurar():
                from PySide6.QtCore import QMetaObject, Q_ARG
                QMetaObject.invokeMethod(ventana, "configurar", Q_ARG("QVariant", 0))
                datos["config_abierto"] = abierto("dialogoPrograma")
                capturar("configurar")
                p = runner.programas[0]
                runner.guardar(p["id"], "Mi Juego", "GE-Proton10-3", "-windowed")
                hijo("dialogoPrograma").setProperty("visible", False)
            def guardado():
                p = next(p for p in runner.programas if p["nombre"] == "Mi Juego")
                datos["guardado"] = [p["proton"], p["argumentos"]]
                runner.jugar(p["id"])
                datos["jugado"] = p["id"]
            def jugando():
                runner._revisar()
                p = next(p for p in runner.programas if p["id"] == datos["jugado"])
                datos["abierto"] = p["abierto"]
                capturar("jugando")
                runner.detener(p["id"])
            def detenido():
                runner._revisar()
                p = next(p for p in runner.programas if p["id"] == datos["jugado"])
                datos["detenido"] = p["abierto"]
                from PySide6.QtCore import QMetaObject, Q_ARG
                QMetaObject.invokeMethod(ventana, "pedirQuitar", Q_ARG("QVariant", 0))
                datos["quitar_abierto"] = abierto("dialogoQuitar")
                capturar("quitar")
                hijo("dialogoQuitar").setProperty("visible", False)
                runner.quitar(p["id"], True)
            def quitado():
                datos["quedan"] = [p["nombre"] for p in runner.programas]
                fin()
            paso(300, vacia); paso(300, llena); paso(200, buscada); paso(300, configurar)
            paso(300, guardado); paso(1500, jugando); paso(2500, detenido); paso(400, quitado)
        """)
        self.assertEqual(d["vacia"], 0)
        self.assertEqual(d["falta"], [])
        self.assertTrue(d["id1"] and d["id2"])
        self.assertEqual(d["repetido"], d["id2"])
        self.assertEqual(len(d["nombres"]), 2)
        self.assertEqual(d["celdas"], 2)
        self.assertEqual(d["buscada"], 1)
        self.assertTrue(d["config_abierto"])
        self.assertEqual(d["guardado"], ["GE-Proton10-3", "-windowed"])
        self.assertTrue(d["abierto"])
        self.assertFalse(d["detenido"])
        self.assertTrue(d["quitar_abierto"])
        self.assertEqual(len(d["quedan"]), 1)

    def test_preguntas(self) -> None:
        d = self.correr("""
            J = os.environ["JUEGOS"]
            def archivo():
                runner.abrirArchivo(J + "/setup_cosa.exe")
            def preguntado():
                dlg = hijo("dialogoArchivo")
                datos["archivo"] = [abierto("dialogoArchivo"), dlg.property("instalador"), dlg.property("nombre")]
                capturar("pregunta-archivo")
                dlg.setProperty("visible", False)
                runner.elegirInstalado.emit([{"exe": J + "/super_juego-x64.exe", "nombre": "Super Juego"},
                                             {"exe": J + "/Otro Programa.exe", "nombre": "Otro"}])
            def instalado():
                datos["instalado"] = abierto("dialogoInstalado")
                capturar("pregunta-instalado")
                runner.aviso.emit("Listo: lo agregué a tu biblioteca.", "exito")
            def avisado():
                datos["aviso"] = hijo("aviso").property("texto")
                fin()
            paso(300, archivo); paso(300, preguntado); paso(300, instalado); paso(300, avisado)
        """)
        self.assertEqual(d["archivo"][0:2], [True, True])
        self.assertTrue(d["instalado"])
        self.assertEqual(d["aviso"], "Listo: lo agregué a tu biblioteca.")

    def test_sin_motor(self) -> None:
        d = self.correr("""
            def ver():
                datos["falta"] = list(runner.faltaMotor)
                capturar("sin-motor")
                # Se puede agregar a la biblioteca sin el motor; lo que no se puede es jugar.
                datos["agregar"] = runner.agregar(os.environ["JUEGOS"] + "/super_juego-x64.exe")
                runner.jugar(datos["agregar"])
                fin()
            paso(300, ver)
        """, motor=False)
        self.assertEqual(d["falta"], ["umu-launcher"])
        self.assertTrue(d["agregar"])
        self.assertTrue(any(k == "error" for _, k in d["avisos"]))


if __name__ == "__main__":
    unittest.main()
