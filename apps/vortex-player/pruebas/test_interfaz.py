"""Pruebas de la ventana, el reproductor y MPRIS de Vortex Player, sin pantalla (offscreen), sin sonido
(mpv con --ao=null) y con MPRIS en un D-Bus aparte (dbus-run-session): no aparece en tu panel de música.

    python3 -m unittest discover -s apps/vortex-player/pruebas
"""
from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

AQUI = Path(__file__).resolve().parent
RAIZ = AQUI.parent
HAY_TODO = all(shutil.which(p) for p in ("ffmpeg", "mpv", "dbus-run-session", "gdbus"))


def _guion(cuerpo: str) -> str:
    """Un programa que abre Vortex Player con música de prueba y corre `cuerpo` (con `paso(ms, f)`)."""
    return textwrap.dedent("""
        import os, sys, subprocess, threading, json
        sys.path.insert(0, %r)
        from PySide6.QtCore import QTimer
        from vortex_player.__main__ import crear_aplicacion, cargar_interfaz
        app = crear_aplicacion(["vp"])
        motor, puente = cargar_interfaz(app)
        assert motor.rootObjects(), "la ventana no cargó"
        ventana = motor.rootObjects()[0]
        r = puente.r
        errores = []
        motor.warnings.connect(lambda ws: errores.extend(w.toString() for w in ws))
        tiempo = [0]
        def paso(ms, f):
            tiempo[0] += ms
            QTimer.singleShot(tiempo[0], f)
        def fin(**datos):
            datos["errores_qml"] = errores
            print("RESULTADO " + json.dumps(datos, ensure_ascii=False), flush=True)
            r.cerrar()
            app.quit()
    """ % str(RAIZ)) + textwrap.dedent(cuerpo) + "\napp.exec()\n"


