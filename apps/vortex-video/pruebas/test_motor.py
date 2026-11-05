"""Pruebas de lo que no necesita cámara: nombres, ajustes y elegir el formato.

    python3 -m unittest discover -s apps/vortex-video/pruebas
"""
from __future__ import annotations

import json
import os
import sys
import tempfile
import unittest
from datetime import datetime
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from vortex_video import motor  # noqa: E402

C920 = [{"ancho": 640, "alto": 480, "fps": 30}, {"ancho": 1920, "alto": 1080, "fps": 5},
        {"ancho": 1920, "alto": 1080, "fps": 30}, {"ancho": 1280, "alto": 720, "fps": 10},
        {"ancho": 1280, "alto": 720, "fps": 30}, {"ancho": 2304, "alto": 1536, "fps": 2}]


class Formatos(unittest.TestCase):
    def test_la_mayor_fluida_no_la_mayor_a_saltos(self):
        self.assertEqual(C920[motor.elegir_formato(C920)], {"ancho": 1920, "alto": 1080, "fps": 30})

    def test_respeta_la_elegida(self):
        self.assertEqual(C920[motor.elegir_formato(C920, "1280x720")]["fps"], 30)
        self.assertEqual(C920[motor.elegir_formato(C920, "1280x720")]["alto"], 720)

    def test_elegida_que_ya_no_existe(self):
        self.assertEqual(motor.resolucion(C920[motor.elegir_formato(C920, "9x9")]), "1920x1080")

    def test_camara_sin_nada_fluido(self):
        lenta = [{"ancho": 640, "alto": 480, "fps": 10}, {"ancho": 320, "alto": 240, "fps": 15}]
        self.assertEqual(motor.elegir_formato(lenta), 0)

    def test_sin_formatos(self):
        self.assertEqual(motor.elegir_formato([]), -1)
        self.assertEqual(motor.resoluciones(C920), ["1920x1080", "1280x720", "640x480"])

    def test_etiquetas(self):
        self.assertEqual(motor.etiqueta_resolucion("1920x1080"), "1920 × 1080 (Full HD)")
        self.assertEqual(motor.etiqueta_resolucion("640x480"), "640 × 480")


class Archivos(unittest.TestCase):
    def setUp(self):
        self.t = tempfile.TemporaryDirectory()
        self.d = Path(self.t.name)
        self.addCleanup(self.t.cleanup)

    def test_nombre_no_pisa(self):
        ahora = datetime(2026, 11, 5, 14, 3, 21)
        a = motor.nombre_nuevo(self.d, "Foto", ".jpg", ahora)
        self.assertEqual(a.name, "Foto 2026-11-05 14.03.21.jpg")
        a.touch()
        self.assertEqual(motor.nombre_nuevo(self.d, "Foto", ".jpg", ahora).name, "Foto 2026-11-05 14.03.21 (2).jpg")

    def test_ultima_captura(self):
        (self.d / "a.jpg").touch()
        (self.d / "b.mp4").touch()
        (self.d / "nota.txt").touch()
        os.utime(self.d / "a.jpg", (1000, 1000))
        os.utime(self.d / "b.mp4", (2000, 2000))
        os.utime(self.d / "nota.txt", (3000, 3000))
        self.assertEqual(motor.ultima_captura(self.d, self.d / "no-existe").name, "b.mp4")
        self.assertIsNone(motor.ultima_captura(self.d / "no-existe"))

    def test_ajustes(self):
        r = self.d / "v" / "video.json"
        self.assertEqual(motor.leer_ajustes(r), motor.AJUSTES_POR_DEFECTO)
        a = motor.leer_ajustes(r)
        a["modo"] = "video"
        motor.guardar_ajustes(a, r)
        self.assertEqual(motor.leer_ajustes(r)["modo"], "video")

    def test_ajustes_rotos(self):
        r = self.d / "video.json"
        r.write_text("no es json")
        self.assertEqual(motor.leer_ajustes(r), motor.AJUSTES_POR_DEFECTO)
        r.write_text(json.dumps({"modo": "raro", "temporizador": 7, "microfono": "si", "otra": 1}))
        self.assertEqual(motor.leer_ajustes(r), motor.AJUSTES_POR_DEFECTO)

    def test_duracion(self):
        self.assertEqual(motor.duracion(0), "00:00")
        self.assertEqual(motor.duracion(75_000), "01:15")
        self.assertEqual(motor.duracion(3_661_000), "1:01:01")


if __name__ == "__main__":
    unittest.main()
