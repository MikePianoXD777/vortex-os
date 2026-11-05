"""La ventana sin pantalla (offscreen), con un sendme falso: carga sin errores de QML, agrega archivos y crea
un código. No toca Tailscale, tu portapapeles ni tu cortafuegos.

    python3 -m unittest discover -s apps/vortex-shared/pruebas
"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent

GUION = textwrap.dedent("""
    import json, sys
    sys.path.insert(0, %r)
    from PySide6.QtCore import QObject, QTimer
    from vortex_shared.ventana import crear_aplicacion, cargar_interfaz
    app = crear_aplicacion(["vs"])
    motor, puente = cargar_interfaz(app)
    errores = []
    motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
    v = motor.rootObjects()[0]
    datos = {}
    def paso1():
        puente.agregarRutas([%r, %r, "/no/existe"])
        datos["archivos"] = [a["nombre"] for a in puente.archivos]
        datos["usuario"] = v.findChild(QObject, "miUsuario").property("text")
        puente.quitar(1)
        datos["tras_quitar"] = len(puente.archivos)
        puente.crearCodigo()
        QTimer.singleShot(1200, paso2)
    def paso2():
        datos["codigo"] = puente.codigo
        datos["ocupado"] = puente.ocupado
        puente.cancelar()
        datos["tras_cancelar"] = [puente.codigo, puente.ocupado]
        puente.mandar("04:42:1a:08:6b:62", "mike")
        QTimer.singleShot(800, paso3)
    def paso3():
        datos["mac"] = puente.estado
        datos["errores"] = errores
        print("DATOS" + json.dumps(datos))
        puente.cerrar()
        app.quit()
    QTimer.singleShot(300, paso1)
    app.exec()
""")


class Interfaz(unittest.TestCase):
    def test_ventana(self):
        with tempfile.TemporaryDirectory() as t:
            d = Path(t)
            (d / "bin").mkdir()
            (d / "bin" / "sendme").write_text('#!/bin/sh\necho "sendme receive blobaaaaaaaaaaaaaaaa"\nexec sleep 600\n')
            (d / "bin" / "sendme").chmod(0o755)
            for f in ("tailscale", "ufw"):
                (d / "bin" / f).write_text("#!/bin/sh\nexit 1\n")
                (d / "bin" / f).chmod(0o755)
            a, b = d / "a.txt", d / "b.txt"
            a.write_text("hola")
            b.write_text("adios")
            entorno = dict(os.environ, QT_QPA_PLATFORM="offscreen", PATH=f"{d / 'bin'}:{os.environ['PATH']}", XDG_CONFIG_HOME=t,
                           XDG_CACHE_HOME=t, VORTEX_SHARED_COLORES=str(d / "no-hay.json"), VORTEX_SHARED_CARPETA=str(d / "rec"))
            r = subprocess.run([sys.executable, "-c", GUION % (str(RAIZ), str(a), str(b))], capture_output=True, text=True, env=entorno, timeout=60)
            linea = [x for x in r.stdout.splitlines() if x.startswith("DATOS")]
            self.assertTrue(linea, r.stderr[-1500:])
            datos = json.loads(linea[0][5:])
            self.assertEqual(datos["archivos"], ["a.txt", "b.txt"])
            self.assertEqual(datos["tras_quitar"], 1)
            self.assertTrue(datos["codigo"].startswith("blob"))
            self.assertTrue(datos["ocupado"])
            self.assertEqual(datos["tras_cancelar"], ["", False])
            self.assertIn("MAC", datos["mac"])
            self.assertEqual(datos["errores"], [])
            ruidos = [x for x in r.stderr.splitlines() if ".qml" in x or "TypeError" in x or "ReferenceError" in x]
            self.assertEqual(ruidos, [])


if __name__ == "__main__":
    unittest.main()
