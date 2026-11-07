"""Pruebas de la ventana, el reproductor y MPRIS de Vortex Player, sin pantalla (offscreen), sin sonido
(mpv con --ao=null) y con MPRIS en un D-Bus aparte (dbus-run-session): no aparece en tu panel de música.

    python3 -m unittest discover -s apps/vortex-player/pruebas
"""
from __future__ import annotations

import os
import re
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
                   "VORTEX_PLAYER_EQ": str(d / "eq.json"), "SERPANTINUM_DIR": str(d / "sin-shell"),
                   "VORTEX_PLAYER_SIN_RED": "1"}
        r = subprocess.run(["dbus-run-session", "--", sys.executable, str(d / "guion.py")], env=entorno,
                           capture_output=True, text=True, timeout=90)
        linea = next((x for x in r.stdout.splitlines() if x.startswith("RESULTADO ")), None)
        self.assertIsNotNone(linea, r.stdout[-2000:] + r.stderr[-3000:])
        return json.loads(linea[len("RESULTADO "):])

    def test_qml_no_recorre_listas_de_python(self) -> None:
        # Cada lectura de una lista de Python desde QML la copia entera: con 708 canciones, recorrerla así
        # subió el Player a 1 GB. Los totales y la cola ya vienen hechos (vp.totales, rep.colaSiguientes).
        malas = re.compile(r"vp\.canciones[.\[]|rep\.cola\b(?!S|R)|vp\.meGusta\.|vp\.descargasLista\[")
        qml = Path(__file__).parent.parent / "vortex_player" / "qml"
        encontradas = [f"{f.name}:{n}" for f in qml.glob("*.qml")
                       for n, l in enumerate(f.read_text().splitlines(), 1) if malas.search(l.split("//")[0])]
        self.assertEqual(encontradas, [])

    def test_totales_cola_y_me_gusta_vienen_de_python(self) -> None:
        d = self.correr("""
            def empezar():
                datos["totales"] = puente.totales
                ruta = puente.canciones[0]["ruta"]
                datos["antes"] = puente.leGusta(ruta, 0)
                version = puente.versionGustos
                puente.alternarMeGusta(ruta)
                datos["despues"] = puente.leGusta(ruta, 0)
                datos["subio"] = puente.versionGustos > version
                puente.tocar(0)
            def sonando():
                datos["restantes"] = r.colaRestantes
                datos["siguientes"] = len(r.colaSiguientes)
                ventana.setProperty("pagina", "cola")
            datos = {}
            paso(2500, empezar)
            paso(800, sonando)
            paso(500, lambda: fin(**datos))
        """, "totales")
        self.assertEqual(d["totales"]["cuantas"], 3)
        self.assertEqual(d["totales"]["hires"], 1)
        self.assertGreater(d["totales"]["duracion"], 0)
        self.assertTrue(d["totales"]["primera"])
        self.assertEqual((d["antes"], d["despues"], d["subio"]), (False, True, True))
        self.assertEqual((d["restantes"], d["siguientes"]), (2, 2))

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

    def test_discord_con_el_reproductor_de_verdad(self) -> None:
        d = self.correr("""
            from PySide6.QtNetwork import QLocalServer
            from vortex_player import presencia_discord as pd
            servidor = QLocalServer()
            assert servidor.listen(os.environ["XDG_RUNTIME_DIR"] + "/discord-ipc-0")
            marcos, conexiones = [], []
            def leer(c):
                nuevos, _ = pd.leer_marcos(bytes(c.readAll()))
                for op, m in nuevos:
                    marcos.append(m)
                    if op == pd.SALUDO:
                        c.write(pd.marco(pd.MARCO, {"evt": "READY"}))
            def nueva():
                c = servidor.nextPendingConnection()
                conexiones.append(c)
                c.readyRead.connect(lambda: leer(c))
            servidor.newConnection.connect(nueva)
            presencia = pd.PresenciaDiscord(r, lambda: puente.bib.ajustes().get("discord", True), "123",
                                            entre_envios=0, espera_ms=50)
            puente.cambioAjustes.connect(presencia.actualizar)
            datos = {}
            def actividades():
                return [m["args"].get("activity") for m in marcos if m.get("cmd") == "SET_ACTIVITY"]
            def empezar():
                datos["prendido_de_serie"] = puente.ajustes.get("discord", True)
                puente.tocar(0)
            def sonando():
                a = actividades()[-1]
                datos["titulo"], datos["tiempo"] = a["details"], a["timestamps"]["end"] - a["timestamps"]["start"]
                r.alternar()
            def pausada():
                datos["pausa"] = actividades()[-1]
                r.alternar()
            def sigue():
                datos["sigue"] = actividades()[-1]["details"]
                puente.ponerDiscord(False)
            def apagada():
                datos["apagada"] = actividades()[-1]
                datos["guardado"] = puente.ajustes.get("discord")
                fin(**datos)
            paso(500, empezar)
            paso(1500, sonando)
            paso(500, pausada)
            paso(500, sigue)
            paso(500, apagada)
        """, "discord")
        self.assertTrue(d["prendido_de_serie"])
        self.assertIn(d["titulo"], ("Uno", "Dos", "Tres"))
        self.assertGreater(d["tiempo"], 0)
        self.assertIsNone(d["pausa"])
        self.assertEqual(d["sigue"], d["titulo"])
        self.assertIsNone(d["apagada"])
        self.assertIs(d["guardado"], False)
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
        self.assertEqual(d["despues"], [])   # 2.4: al bajarla se quita de las recomendadas (ya la tienes)
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


    def test_letra_perfil_y_seguir_donde_te_quedaste(self) -> None:
        # 2.4: la letra sincronizada (de la caché, sin internet), un perfil del ecualizador, y al cerrar y
        # volver a abrir sigue en la misma canción, en pausa y en el mismo segundo
        d = self.correr("""
            from vortex_player import letras, rutas
            datos = {}
            def empezar():
                i = next(k for k, c in enumerate(puente.canciones) if c["titulo"] == "Dos")
                c = puente.canciones[i]
                f = letras._archivo(rutas.cache() / "letras", c["titulo"], c.get("artista") or "", int(c.get("duracion") or 0))
                f.parent.mkdir(parents=True, exist_ok=True)
                f.write_text(json.dumps({"lineas": letras.leer_lrc("[00:01.00] uno\\n[00:05.00] cinco\\n[00:20.00] veinte"),
                                         "sincronizada": True, "cuando": 9e12}))
                puente.tocar(i)
            def letra():
                ventana.setProperty("pagina", "letra")
                r.buscar(6)
            def medir():
                l = puente.letra
                datos["letra"] = [x["texto"] for x in l["lineas"]]
                datos["sincronizada"] = l["sincronizada"]
                datos["linea"] = puente.lineaActual(r.posicion)
                puente.ponerPerfil("Rock")
            def cerrar():
                datos["perfil"] = puente.perfilEq
                r.buscar(12)
            def guardar():
                puente._guardar_sesion()
                fin(**datos)
            paso(2500, empezar)
            paso(1500, letra)
            paso(800, medir)
            paso(600, cerrar)
            paso(800, guardar)
        """, "sesion")
        self.assertEqual((d["letra"], d["sincronizada"], d["linea"]), (["uno", "cinco", "veinte"], True, 1))
        self.assertEqual(d["perfil"], "Rock")
        self.assertTrue((self.t / "sesion" / "eq.log").read_text().strip())   # el perfil llegó al ecualizador
        self.assertEqual(d["errores_qml"], [])
        d = self.correr("""
            datos = {}
            def medir():
                datos["sonando"] = r.cancion.get("titulo")
                datos["pausado"] = r.pausado
                datos["posicion"] = r.posicion
                datos["origen"] = r.origen
                datos["cola"] = len(r.sesion()["rutas"])
                fin(**datos)
            paso(2500, medir)
        """, "sesion")
        self.assertEqual((d["sonando"], d["pausado"], d["origen"], d["cola"]), ("Dos", True, "Tu biblioteca", 3))
        self.assertAlmostEqual(d["posicion"], 12, delta=1.5)
        self.assertEqual(d["errores_qml"], [])

    def test_me_gusta_cola_dormir_y_crossfade(self) -> None:
        # 2.4: ❤️ (playlist «Tus me gusta», primero en la lista), la cola «A continuación», el temporizador,
        # el crossfade (la que termina sigue en otro mpv) y el mini reproductor
        d = self.correr("""
            from PySide6.QtCore import QObject
            datos = {}
            def titulos():
                return [c["titulo"] for c in r.cola[r.indice:]]
            def empezar():
                puente.ordenar("titulo")   # Dos, Tres, Uno
                rutas = {c["titulo"]: c for c in puente.canciones}
                puente.alternarMeGusta(rutas["Uno"]["ruta"])
                puente.alternarMeGusta(rutas["Tres"]["ruta"])
                datos["playlists"] = [(p["nombre"], p["meGusta"], p["canciones"]) for p in puente.playlists]
                datos["me_gusta"] = [rutas[t]["ruta"] in puente.meGusta for t in ("Uno", "Dos", "Tres")]
                puente.alternarMeGusta(rutas["Tres"]["ruta"])
                datos["quitada"] = rutas["Tres"]["ruta"] in puente.meGusta
                puente.tocar(0)   # Dos (y la cola: Tres, Uno)
                puente.tocarDespues(rutas["Uno"])
                puente.tocarDespues(rutas["Tres"])
                puente.agregarACola(rutas["Dos"])
                datos["cola"] = titulos()
                r.quitarDeColaEn(r.indice + 2)
                datos["sin_una"] = titulos()
                ventana.setProperty("pagina", "cola")
                mini = ventana.findChild(QObject, "mini")
                datos["hay_mini"] = mini is not None
                if mini:
                    mini.setProperty("visible", True)
            def saltar():
                r.saltarA(r.indice + 1)
                datos["salto"] = r.cancion.get("titulo")
                r.dormir(-1)
                datos["dormir"] = [r.dormirActivo, r.dormirTexto]
                r.dormir(0)
                r.dormir(30)
                datos["dormir30"] = r.dormirTexto
                r.dormir(0)
                r.ponerFundido(3)
                datos["antes"] = r.cancion.get("titulo")
            def adelantar():
                r.buscar(r.duracion - 4)
            def fundido():
                datos["despues"] = r.cancion.get("titulo")
                datos["sonando_la_que_sale"] = len(r._cola_fundido)
                datos["pausado"] = r.pausado
                fin(**datos)
            paso(2500, empezar)
            paso(1500, saltar)
            paso(1000, adelantar)
            paso(2500, fundido)
        """, "megusta")
        self.assertEqual(d["playlists"][0], ["Tus me gusta", True, 2])
        self.assertEqual((d["me_gusta"], d["quitada"]), ([True, False, True], False))
        self.assertEqual(d["cola"], ["Dos", "Uno", "Tres", "Tres", "Uno", "Dos"])
        self.assertEqual(d["sin_una"], ["Dos", "Uno", "Tres", "Uno", "Dos"])
        self.assertTrue(d["hay_mini"])
        self.assertEqual(d["salto"], "Uno")
        self.assertEqual(d["dormir"], [True, "Al terminar la canción"])
        self.assertIn(d["dormir30"], ("30:00", "29:59"))
        self.assertEqual((d["antes"], d["despues"], d["pausado"]), ("Uno", "Tres", False))
        self.assertEqual(d["sonando_la_que_sale"], 1)
        self.assertEqual(d["errores_qml"], [])

    def test_radio_sin_fin_y_tu_resumen(self) -> None:
        # 1.4: al acabarse la cola siguen las de la radio (aquí, sin red, se le da la respuesta a mano) y
        # cada canción que sonó 30 s o más cuenta para «Tu resumen» (la tarjeta de Ajustes y el aviso)
        d = self.correr("""
            import time
            from vortex_player import biblioteca
            datos = {"se_acaba": 0}
            r.seAcaba.connect(lambda: datos.__setitem__("se_acaba", datos["se_acaba"] + 1))
            def empezar():
                puente.ordenar("titulo")   # Dos, Tres, Uno
                puente.tocar(2)            # Uno: la última
                datos["pidio"] = datos["se_acaba"]
            def adelantar():
                r.buscar(r.duracion - 1)
            def radio():
                datos["final"] = [r._final, r.pausado]
                uno = puente.canciones[0]["ruta"]
                puente._radio_vuelta, puente._radio_para = 7, r.cola[-1]["ruta"]
                puente._fin_radio(7, [{"id": "aaaaaaaaaaa", "url": uno, "titulo": "Radio uno", "artistas": "Alguien",
                                       "album": "", "duracion": 30, "portada": ""}], "")
                datos["sonando"] = [r.cancion.get("titulo"), bool(r.cancion.get("radio")), r.pausado]
                ventana.setProperty("pagina", "cola")
                # una respuesta vieja (la cola ya cambió) no se agrega
                puente._radio_vuelta, puente._radio_para = 8, "otra"
                puente._fin_radio(8, [{"id": "bbbbbbbbbbb", "url": uno, "titulo": "Vieja", "artistas": "",
                                       "duracion": 30}], "")
                datos["cola"] = [c["titulo"] for c in r.cola]
            def contar():
                datos["escuchado"] = r._escuchado
                r.escuchada.emit({"ruta": "x", "titulo": "Idol", "artista": "YOASOBI - Topic", "duracion": 200}, 120.0)
                r.escuchada.emit({"ruta": "x", "titulo": "Idol", "artista": "YOASOBI", "duracion": 200}, 60.0)
                antes = biblioteca.semana(1)[0] + 3600
                puente.bib.anotar_escucha({"ruta": "y", "titulo": "Vieja", "artista": "Otro"}, 90, cuando=antes)
                puente._resumen = {}
                s = puente.resumen
                datos["semana"] = [s["semana"]["escuchas"], s["semana"]["minutos"], s["semana"]["artistas"][0]["artista"],
                                   s["semana"]["canciones"][0]["veces"]]
                datos["pasada"] = s["pasada"]["escuchas"]
                datos["aviso"] = s["aviso"]
                puente.resumenVisto()
                datos["aviso_despues"] = puente.resumen["aviso"]
                ventana.setProperty("pagina", "ajustes")
            paso(2500, empezar)
            paso(800, adelantar)
            paso(2500, radio)
            paso(3000, contar)
            paso(800, lambda: fin(**datos))
        """, "radio")
        self.assertGreaterEqual(d["pidio"], 1)
        self.assertEqual(d["final"], [True, True])
        self.assertEqual(d["sonando"], ["Radio uno", True, False])
        self.assertEqual(d["cola"], ["Dos", "Tres", "Uno", "Radio uno"])
        self.assertGreater(d["escuchado"], 1.5)
        self.assertEqual(d["semana"], [2, 3, "YOASOBI", 2])
        self.assertEqual((d["pasada"], d["aviso"], d["aviso_despues"]), (1, True, False))
        self.assertEqual(d["errores_qml"], [])

    def test_novedades_15(self) -> None:
        # 1.5: tu mezcla del día, seguir lo del cel, lo nuevo de tus artistas, la radio con datos móviles,
        # iniciar radio, compartir y el despertador (sin red: las respuestas de internet se dan a mano)
        d = self.correr("""
            import os, time
            from PySide6.QtGui import QGuiApplication
            datos = {}
            def mezcla():
                rutas = [c["ruta"] for c in puente.canciones]
                puente._fin_mezcla({"fecha": time.strftime("%Y-%m-%d"), "tuyas": rutas[:2], "nuevas": [
                    {"id": "aaaaaaaaaaa", "url": "https://music.youtube.com/watch?v=aaaaaaaaaaa", "titulo": "Nueva",
                     "artistas": "Alguien", "duracion": 30, "portada": ""}]})
                m = puente.mezcla
                datos["mezcla"] = [m["cuantas"], m["nuevas"], m["hoy"]]
                datos["guardada"] = len(puente._leer_json("mezcla.json")["canciones"])
                puente.tocarMezcla()
                datos["origen"] = r.origen
                datos["cola"] = len(r.cola)
            def seguir():
                puente._nube["correo"] = "alguien@ejemplo.com"
                uno = next(c for c in puente.canciones if c["titulo"] == "Uno")
                puente._fin_otro_aparato({"aparato": "cel-1", "nombre": "tu cel", "id": "bbbbbbbbbbb", "titulo": "Del cel",
                                          "artista": "X", "posicion": 42.0, "cuando": time.time() * 1000 - 60000})
                datos["ofrece"] = puente.seguir.get("titulo")
                puente.descartarSeguir()
                puente._fin_otro_aparato({"aparato": puente._aparato(), "id": "ccccccccccc", "cuando": time.time() * 1000})
                datos["propio"] = puente.seguir.get("titulo")      # lo que subió esta PC no se ofrece
                puente._fin_otro_aparato({"aparato": "cel-1", "id": "bbbbbbbbbbb", "cuando": time.time() * 1000 - 120000})
                datos["visto"] = puente.seguir.get("id")           # una más vieja que la descartada tampoco
                puente._nube["correo"] = ""
            def novedades():
                puente._fin_novedades([{"id": "MPREb_1", "titulo": "Hello", "artista": "Ado", "tipo": "Single",
                                        "anio": 2026, "portada": ""}], {"Ado": "UCado"})
                datos["novedades"] = [x["titulo"] for x in puente.novedades]
                datos["vistos"] = puente._leer_json("novedades.json")["vistos"]
                ventana.setProperty("pagina", "agregar")
            def radio_datos():
                os.environ["VORTEX_PLAYER_MEDIDA"] = "1"
                uno = next(c for c in puente.canciones if c["titulo"] == "Uno")
                puente.iniciarRadio(uno)
                datos["radio"] = [r.origen, len(r.cola), all(not c.get("radio") for c in r.cola)]
                del os.environ["VORTEX_PLAYER_MEDIDA"]
                puente.compartir({"ruta": "/m/Idol [ZRtdQ81jPUQ].opus", "titulo": "Idol", "artista": "YOASOBI"}, "copiar")
                datos["enlace"] = QGuiApplication.clipboard().text()
            def despertador():
                puente.ponerDespertador({"activo": True, "hora": time.strftime("%H:%M")})
                datos["despertador"] = puente.ajustes["despertador"]
                puente._revisar_despertador()
                datos["desperto"] = [r.origen, r.pausado, puente._despertado == time.strftime("%Y-%m-%d")]
                puente._revisar_despertador()   # una vez al día
                ventana.setProperty("pagina", "ajustes")
            paso(2500, mezcla)
            paso(800, seguir)
            paso(500, novedades)
            paso(800, radio_datos)
            paso(800, despertador)
            def practicar():
                puente.tocar(0)
                r.ponerVelocidad(0.75)
                r.buscar(1.0)
            def marcar():
                datos["filtros"] = [r._filtros(), r.velocidad]
                r.marcarAB()
            def marcar_b():
                r.marcarAB()
                datos["ab"] = [r.abA >= 0, r.abB > r.abA]
            def repite():
                datos["dentro"] = r.abA - 0.3 <= r.posicion <= r.abB + 0.6
                r.siguiente()
                datos["ab_otra"] = [r.abA, r.abB]
                r.ponerVelocidad(1)
                datos["filtros_normal"] = r._filtros()
            paso(800, practicar)
            paso(1200, marcar)
            paso(1500, marcar_b)
            paso(2500, repite)
            paso(1500, lambda: fin(**datos))
        """, "novedades15")
        self.assertEqual(d["mezcla"], [3, 1, True])
        self.assertEqual((d["guardada"], d["origen"], d["cola"]), (3, "Tu mezcla del día", 3))
        self.assertEqual((d["ofrece"], d["propio"], d["visto"]), ("Del cel", None, None))
        self.assertEqual((d["novedades"], d["vistos"]), (["Hello"], ["MPREb_1"]))
        self.assertEqual(d["radio"], ["Radio de Uno", 3, True])
        self.assertIn("music.youtube.com/watch?v=ZRtdQ81jPUQ", d["enlace"])
        self.assertTrue(d["despertador"]["activo"])
        self.assertEqual(d["desperto"], ["Despertador", False, True])
        self.assertEqual(d["filtros"], ["scaletempo2", 0.75])
        self.assertEqual(d["ab"], [True, True])
        self.assertTrue(d["dentro"])
        self.assertEqual((d["ab_otra"], d["filtros_normal"]), ([-1.0, -1.0], ""))
        self.assertEqual(d["errores_qml"], [])

    def test_compartir_playlist(self) -> None:
        # 1.6: «Compartir» de una playlist copia un enlace de YouTube; las tuyas sin id se buscan (sin red: a mano)
        d = self.correr("""
            from PySide6.QtGui import QGuiApplication
            from vortex_player import descargas
            buscadas = []
            def buscar_falso(texto, fuente="ytmusic", n=20, **_):
                buscadas.append(texto.strip())
                return [] if "Tres" in texto else [{"id": ("id" + texto.strip().split()[-1] + "xxxxxxxx")[:11]}]
            descargas.buscar = buscar_falso
            descargas.recomendar = lambda *a, **k: []
            datos = {}
            def abrir():
                pid = puente.crearPlaylist("Para Ana")
                for c in puente.canciones:
                    puente.agregarAPlaylist(pid, c["ruta"])
                puente.verPlaylist(pid)
            def compartir():
                puente.compartirPlaylist("copiar")
            def ver():
                datos["enlace"] = QGuiApplication.clipboard().text()
                datos["buscadas"] = sorted(buscadas)
                datos["tipo"] = list(descargas.tipo_de(datos["enlace"]) or [])
                fin(**datos)
            paso(2500, abrir)
            paso(800, compartir)
            paso(1500, ver)
        """, "compartir_playlist")
        self.assertEqual(d["buscadas"], ["Dos", "Tres", "Uno"])
        self.assertTrue(d["enlace"].startswith("https://www.youtube.com/watch_videos?video_ids="))
        self.assertTrue(d["enlace"].endswith("&title=Para%20Ana"))
        self.assertEqual(sorted(d["tipo"][2].split(",")), ["idDosxxxxxx", "idUnoxxxxxx"])   # «Tres» no se encontró
        self.assertEqual(d["errores_qml"], [])

if __name__ == "__main__":
    unittest.main()
