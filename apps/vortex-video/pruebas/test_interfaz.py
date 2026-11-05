"""La ventana sin pantalla (offscreen) y haciendo de cuenta que no hay cámara: nunca abre la tuya. Carga sin
errores de QML, dice que falta la cámara, guarda los ajustes y muestra la última captura.

    python3 -m unittest discover -s apps/vortex-video/pruebas
"""
from __future__ import annotations

import base64
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
PNG = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAEBgIApD5fRAAAAABJRU5ErkJggg=="

GUION = textwrap.dedent("""
    import json, sys
    sys.path.insert(0, %r)
    from PySide6.QtCore import QObject, QTimer
    from vortex_video.__main__ import crear_aplicacion, cargar_interfaz
    app = crear_aplicacion(["vv"])
    errores = []
    from PySide6.QtQml import QQmlApplicationEngine
    motor, puente = cargar_interfaz(app)
    motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
    v = motor.rootObjects()[0]
    datos = {}
    def medir():
        datos["mensaje"] = v.findChild(QObject, "mensajeSinCamara").property("text")
        datos["disparador"] = v.findChild(QObject, "disparador").property("enabled")
        datos["ultima"] = puente.ultima
        datos["miniatura"] = puente.miniatura
        datos["errores"] = errores
        puente.guardar("modo", "video")
        print("DATOS" + json.dumps(datos))
        app.quit()
    QTimer.singleShot(800, medir)
    app.exec()
""")


class Interfaz(unittest.TestCase):
    def test_sin_camara(self):
        with tempfile.TemporaryDirectory() as t:
            fotos = Path(t) / "PicturesLocation" / "Cámara"
            fotos.mkdir(parents=True)
            # Una imagen de verdad de 1x1, con el formato que dice su contenido.
            (fotos / "Foto vieja.png").write_bytes(base64.b64decode(PNG))
            entorno = dict(os.environ, QT_QPA_PLATFORM="offscreen", VORTEX_VIDEO_SIN_CAMARA="1", VORTEX_VIDEO_CARPETA=t,
                           XDG_CONFIG_HOME=t, XDG_CACHE_HOME=t, VORTEX_VIDEO_COLORES=str(Path(t) / "no-hay.json"))
            r = subprocess.run([sys.executable, "-c", GUION % str(RAIZ)], capture_output=True, text=True, env=entorno, timeout=60)
            linea = [x for x in r.stdout.splitlines() if x.startswith("DATOS")]
            self.assertTrue(linea, r.stderr[-1500:])
            datos = json.loads(linea[0][5:])
            self.assertEqual(datos["mensaje"], "No encuentro tu cámara")
            self.assertFalse(datos["disparador"])
            self.assertTrue(datos["ultima"].endswith("Foto vieja.png"))
            self.assertTrue(datos["miniatura"].startswith("file:"))
            self.assertEqual(datos["errores"], [])
            ruidos = [x for x in r.stderr.splitlines() if ".qml" in x or "TypeError" in x or "ReferenceError" in x]
            self.assertEqual(ruidos, [])
            self.assertEqual(json.loads((Path(t) / "vortex" / "video.json").read_text())["modo"], "video")


if __name__ == "__main__":
    unittest.main()