@unittest.skipUnless(HAY_TODO, "faltan ffmpeg, mpv o dbus")
class Interfaz(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.t = Path(tempfile.mkdtemp(prefix="vp-", dir="/tmp"))   # corto: el socket de mpv tiene límite
        m = cls.t / "Música"
        for nombre, titulo, opciones in (("a.flac", "Uno", ["-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"]),
                                         ("b.wav", "Dos", ["-c:a", "pcm_s16le"]),
                                         ("c.opus", "Tres", ["-c:a", "libopus"])):
            (m / "disco").mkdir(parents=True, exist_ok=True)
            subprocess.run(["ffmpeg", "-v", "quiet", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=30",
                            *opciones, "-metadata", f"title={titulo}", str(m / "disco" / nombre)], check=True)

    @classmethod
    def tearDownClass(cls) -> None:
        shutil.rmtree(cls.t, ignore_errors=True)

    def correr(self, cuerpo: str, nombre: str) -> dict:
        import json
        d = self.t / nombre
        d.mkdir(exist_ok=True)
        (d / "guion.py").write_text(_guion(cuerpo))
        entorno = {**os.environ, "QT_QPA_PLATFORM": "offscreen", "VORTEX_PLAYER_AO": "null",
                   "VORTEX_PLAYER_MUSICA": str(self.t / "Música"), "VORTEX_PLAYER_DATOS": str(d / "datos"),
                   "VORTEX_PLAYER_CONFIG": str(d / "config"), "VORTEX_PLAYER_CACHE": str(d / "cache"),
                   "XDG_CONFIG_HOME": str(d / "xdg"), "XDG_RUNTIME_DIR": str(d),
                   "VORTEX_PLAYER_EQ": str(d / "eq.json"), "SERPANTINUM_DIR": str(d / "sin-shell")}
        r = subprocess.run(["dbus-run-session", "--", sys.executable, str(d / "guion.py")], env=entorno,
                           capture_output=True, text=True, timeout=90)
        linea = next((x for x in r.stdout.splitlines() if x.startswith("RESULTADO ")), None)
        self.assertIsNotNone(linea, r.stdout[-2000:] + r.stderr[-3000:])
        return json.loads(linea[len("RESULTADO "):])

    def test_toca_salta_y_playlists(self) -> None:
        d = self.correr("""
            def empezar():
                datos["canciones"] = [c["titulo"] for c in puente.canciones]
                datos["etiquetas"] = [c["etiqueta"] for c in puente.canciones]
                puente.ordenar("calidad")
                datos["por_calidad"] = [c["titulo"] for c in puente.canciones]
                puente.tocar(0)
            def medir():
                datos["sonando"] = r.cancion.get("titulo")
                datos["avanza"] = r.posicion > 0.3
                r.siguiente()
            def despues():
                datos["siguiente"] = r.cancion.get("titulo")
                r.alternar()
            def pausado():
                datos["pausado"] = r.pausado
                r.alternarRepetir(); r.alternarRepetir()
                datos["repetir"] = r.repetir
                pid = puente.crearPlaylist("Para correr")
                puente.agregarAPlaylist(pid, puente.canciones[2]["ruta"])
                puente.verPlaylist(pid)
                datos["playlist"] = [c["titulo"] for c in puente.canciones]
                datos["playlists"] = [p["nombre"] for p in puente.playlists]
                for p in ("agregar", "ecualizador", "ajustes", "canciones"):
                    ventana.setProperty("pagina", p)
            datos = {}
            paso(2500, empezar)
            paso(1500, medir)
            paso(800, despues)
            paso(600, pausado)
            paso(600, lambda: fin(**datos))
        """, "toca")
        self.assertEqual(sorted(d["canciones"]), ["Dos", "Tres", "Uno"])
        self.assertEqual(d["por_calidad"], ["Uno", "Dos", "Tres"])
        self.assertIn("Hi-Res 24/96", d["etiquetas"])
        self.assertEqual(d["sonando"], "Uno")
        self.assertTrue(d["avanza"], "mpv no avanzó")
        self.assertEqual(d["siguiente"], "Dos")
        self.assertTrue(d["pausado"])
        self.assertEqual(d["repetir"], "una")
        self.assertEqual(d["playlist"], ["Tres"])
        self.assertEqual(d["playlists"], ["Para correr"])
        self.assertEqual(d["errores_qml"], [])

    def test_mpris(self) -> None:
        d = self.correr("""
            from vortex_player.mpris import Mpris
            mpris = Mpris(r, puente.portada_de, puente.mostrarVentana.emit)
            datos = {}
            def gdbus(*args):
                return subprocess.run(["gdbus", "call", "--session", "--dest", "org.mpris.MediaPlayer2.vortex_player",
                                       "--object-path", "/org/mpris/MediaPlayer2", *args],
                                      capture_output=True, text=True, timeout=10).stdout.strip()
            def en_hilo(f):
                threading.Thread(target=f, daemon=True).start()
            def preguntar():
                datos["identidad"] = gdbus("--method", "org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2", "Identity")
                datos["estado0"] = gdbus("--method", "org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "PlaybackStatus")
            def tocar():
                puente.tocar(0)
            def preguntar2():
                datos["estado1"] = gdbus("--method", "org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "PlaybackStatus")
                datos["meta"] = gdbus("--method", "org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "Metadata")
                gdbus("--method", "org.mpris.MediaPlayer2.Player.PlayPause")
            def preguntar3():
                datos["estado2"] = gdbus("--method", "org.freedesktop.DBus.Properties.Get", "org.mpris.MediaPlayer2.Player", "PlaybackStatus")
                gdbus("--method", "org.mpris.MediaPlayer2.Player.Next")
            def ver():
                datos["titulo_tras_next"] = r.cancion.get("titulo")
            paso(2000, lambda: en_hilo(preguntar))
            paso(1500, tocar)
            paso(1500, lambda: en_hilo(preguntar2))
            paso(1500, lambda: en_hilo(preguntar3))
            paso(1500, ver)
            paso(300, lambda: fin(**datos))
        """, "mpris")
        self.assertIn("Vortex Player", d["identidad"])
        self.assertIn("Stopped", d["estado0"])
        self.assertIn("Playing", d["estado1"])
        self.assertIn("'xesam:title': <'", d["meta"])
        self.assertIn("Paused", d["estado2"])
        self.assertNotEqual(d["titulo_tras_next"], None)
        self.assertEqual(d["errores_qml"], [])


if __name__ == "__main__":
    unittest.main()
