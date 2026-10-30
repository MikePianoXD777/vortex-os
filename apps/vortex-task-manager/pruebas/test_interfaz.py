"""Pruebas de la ventana de Vortexine Task Manager, sin pantalla (offscreen): carga sin errores de QML,
la lista se llena, se abre una app, se busca y se cambia de página. No cierra ni toca nada tuyo: lo único
que finaliza es un «sleep» con nombre único que lanza ella misma.

    python3 -m unittest discover -s apps/vortex-task-manager/pruebas
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import textwrap
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

GUION = textwrap.dedent("""
    import json, os, shutil, subprocess, sys, tempfile
    sys.path.insert(0, %r)
    from PySide6.QtCore import QTimer
    from vortex_task_manager.__main__ import crear_aplicacion, cargar_interfaz
    app = crear_aplicacion(["tm"])
    motor, puente = cargar_interfaz(app)
    errores = []
    motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
    v = motor.rootObjects()[0]
    # Un «sleep» con nombre único: es lo único que la prueba finaliza (por su PID).
    unico = os.path.join(tempfile.mkdtemp(), "vtmprueba%%d" %% os.getpid())
    shutil.copy("/usr/bin/sleep", unico)
    hijo = subprocess.Popen([unico, "60"])
    nombre = os.path.basename(unico)
    datos = {}
    def medir():
        filas = puente.modelo.filas()
        datos["secciones"] = [f["nombre"] for f in filas if f["tipo"] == "seccion"]
        datos["grupos"] = sum(f["tipo"] == "grupo" for f in filas)
        puente.filtro = nombre
    def filtrado():
        filas = puente.modelo.filas()
        grupos = [f for f in filas if f["tipo"] == "grupo"]
        # Fuera de Hyprland sale solo; aquí puede ir dentro de la ventana desde la que se corre la prueba.
        datos["filtrado"] = all(hijo.pid in puente._por_clave[g["clave"]]["pids"] for g in grupos) and len(grupos) == 1
        clave = grupos[0]["clave"] + "/" + str(hijo.pid)
        datos["detalles"] = puente.detalles(clave)["pid"] == hijo.pid
        puente.finalizar(clave, False)
    def finalizado():
        datos["finalizado"] = hijo.poll() == -15
        puente.filtro = ""
        puente.orden = "mem"
        g = next(f for f in puente.modelo.filas() if f["tipo"] == "grupo" and f["n"] > 1)
        puente.alternarGrupo(g["clave"])
        datos["abierto"] = any(f["tipo"] == "proceso" and f["padre"] == g["clave"] for f in puente.modelo.filas())
        v.setProperty("pagina", "rendimiento")
    def rendimiento():
        datos["historia"] = len(puente.historia["cpu"])
        datos["verProcesos"] = puente.verProcesos
        v.setProperty("pagina", "servicios")
    def fin():
        datos["servicios"] = len(puente.serviciosUsuario) + len(puente.serviciosSistema)
        datos["errores"] = errores
        print("RESULTADO " + json.dumps(datos), flush=True)
        hijo.kill()
        puente.cerrar()
        app.quit()
    t = 0
    for ms, f in ((2500, medir), (300, filtrado), (800, finalizado), (2500, rendimiento), (3000, fin)):
        t += ms
        QTimer.singleShot(t, f)
    app.exec()
""") % str(RAIZ)


class Interfaz(unittest.TestCase):
    def test_ventana(self):
        entorno = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "VORTEX_TASK_MANAGER_COLORES": "/no/existe"}
        r = subprocess.run([sys.executable, "-c", GUION], capture_output=True, text=True, timeout=60, env=entorno)
        linea = next((l for l in r.stdout.splitlines() if l.startswith("RESULTADO ")), None)
        self.assertIsNotNone(linea, r.stdout + r.stderr)
        d = json.loads(linea[len("RESULTADO "):])
        self.assertEqual(d["errores"], [])
        self.assertIn("Segundo plano", d["secciones"])
        self.assertGreater(d["grupos"], 3)
        self.assertTrue(d["filtrado"])
        self.assertTrue(d["detalles"])
        self.assertTrue(d["finalizado"])
        self.assertTrue(d["abierto"])
        self.assertGreaterEqual(d["historia"], 4)
        self.assertFalse(d["verProcesos"])
        self.assertGreater(d["servicios"], 0)


if __name__ == "__main__":
    unittest.main()
