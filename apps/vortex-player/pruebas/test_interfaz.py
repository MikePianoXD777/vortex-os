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
        # Un equalizer.sh falso: el de verdad (el de /usr/share/serpantinum) cambia el ecualizador y el
        # EasyEffects reales de quien corre las pruebas (el 29/10 tumbó EasyEffects y el shell de Mike)
        eq = d / "sin-shell" / "quickshell" / "media" / "equalizer.sh"
        eq.parent.mkdir(parents=True, exist_ok=True)
        eq.write_text(f'#!/bin/sh\necho "$*" >>"{d}/eq.log"\n')
        eq.chmod(0o755)
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

    def test_buscar_y_bajar_a_playlist(self) -> None:
        # 2.1: buscar en YouTube Music, bajar un resultado directo a una playlist y que salga «En tu biblioteca».
        # Sin internet: la búsqueda y la descarga son falsas, pero el archivo pasa de verdad por la biblioteca.
        d = self.correr("""
            import shutil, time
            from pathlib import Path
            from vortex_player import descargas
            origen = Path(os.environ["VORTEX_PLAYER_MUSICA"]) / "disco" / "b.wav"
            def buscar_falso(texto, fuente="ytmusic", n=20, **_):
                time.sleep(0.2)
                return [{"id": "abcdefghijk", "url": "https://music.youtube.com/watch?v=abcdefghijk", "titulo": "Idol",
                         "artistas": "YOASOBI", "album": "Idol", "duracion": 214, "portada": ""},
                        {"id": "zzzzzzzzzzz", "url": "https://music.youtube.com/watch?v=zzzzzzzzzzz", "titulo": "Otra",
                         "artistas": "Nadie", "album": "", "duracion": 0, "portada": ""}]
            def descargar_falso(url, destino, *a, **k):
                destino = Path(os.environ["VORTEX_PLAYER_DATOS"]) / "bajadas"   # no en la música de las otras pruebas
                yield {"tipo": "inicio", "nombre": "", "total": 1, "es_lista": False}
                time.sleep(0.5)
                (destino / "YOASOBI").mkdir(parents=True, exist_ok=True)
                ruta = destino / "YOASOBI" / "Idol [abcdefghijk].wav"
                shutil.copy(origen, ruta)
                yield {"tipo": "archivo", "ruta": str(ruta)}
                yield {"tipo": "fin", "bajadas": 1, "fallidas": 0}
            descargas.buscar = buscar_falso
            descargas.descargar = descargar_falso
            datos = {}
            def buscar():
                ventana.setProperty("pagina", "agregar")
                puente.buscarEnLinea("yoasobi idol", "ytmusic")
                datos["buscando"] = puente.buscandoEnLinea
            def bajar():
                datos["resultados"] = [(c["titulo"], c["estado"]) for c in puente.enLinea]
                datos["pid"] = puente.crearPlaylist("Anime")
                puente.bajarResultado(0, datos["pid"])
                datos["durante"] = puente.enLinea[0]["estado"]
            def ver():
                datos["despues"] = [c["estado"] for c in puente.enLinea]
                puente.verPlaylist(datos["pid"])
                datos["playlist"] = [c["titulo"] for c in puente.canciones]
                datos["descarga"] = puente.descargasLista[0]["nombre"]
            paso(1500, buscar)
            paso(700, bajar)
            paso(1500, ver)
            paso(200, lambda: fin(**datos))
        """, "buscar")
        self.assertTrue(d["buscando"])
        self.assertEqual(d["resultados"], [["Idol", ""], ["Otra", ""]])
        self.assertEqual(d["durante"], "bajando")
        self.assertEqual(d["despues"], ["biblioteca", ""])
        self.assertEqual(len(d["playlist"]), 1)
        self.assertEqual(d["descarga"], "Idol · YOASOBI")
        self.assertEqual(d["errores_qml"], [])

    def test_recomendaciones_de_una_playlist(self) -> None:
        # 2.1: al abrir una playlist salen solas canciones parecidas; «Agregar» la baja y la mete a la playlist
        d = self.correr("""
            import shutil, time
            from pathlib import Path
            from vortex_player import descargas
            origen = Path(os.environ["VORTEX_PLAYER_MUSICA"]) / "disco" / "b.wav"
            llamadas = []
            def recomendar_falso(semillas, tienes, n=20, **_):
                llamadas.append([s["titulo"] for s in semillas])
                time.sleep(0.2)
                vuelta = len(llamadas)
                return [{"id": f"reco{vuelta}aaaaaa"[:11], "url": "https://music.youtube.com/watch?v=x", "titulo": f"Parecida {vuelta}",
                         "artistas": "Alguien", "album": "", "duracion": 200, "portada": ""}]
            def descargar_falso(url, destino, *a, **k):
                destino = Path(os.environ["VORTEX_PLAYER_DATOS"]) / "bajadas"
                yield {"tipo": "inicio", "nombre": "", "total": 1, "es_lista": False}
                destino.mkdir(parents=True, exist_ok=True)
                ruta = destino / "Parecida 1 [reco1aaaaaa].wav"
                shutil.copy(origen, ruta)
                yield {"tipo": "archivo", "ruta": str(ruta)}
                yield {"tipo": "fin", "bajadas": 1, "fallidas": 0}
            descargas.recomendar = recomendar_falso
            descargas.descargar = descargar_falso
            datos = {}
            def abrir():
                pid = puente.crearPlaylist("Mía")
                datos["pid"] = pid
                puente.agregarAPlaylist(pid, puente.canciones[0]["ruta"])
                puente.verPlaylist(pid)
                datos["cargando"] = puente.cargandoRecomendaciones
            def ver():
                datos["recos"] = [(c["titulo"], c["estado"]) for c in puente.recomendaciones]
                datos["para"] = puente.recomendandoPara == datos["pid"]
                puente.agregarRecomendada(0)
            def agregada():
                datos["despues"] = [c["estado"] for c in puente.recomendaciones]
                datos["en_playlist"] = len(puente.canciones)
                puente.verPlaylist(-1)
                puente.verPlaylist(datos["pid"])   # al volver: las mismas, sin pedir otra vez
                datos["llamadas_al_volver"] = len(llamadas)
                puente.recomendar(datos["pid"], True)   # «Otras»
            def otras():
                datos["otras"] = [c["titulo"] for c in puente.recomendaciones]
                datos["llamadas"] = len(llamadas)
            paso(1500, abrir)
            paso(700, ver)
            paso(1500, agregada)
            paso(700, otras)
            paso(200, lambda: fin(**datos))
        """, "recos")
        self.assertTrue(d["cargando"])
        self.assertTrue(d["para"])
        self.assertEqual(d["recos"], [["Parecida 1", ""]])
        self.assertEqual(d["despues"], ["biblioteca"])
        self.assertEqual(d["en_playlist"], 2)
        self.assertEqual(d["llamadas_al_volver"], 1)
        self.assertEqual((d["otras"], d["llamadas"]), (["Parecida 2"], 2))
        self.assertEqual(d["errores_qml"], [])

    def test_avance_de_una_recomendada(self) -> None:
        # 2.1.1: ▶ en una recomendada toca 30 s (un mpv aparte): tu música se pausa y al terminar sigue
        d = self.correr("""
            import stat, time
            from pathlib import Path
            from vortex_player import descargas
            t = Path(os.environ["VORTEX_PLAYER_DATOS"]).parent
            log = t / "avance.log"
            falso = t / "mpv-avance"
            # El mpv del avance, falso: anota sus argumentos, «abre el audio» y suena 1.5 s. Con FALLA_CLIENTE,
            # el cliente visionos falla y tiene que intentarlo otra vez sin él
            falso.write_text("#!/bin/sh\\necho \\"$*\\" >>" + str(log) + "\\n"
                             "case \\"$*\\" in *visionos*) [ -e " + str(t / "falla") + " ] && exit 2 ;; esac\\n"
                             "sleep 0.3; echo 'AO: [null] 48000Hz'; sleep 1.5\\n")
            falso.chmod(falso.stat().st_mode | stat.S_IEXEC)
            descargas.recomendar = lambda semillas, tienes, n=20, **_: [
                {"id": "recoaaaaaaa", "url": "https://music.youtube.com/watch?v=recoaaaaaaa", "titulo": "Parecida",
                 "artistas": "Alguien", "album": "", "duracion": 200, "portada": ""}]
            datos = {}
            def abrir():
                pid = puente.crearPlaylist("Mía")
                puente.agregarAPlaylist(pid, puente.canciones[0]["ruta"])
                puente.verPlaylist(pid)
                puente.tocar(0)
            def escuchar():
                os.environ["VORTEX_PLAYER_MPV"] = str(falso)
                datos["sonaba"] = not r.pausado
                puente.escucharRecomendada(0)
                datos["cargando"] = (puente.avance.idActual, puente.avance.sonando)
            def sonando():
                datos["sonando"] = (puente.avance.idActual, puente.avance.sonando)
                datos["musica_pausada"] = r.pausado
                datos["args"] = log.read_text()
            def acabo():
                datos["al_terminar"] = puente.avance.idActual
                datos["musica_sigue"] = not r.pausado
                puente.escucharRecomendada(0)
            def parar():
                puente.escucharRecomendada(0)   # otra vez: lo para
            def parado():
                datos["parado"] = puente.avance.idActual
                datos["musica_sigue_2"] = not r.pausado
                (t / "falla").touch()
                log.write_text("")
                puente.escucharRecomendada(0)
            def sin_cliente():
                datos["intentos"] = log.read_text().splitlines()
                datos["sonando_2"] = puente.avance.sonando
                r.seguir()                      # le das play a tu música: el avance se calla
            def play():
                datos["tras_play"] = puente.avance.idActual
                datos["musica_3"] = not r.pausado
            paso(1500, abrir)
            paso(1500, escuchar)
            paso(800, sonando)
            paso(1800, acabo)
            paso(900, parar)
            paso(900, parado)
            paso(1000, sin_cliente)
            paso(1500, play)                    # mpv a veces tarda en quitar la pausa con todas las pruebas juntas
            paso(200, lambda: fin(**datos))
        """, "avance")
        self.assertTrue(d["sonaba"])
        self.assertEqual(d["cargando"], ["recoaaaaaaa", False])
        self.assertEqual(d["sonando"], ["recoaaaaaaa", True])
        self.assertTrue(d["musica_pausada"])
        self.assertIn("--start=70 --end=100", d["args"])          # desde el 35 % de la canción, 30 s
        self.assertIn("player_client=visionos", d["args"])
        self.assertIn("watch?v=recoaaaaaaa", d["args"])
        self.assertEqual(d["al_terminar"], "")
        self.assertTrue(d["musica_sigue"])
        self.assertEqual(d["parado"], "")
        self.assertTrue(d["musica_sigue_2"])
        self.assertEqual(len(d["intentos"]), 2)
        self.assertNotIn("visionos", d["intentos"][1])
        self.assertTrue(d["sonando_2"])
        self.assertEqual(d["tras_play"], "")
        self.assertTrue(d["musica_3"])
        self.assertEqual(d["errores_qml"], [])

    def test_arrastrar_banda_del_ecualizador(self) -> None:
        # 2.1: al arrastrar una barra temblaba y se soltaba (cada movimiento volvía a crear las barras)
        d = self.correr("""
            from PySide6.QtCore import QPointF, Qt
            from PySide6.QtTest import QTest
            datos = {"valores": [], "misma": True}
            barras = []
            def sliders():
                encontrados, pila = [], [ventana.contentItem()]
                while pila:
                    o = pila.pop(0)
                    if o.inherits("QQuickSlider") and o.isVisible() and o.property("vertical"):
                        encontrados.append(o)
                    pila.extend(o.childItems())
                return encontrados
            def ir():
                ventana.setProperty("pagina", "ecualizador")
            def agarrar():
                barras[:] = sliders()
                datos["barras"] = len(barras)
                b = barras[3]
                datos["id"] = id(b)
                c = b.mapToScene(QPointF(b.width() / 2, b.height() / 2)).toPoint()
                datos["xy"] = [c.x(), c.y()]
                QTest.mousePress(ventana, Qt.LeftButton, Qt.NoModifier, c)
            def mover(dy):
                def f():
                    x, y = datos["xy"]
                    datos["xy"] = [x, y + dy]
                    from PySide6.QtCore import QPoint
                    QTest.mouseMove(ventana, QPoint(x, y + dy))
                    actuales = sliders()
                    datos["misma"] = datos["misma"] and len(actuales) == 10 and actuales[3] is barras[3]
                    datos["valores"].append(barras[3].property("value"))
                return f
            def soltar():
                from PySide6.QtCore import QPoint
                QTest.mouseRelease(ventana, Qt.LeftButton, Qt.NoModifier, QPoint(*datos["xy"]))
                datos["eq"] = puente.eq["bandas"][3]["g"]
                datos["final"] = barras[3].property("value")
            from PySide6.QtCore import QObject
            paso(1500, ir)
            paso(500, agarrar)
            for _ in range(8):
                paso(60, mover(-12))
            paso(600, mover(-12))   # después de que se aplicó (400 ms) sigue en la mano
            paso(100, soltar)
            paso(300, lambda: fin(**datos))
        """, "eq")
        self.assertEqual(d["barras"], 10)
        self.assertTrue(d["misma"], "la barra se volvió a crear mientras se arrastraba")
        v = d["valores"]
        self.assertTrue(all(b >= a for a, b in zip(v, v[1:])) and v[-1] > 0, v)
        self.assertEqual(d["final"], d["eq"])
        self.assertEqual(d["errores_qml"], [])


if __name__ == "__main__":
    unittest.main()
