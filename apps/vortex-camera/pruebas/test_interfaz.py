"""Pruebas de la ventana de Vortexine's Camera, sin pantalla (offscreen) y con la cámara falsa: carga sin
errores de QML, el mosaico recibe video, una cámara con el código malo dice «Código incorrecto», la
vista grande y las grabaciones reciben video, y el código de seguridad no sale en la terminal.
La configuración va a una carpeta temporal: no toca tus cámaras.

    python3 -m unittest discover -s apps/vortex-camera/pruebas
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
    import json, sys, time
    sys.path.insert(0, %r)
    sys.path.insert(0, %r)
    from camara_falsa import CamaraFalsa
    from vortex_camera.__main__ import crear_aplicacion, cargar_interfaz
    from vortex_camera.puente import Puente
    from vortex_camera.camaras import Almacen

    app = crear_aplicacion(["vc"])
    falsa = CamaraFalsa(codigo="CLAVE7Z9")
    puente = Puente(app, Almacen(), revisar=False)
    motor, puente = cargar_interfaz(app, puente)
    errores = []
    motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
    v = motor.rootObjects()[0]
    datos = {}

    def esperar(condicion, segundos=15):
        fin = time.time() + segundos
        while time.time() < fin:
            app.processEvents()
            if condicion():
                return True
            time.sleep(0.02)
        return False

    def todos(item):
        # Los delegados de un Repeater no son hijos QObject de la ventana: se recorre el árbol visual.
        yield item
        for hijo in item.childItems():
            yield from todos(hijo)

    def objetos(prop):
        return [o for o in todos(v.contentItem()) if o.metaObject().indexOfProperty(prop) >= 0]

    def celdas():
        return [o for o in objetos("camId") if o.metaObject().indexOfProperty("pos") >= 0]

    datos["vacio"] = puente.agregarCamara({"nombre": "", "codigo": "x", "ip": "1.2.3.4"})
    datos["sin_codigo"] = puente.agregarCamara({"nombre": "A", "ip": "1.2.3.4"})
    datos["ip_mala"] = puente.agregarCamara({"nombre": "A", "codigo": "x", "ip": "no es ip!"})
    datos["agregar"] = puente.agregarCamara({"nombre": "Patio", "codigo": falsa.codigo, "ip": "127.0.0.1", "puerto": falsa.puerto})
    puente.agregarCamara({"nombre": "Calle", "codigo": "MALO", "ip": "127.0.0.1", "puerto": falsa.puerto})
    patio, calle = [c["id"] for c in puente.camaras]
    datos["codigo_en_qml"] = "CLAVE7Z9" in json.dumps(puente.camaras)

    def celda(ident):
        return next((c for c in celdas() if c.property("camId") == ident), None)

    datos["mosaico_video"] = esperar(lambda: celda(patio) is not None and celda(patio).property("conVideo"))
    datos["estados"] = esperar(lambda: puente.estados.get(patio) == "en_linea" and puente.estados.get(calle) == "codigo")
    datos["estados_valor"] = puente.estados
    datos["calle_sin_video"] = not celda(calle).property("conVideo")

    # vista grande: el mosaico suelta su video y la grande recibe el principal
    v.setProperty("grande", patio)
    grande = next(o for o in objetos("audio") if o.metaObject().indexOfProperty("camId") >= 0 and o.metaObject().indexOfProperty("pos") < 0 and o.property("url") is not None and "subtype=0" in (o.property("url") or ""))
    datos["grande_url_principal"] = "subtype=0" in grande.property("url")
    datos["grande_video"] = esperar(lambda: grande.property("conVideo"))
    datos["mosaico_suelta"] = celda(patio).property("url") == ""
    v.setProperty("grande", "")

    # división y ventanas
    puente.ponerDivision(0, 9)
    datos["nueve"] = esperar(lambda: len(celdas()) == 9, 5)
    i = puente.nuevaVentana()
    puente.renombrarVentana(i, "Calle")
    datos["ventanas"] = [w["nombre"] for w in puente.ventanas]

    # grabaciones de la microSD
    v.setProperty("pagina", "grabaciones")
    pag = next(o for o in objetos("fecha") if o.metaObject().indexOfProperty("hora") >= 0)
    from PySide6.QtCore import QMetaObject, Q_ARG, Qt
    QMetaObject.invokeMethod(pag, "elegir", Qt.DirectConnection, Q_ARG("QVariant", patio))
    datos["grabado_url"] = "/cam/playback?" in pag.property("url")
    datos["grabado_video"] = esperar(lambda: pag.property("conVideo"))
    datos["ultima_ruta"] = falsa.rutas[-1].split("?")[0]

    datos["errores"] = errores
    puente.cerrar()
    falsa.cerrar()
    print("DATOS" + json.dumps(datos))
    sys.stdout.flush()
""")


class PruebaInterfaz(unittest.TestCase):
    def test_ventana(self):
        with tempfile.TemporaryDirectory() as carpeta:
            entorno = dict(os.environ, QT_QPA_PLATFORM="offscreen", VORTEX_CAMERA_CONFIG=carpeta,
                           VORTEX_CAMERA_COLORES=str(Path(carpeta) / "no-hay.json"))
            r = subprocess.run([sys.executable, "-c", GUION % (str(RAIZ), str(RAIZ / "pruebas"))],
                               capture_output=True, text=True, timeout=150, env=entorno)
        linea = next((l for l in r.stdout.splitlines() if l.startswith("DATOS")), None)
        self.assertIsNotNone(linea, r.stdout[-2000:] + r.stderr[-3000:])
        d = json.loads(linea[5:])
        self.assertEqual(d["errores"], [])
        self.assertEqual(d["vacio"], "Ponle un nombre a la cámara.")
        self.assertIn("código de seguridad", d["sin_codigo"])
        self.assertIn("IP", d["ip_mala"])
        self.assertEqual(d["agregar"], "")
        self.assertFalse(d["codigo_en_qml"])
        self.assertTrue(d["mosaico_video"], "el mosaico no recibió video")
        self.assertTrue(d["estados"], d["estados_valor"])
        self.assertTrue(d["calle_sin_video"])
        self.assertTrue(d["grande_url_principal"])
        self.assertTrue(d["grande_video"], "la vista grande no recibió video")
        self.assertTrue(d["mosaico_suelta"])
        self.assertTrue(d["nueve"])
        self.assertEqual(d["ventanas"], ["Ventana 1", "Calle"])
        self.assertTrue(d["grabado_url"])
        self.assertTrue(d["grabado_video"], "las grabaciones no recibieron video")
        self.assertEqual(d["ultima_ruta"], "cam/playback")
        # El código no se escribe en la terminal (ni el bueno ni el malo).
        self.assertNotIn("CLAVE7Z9", r.stderr)
        self.assertNotIn("admin:MALO", r.stderr)


if __name__ == "__main__":
    unittest.main()
