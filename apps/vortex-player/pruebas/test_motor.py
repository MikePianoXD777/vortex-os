"""Pruebas del motor de Vortex Player (sin Qt, sin red y sin tocar tu música ni tu audio).

    python3 -m unittest discover -s apps/vortex-player/pruebas

Todo pasa en carpetas temporales: archivos de audio de verdad hechos con ffmpeg, y yt-dlp, pactl,
easyeffects y la página de Spotify de mentira.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI.parent))
REPO = AQUI.parent.parent.parent

from vortex_player import audio, biblioteca, calidad, descargas, ecualizador, letras  # noqa: E402

HAY_FFMPEG = shutil.which("ffmpeg") and shutil.which("ffprobe")


def tono(ruta: Path, *opciones: str, titulo: str = "", artista: str = "") -> Path:
    ruta.parent.mkdir(parents=True, exist_ok=True)
    meta = []
    if titulo:
        meta += ["-metadata", f"title={titulo}"]
    if artista:
        meta += ["-metadata", f"artist={artista}"]
    subprocess.run(["ffmpeg", "-v", "quiet", "-y", "-f", "lavfi", "-i", "sine=frequency=440:duration=1",
                    *opciones, *meta, str(ruta)], check=True)
    return ruta


def ejecutable(ruta: Path, texto: str) -> Path:
    ruta.parent.mkdir(parents=True, exist_ok=True)
    ruta.write_text("#!/usr/bin/env bash\n" + textwrap.dedent(texto))
    ruta.chmod(0o755)
    return ruta


class Temporal(unittest.TestCase):
    def setUp(self) -> None:
        self.t = Path(tempfile.mkdtemp(prefix="vortex-player-"))
        self.entorno = dict(os.environ)
        os.environ.update({"VORTEX_PLAYER_MUSICA": str(self.t / "Música"), "VORTEX_PLAYER_DATOS": str(self.t / "datos"),
                           "VORTEX_PLAYER_CONFIG": str(self.t / "config"), "VORTEX_PLAYER_CACHE": str(self.t / "cache"),
                           "XDG_CONFIG_HOME": str(self.t / "xdg")})

    def tearDown(self) -> None:
        os.environ.clear()
        os.environ.update(self.entorno)
        shutil.rmtree(self.t, ignore_errors=True)


class Calidad(unittest.TestCase):
    def test_etiquetas(self) -> None:
        self.assertEqual(calidad.etiqueta("flac", 24, 96000, 0), "Hi-Res 24/96")
        self.assertEqual(calidad.etiqueta("flac", 24, 44100, 0), "Hi-Res 24/44,1")
        self.assertEqual(calidad.etiqueta("flac", 16, 44100, 0), "CD 16/44,1")
        self.assertEqual(calidad.etiqueta("pcm_s16le", 16, 48000, 1536), "CD 16/48")
        self.assertEqual(calidad.etiqueta("alac", 16, 192000, 0), "Hi-Res 16/192")
        self.assertEqual(calidad.etiqueta("opus", 0, 48000, 160), "Opus 160 kbps")
        self.assertEqual(calidad.etiqueta("aac", 0, 44100, 256), "AAC 256 kbps")
        self.assertEqual(calidad.nivel("pcm_s24le", 24, 96000), "hires")
        self.assertEqual(calidad.nivel("flac", 16, 44100), "cd")
        self.assertEqual(calidad.nivel("mp3", 0, 44100), "comprimida")

    def test_de_ffprobe(self) -> None:
        j = {"streams": [{"codec_type": "audio", "codec_name": "flac", "sample_rate": "96000",
                          "bits_per_raw_sample": "24", "tags": {"TITLE": "Canción"}},
                         {"codec_type": "video", "codec_name": "mjpeg"}],
             "format": {"duration": "201.5", "tags": {"ARTIST": "Alguien", "track": "3/12"}}}
        i = calidad.de_ffprobe("/m/x.flac", j)
        assert i is not None
        self.assertEqual((i.titulo, i.artista, i.pista, i.bits, i.hz, i.portada), ("Canción", "Alguien", 3, 24, 96000, True))
        self.assertEqual(i.etiqueta, "Hi-Res 24/96")
        self.assertIsNone(calidad.de_ffprobe("/m/x.jpg", {"streams": [{"codec_type": "video"}]}))

    @unittest.skipUnless(HAY_FFMPEG, "falta ffmpeg")
    def test_archivos_de_verdad(self) -> None:
        with tempfile.TemporaryDirectory() as d:
            hires = calidad.leer(tono(Path(d) / "a.flac", "-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24"))
            cd = calidad.leer(tono(Path(d) / "b.wav", "-ar", "44100", "-c:a", "pcm_s16le"))
            opus = calidad.leer(tono(Path(d) / "c.opus", "-c:a", "libopus", "-b:a", "160k"))
            self.assertEqual(hires.etiqueta, "Hi-Res 24/96")
            self.assertEqual(cd.etiqueta, "CD 16/44,1")
            self.assertTrue(opus.etiqueta.startswith("Opus"), opus.etiqueta)
            self.assertEqual(opus.nivel, "comprimida")
            self.assertIsNone(calidad.leer(Path(d) / "no-existe.flac"))


@unittest.skipUnless(HAY_FFMPEG, "falta ffmpeg")
class Biblioteca(Temporal):
    def test_escanear_y_playlists(self) -> None:
        m = self.t / "Música"
        a = tono(m / "Álbum" / "uno.flac", "-ar", "96000", "-sample_fmt", "s32", "-bits_per_raw_sample", "24",
                 titulo="Uno", artista="Banda")
        b = tono(m / "Vortex Player" / "Banda" / "Dos [abc].opus", "-c:a", "libopus", titulo="Dos", artista="Banda")
        (m / ".oculta").mkdir(parents=True)
        tono(m / ".oculta" / "x.flac")
        (m / "notas.txt").write_text("no es audio")
        bib = biblioteca.Biblioteca()
        r = bib.escanear()
        self.assertEqual((r["nuevas"], r["total"]), (2, 2))
        canciones = {c["titulo"]: c for c in bib.canciones()}
        self.assertEqual(canciones["Uno"]["etiqueta"], "Hi-Res 24/96")
        self.assertEqual(canciones["Uno"]["origen"], "local")
        self.assertEqual(canciones["Dos"]["origen"], "youtube")
        self.assertEqual([c["titulo"] for c in bib.canciones(orden="calidad")], ["Uno", "Dos"])
        self.assertEqual([c["titulo"] for c in bib.canciones("dos")], ["Dos"])
        self.assertEqual(bib.escanear()["nuevas"], 0)   # la segunda vez no vuelve a leer nada

        pid = bib.crear_playlist("Para correr", url="https://open.spotify.com/playlist/X")
        self.assertEqual(bib.crear_playlist("Otra vez", url="https://open.spotify.com/playlist/X"), pid)
        bib.agregar_a_playlist(pid, str(b))
        bib.agregar_a_playlist(pid, str(a))
        bib.agregar_a_playlist(pid, str(a))
        self.assertEqual([c["titulo"] for c in bib.canciones_de_playlist(pid)], ["Dos", "Uno"])
        self.assertEqual(bib.playlists()[0]["canciones"], 2)
        bib.quitar_de_playlist(pid, str(b))
        self.assertEqual([c["titulo"] for c in bib.canciones_de_playlist(pid)], ["Uno"])

        a.unlink()
        self.assertEqual(bib.escanear()["borradas"], 1)
        self.assertEqual([c["titulo"] for c in bib.canciones()], ["Dos"])
        bib.borrar_playlist(pid)
        self.assertEqual(bib.playlists(), [])
        bib.cerrar()

    def test_ajustes_y_otras_carpetas(self) -> None:
        otra = self.t / "Disco externo"
        tono(otra / "tres.wav", "-c:a", "pcm_s24le", "-ar", "48000")
        biblioteca.Biblioteca.guardar_ajuste("carpetas", [str(otra)])
        self.assertEqual(biblioteca.Biblioteca.ajustes()["volumen"], 80)
        bib = biblioteca.Biblioteca()
        bib.escanear()
        self.assertEqual(bib.canciones()[0]["etiqueta"], "Hi-Res 24/48")
        bib.cerrar()


PAGINA_SPOTIFY = """<html><script id="__NEXT_DATA__" type="application/json">%s</script></html>""" % json.dumps(
    {"props": {"pageProps": {"state": {"data": {"entity": {
        "type": "playlist", "name": "Mis favoritas",
        "trackList": [{"title": "Kick Back", "subtitle": "Kenshi Yonezu", "duration": 193000},
                      {"title": "Idol", "subtitle": "YOASOBI, Otro", "duration": 213000},
                      {"title": "", "subtitle": "nadie"}]}}}}}})


class Descargas(Temporal):
    def test_tipo_de(self) -> None:
        self.assertEqual(descargas.tipo_de("https://open.spotify.com/playlist/37i9dQZF1DX?si=1"),
                         ("spotify", "playlist", "37i9dQZF1DX"))
        self.assertEqual(descargas.tipo_de("https://open.spotify.com/intl-es/track/abc123"), ("spotify", "track", "abc123"))
        self.assertEqual(descargas.tipo_de("spotify:album:XYZ"), ("spotify", "album", "XYZ"))
        self.assertEqual(descargas.tipo_de("https://music.youtube.com/watch?v=dQw4&list=RD1"), ("youtube", "cancion", "dQw4"))
        self.assertEqual(descargas.tipo_de("https://youtu.be/dQw4"), ("youtube", "cancion", "dQw4"))
        self.assertEqual(descargas.tipo_de("music.youtube.com/playlist?list=PL9"), ("youtube", "playlist", "PL9"))
        self.assertEqual(descargas.tipo_de("https://music.youtube.com/browse/MPREb_x"), ("youtube", "playlist", "MPREb_x"))
        self.assertIsNone(descargas.tipo_de("https://example.com/cancion.mp3"))
        self.assertIsNone(descargas.tipo_de("hola"))

    def test_spotify(self) -> None:
        nombre, pistas = descargas.leer_spotify(PAGINA_SPOTIFY)
        self.assertEqual(nombre, "Mis favoritas")
        self.assertEqual(pistas, [{"titulo": "Kick Back", "artistas": "Kenshi Yonezu", "duracion": 193},
                                  {"titulo": "Idol", "artistas": "YOASOBI, Otro", "duracion": 213}])
        self.assertEqual(descargas.busqueda_ytmusic(pistas[1]), "https://music.youtube.com/search?q=YOASOBI%20Idol#songs")
        with self.assertRaises(ValueError):
            descargas.leer_spotify("<html></html>")

    def test_comando_sin_recomprimir(self) -> None:
        c = descargas.comando("https://youtu.be/x", Path("/m"), navegador="firefox", una=True)
        self.assertIn("-x", c)
        self.assertNotIn("--audio-format", c)
        self.assertEqual(c[c.index("--cookies-from-browser") + 1], "firefox")
        self.assertIn("--no-playlist", c)
        self.assertEqual(c[-2:], ["--", "https://youtu.be/x"])
        self.assertNotIn("--cookies-from-browser", descargas.comando("u", Path("/m")))
        self.assertIn("--playlist-items", descargas.comando("https://music.youtube.com/search?q=a#songs", Path("/m")))

    def test_leer_linea(self) -> None:
        self.assertEqual(descargas.leer_linea("VORTEXA\t/m/a b.opus\n"), {"tipo": "archivo", "ruta": "/m/a b.opus"})
        self.assertEqual(descargas.leer_linea("VORTEXP\t 42.5%\tIdol"), {"tipo": "progreso", "porcentaje": 42.5, "titulo": "Idol"})
        self.assertIn("privada", descargas.leer_linea("ERROR: [youtube] x: Private video")["texto"])
        self.assertIsNone(descargas.leer_linea("[download] Destination: x"))

    def _yt_dlp_falso(self) -> Path:
        # Baja «algo» por cada búsqueda o URL; con «falla» en la búsqueda no encuentra nada
        return ejecutable(self.t / "bin" / "yt-dlp", f"""
            echo "$@" >>"{self.t}/llamadas"
            obj="${{@: -1}}"
            if [[ " $* " == *" -J "* ]]; then echo '{{"title": "Album - Lo mejor", "entries": [{{}}, {{}}]}}'; exit 0; fi
            [[ "$obj" == *falla* ]] && {{ echo "ERROR: [youtube] x: Video unavailable"; exit 1; }}
            printf 'VORTEXP\\t 50.0%%\\tCancion\\n'
            n=$(wc -l <"{self.t}/llamadas")
            printf 'VORTEXA\\t/m/cancion%s.opus\\n' "$n"
            [[ "$*" == *--yes-playlist* && "$*" != *--playlist-items* ]] && printf 'VORTEXA\\t/m/otra.opus\\n'
            exit 0
        """)

    def test_descargar_spotify(self) -> None:
        yt = self._yt_dlp_falso()
        pagina = PAGINA_SPOTIFY.replace("Idol", "Idol falla")
        evs = list(descargas.descargar("https://open.spotify.com/playlist/AAA", self.t / "dest", yt_dlp=str(yt),
                                       bajar=lambda url: pagina))
        self.assertEqual(evs[0], {"tipo": "inicio", "nombre": "Mis favoritas", "total": 2, "es_lista": True})
        self.assertEqual([e["ruta"] for e in evs if e["tipo"] == "archivo"], ["/m/cancion1.opus"])
        self.assertTrue(any("No encontré «Idol falla»" in e.get("texto", "") for e in evs))
        self.assertEqual(evs[-1], {"tipo": "fin", "bajadas": 1, "fallidas": 1})
        llamadas = (self.t / "llamadas").read_text()
        self.assertIn("music.youtube.com/search?q=Kenshi%20Yonezu%20Kick%20Back#songs", llamadas)

    def test_descargar_youtube(self) -> None:
        yt = self._yt_dlp_falso()
        evs = list(descargas.descargar("https://music.youtube.com/playlist?list=PL1", self.t / "dest", yt_dlp=str(yt)))
        self.assertEqual(evs[0], {"tipo": "inicio", "nombre": "Lo mejor", "total": 2, "es_lista": True})
        self.assertEqual([e["i"] for e in evs if e["tipo"] == "archivo"], [0, 1])
        self.assertEqual(evs[-1]["bajadas"], 2)
        una = list(descargas.descargar("https://youtu.be/abc", self.t / "dest", yt_dlp=str(yt)))
        self.assertEqual(una[0]["es_lista"], False)
        self.assertEqual(una[-1], {"tipo": "fin", "bajadas": 1, "fallidas": 0})

    def test_sin_yt_dlp_y_enlace_raro(self) -> None:
        evs = list(descargas.descargar("https://youtu.be/abc", self.t / "d", yt_dlp=str(self.t / "no-existe")))
        self.assertTrue(any(e.get("fatal") and "pacman -S yt-dlp" in e["texto"] for e in evs))
        evs = list(descargas.descargar("https://example.com", self.t / "d"))
        self.assertTrue(evs[0]["fatal"])
        self.assertEqual(evs[-1]["tipo"], "fin")


def _fila_ytm(vid: str, titulo: str, segunda: str) -> dict:
    """Una canción como la manda la búsqueda de YouTube Music."""
    col = lambda t: {"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": x} for x in t]}}}
    return {"musicResponsiveListItemRenderer": {
        "playlistItemData": {"videoId": vid}, "flexColumns": [col([titulo]), col(segunda.split("|"))],
        "thumbnail": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [{"url": "https://x/a=w60-h60-l90"}]}}}}}


def _fila_radio(vid: str, titulo: str, linea: str, dur: str) -> dict:
    """Una canción como la manda la radio («A continuación») de YouTube Music."""
    return {"playlistPanelVideoRenderer": {"videoId": vid, "title": {"runs": [{"text": titulo}]},
                                           "longBylineText": {"runs": [{"text": linea}]},
                                           "lengthText": {"runs": [{"text": dur}]},
                                           "thumbnail": {"thumbnails": [{"url": "https://x/b=w60-h60"}]}}}


class Buscar(unittest.TestCase):
    """2.1: buscar en YouTube Music / YouTube y recomendaciones, con respuestas falsas (sin internet)."""

    def test_leer_busqueda_de_ytmusic(self) -> None:
        r = {"contents": [_fila_ytm("aaaaaaaaaaa", "Idol", "YOASOBI| • |Idol| • |3:34"),
                          _fila_ytm("bbbbbbbbbbb", "Sin álbum", "Canción| • |Alguien| • |1:02:03")]}
        c = descargas.leer_ytmusic(r)
        self.assertEqual([(x["titulo"], x["artistas"], x["album"], x["duracion"]) for x in c],
                         [("Idol", "YOASOBI", "Idol", 214), ("Sin álbum", "Alguien", "", 3723)])
        self.assertEqual(c[0]["portada"], "https://x/a=w226-h226-l90")
        self.assertEqual(c[0]["url"], "https://music.youtube.com/watch?v=aaaaaaaaaaa")

    def test_buscar_cae_a_yt_dlp(self) -> None:
        # YouTube Music contestó algo que no se entiende: se usa la búsqueda de yt-dlp
        with tempfile.TemporaryDirectory() as t:
            falso = Path(t) / "yt-dlp"
            falso.write_text("#!/bin/sh\necho '{\"entries\": [{\"id\": \"ccccccccccc\", \"title\": \"T\", \"channel\": \"C\", \"duration\": 61}, {\"id\": \"UCcanal\"}]}'\n")
            falso.chmod(0o755)
            c = descargas.buscar("algo", "ytmusic", yt_dlp=str(falso), pedir=lambda u, b: {})
            self.assertEqual([(x["id"], x["titulo"], x["artistas"], x["duracion"]) for x in c], [("ccccccccccc", "T", "C", 61)])
            c = descargas.buscar("algo", "youtube", yt_dlp=str(falso), pedir=lambda u, b: {})
            self.assertTrue(c[0]["url"].startswith("https://www.youtube.com/watch?v="))
        self.assertEqual(descargas.buscar("   ", "ytmusic"), [])

    def test_recomendar(self) -> None:
        radios = {
            "semilla0001": [_fila_radio("semilla0001", "Idol", "YOASOBI • Idol • 2023", "3:34"),
                            _fila_radio("r1111111111", "Monster", "YOASOBI • THE BOOK 2 • 2021", "3:26"),
                            _fila_radio("r2222222222", "Ya la tengo (Live)", "LiSA • X", "4:00"),
                            _fila_radio("r3333333333", "Comun", "Alguien • Y", "2:00")],
            "semilla0002": [_fila_radio("r4444444444", "Gurenge", "LiSA • LEO-NiNE • 2020", "3:58"),
                            _fila_radio("r3333333333", "Comun", "Alguien • Y", "2:00")],
        }
        pedidos = []

        def pedir(url: str, cuerpo: dict) -> dict:
            pedidos.append(url.split("/")[-1].split("?")[0])
            if "search" in url:   # la semilla sin id (un archivo tuyo) se busca primero
                return {"x": [_fila_ytm("semilla0002", "Homura", "LiSA| • |homura| • |4:35")]}
            return {"contents": [radios[cuerpo["videoId"]]]}

        c = descargas.recomendar([{"id": "semilla0001", "titulo": "Idol", "artista": "YOASOBI"},
                                  {"titulo": "Homura", "artista": "LiSA"}],
                                 [{"titulo": "Ya la tengo", "artista": "LiSA"}], pedir=pedir)
        # Intercaladas, sin la semilla, sin repetir y sin lo que ya tienes (aunque diga «(Live)»)
        self.assertEqual([x["titulo"] for x in c], ["Monster", "Gurenge", "Comun"])
        self.assertEqual((c[0]["artistas"], c[0]["album"], c[0]["duracion"]), ("YOASOBI", "THE BOOK 2", 206))
        self.assertEqual(pedidos, ["search", "next", "next"])
        self.assertEqual(descargas.recomendar([], []), [])


class Audio(Temporal):
    def _pactl(self, bt: bool) -> str:
        sinks = [{"name": "alsa_output.pci", "description": "Audio Interno", "sample_specification": "s32le 2ch 48000Hz",
                  "properties": {"device.api": "alsa", "device.description": "Audio Interno"}},
                 {"name": "bluez_output.AA.1", "description": "SOUNDPEATS Air6 HS", "sample_specification": "s32le 2ch 96000Hz",
                  "properties": {"device.api": "bluez5", "api.bluez5.codec": "aac", "device.name": "bluez_card.AA",
                                 "device.description": "SOUNDPEATS Air6 HS"}}]
        cards = [{"name": "bluez_card.AA", "profiles": {"a2dp-sink-aac": {"available": True},
                                                         "a2dp-sink-ldac": {"available": True}, "off": {}}}]
        (self.t / "sinks.json").write_text(json.dumps(sinks))
        (self.t / "cards.json").write_text(json.dumps(cards))
        return str(ejecutable(self.t / "bin" / "pactl", f"""
            case "$*" in
              get-default-sink) echo {"bluez_output.AA.1" if bt else "alsa_output.pci"} ;;
              "-f json list sinks") cat "{self.t}/sinks.json" ;;
              "-f json list cards") cat "{self.t}/cards.json" ;;
              "set-card-profile bluez_card.AA a2dp-sink-ldac") echo ok >"{self.t}/ldac" ;;
              *) exit 1 ;;
            esac
        """))

    def test_salida_bluetooth(self) -> None:
        s = audio.salida(self._pactl(True))
        self.assertEqual((s["nombre"], s["bluetooth"], s["codecTexto"], s["hz"], s["ldac"]),
                         ("SOUNDPEATS Air6 HS", True, "AAC", 96000, True))
        self.assertTrue(audio.forzar_ldac("bluez_card.AA", self._pactl(True)))
        self.assertTrue((self.t / "ldac").exists())
        self.assertFalse(audio.forzar_ldac("bluez_card.ZZ", self._pactl(True)))

    def test_salida_normal_y_sin_pipewire(self) -> None:
        s = audio.salida(self._pactl(False))
        self.assertEqual((s["bluetooth"], s["codec"], s["ldac"]), (False, "", False))
        self.assertEqual(audio.salida(str(self.t / "no-existe")), {})

    def test_configs(self) -> None:
        self.assertTrue(audio.config_hires())
        self.assertFalse(audio.config_hires())   # la segunda vez no cambia nada
        self.assertIn("96000", audio.archivo_hires().read_text())
        self.assertTrue(audio.config_ldac(True))
        self.assertIn('ldac.quality = "hq"', audio.archivo_ldac().read_text())
        self.assertTrue(audio.config_ldac(False))
        self.assertFalse(audio.archivo_ldac().exists())
        self.assertFalse(audio.config_ldac(False))


class Ecualizador(Temporal):
    def test_bandas(self) -> None:
        viejo = {"b1": "5", "b2": "7", "b10": "2", "preset": "Bass"}
        b = ecualizador.bandas_de(viejo)
        self.assertEqual((len(b), b[0], b[1]["g"], b[9]["g"], b[4]["g"]), (10, {"f": 31.0, "g": 5.0}, 7.0, 2.0, 0.0))
        nuevo = {"bandas": [{"f": 50, "g": 3}, {"f": 9000, "g": -20}]}
        self.assertEqual(ecualizador.texto_bandas(ecualizador.bandas_de(nuevo)), "50:3,9000:-12")
        self.assertFalse(ecualizador.poner_bandas([{"f": 100, "g": 0}]))   # una sola banda no

    @unittest.skipUnless(shutil.which("jq"), "falta jq")
    def test_mismo_estado_que_el_panel(self) -> None:
        home = self.t / "home"
        bin_ = self.t / "bin"
        for n in ("easyeffects", "systemctl"):
            ejecutable(bin_ / n, "exit 0\n")
        ejecutable(bin_ / "pgrep", "exit 0\n")
        os.environ.update({"HOME": str(home), "SERPANTINUM_DIR": str(REPO / "shell/src"),
                           "PATH": f"{bin_}:{os.environ['PATH']}", "XDG_DATA_HOME": str(home / ".local/share"),
                           "XDG_RUNTIME_DIR": str(self.t / "run"), "VORTEX_PLAYER_EQ": str(home / ".local/state/serpantinum/music/eq_state.json")})
        self.assertIsNotNone(ecualizador.script())
        bandas = [{"f": 40, "g": 4}, {"f": 1000, "g": -2.5}, {"f": 12000, "g": 6}]
        self.assertTrue(ecualizador.poner_bandas(bandas))
        e = ecualizador.estado()
        self.assertEqual(e["bandas"], [{"f": 40.0, "g": 4.0}, {"f": 1000.0, "g": -2.5}, {"f": 12000.0, "g": 6.0}])
        self.assertEqual((e["preset"], e["pending"]), ("Custom", False))
        self.assertTrue(ecualizador.poner_preset("Rock"))
        self.assertEqual(ecualizador.estado()["preset"], "Rock")
        self.assertEqual(len(ecualizador.estado()["bandas"]), 10)
        self.assertFalse(ecualizador.poner_preset("Inventado"))


class Perfiles(unittest.TestCase):
    def test_perfiles_en_tus_bandas(self) -> None:
        diez = [{"f": f, "g": 0.0} for f in ecualizador.FRECUENCIAS]
        self.assertGreaterEqual(len(ecualizador.PERFILES), 15)
        for nombre, valores in ecualizador.PERFILES.items():
            self.assertEqual(len(valores), 10, nombre)
            self.assertEqual([b["g"] for b in ecualizador.bandas_de_perfil(nombre, diez)], [float(v) for v in valores])
            self.assertEqual(ecualizador.perfil_de(ecualizador.bandas_de_perfil(nombre, diez)), nombre)
        # en bandas que no coinciden se interpola en escala logarítmica (a la mitad entre 62 y 125 Hz)
        self.assertAlmostEqual(ecualizador.db_en(ecualizador.PERFILES["Graves"], (62 * 125) ** 0.5), 5.5, places=1)
        self.assertEqual(ecualizador.db_en(ecualizador.PERFILES["Agudos"], 20000), 7)
        tres = ecualizador.bandas_de_perfil("Rock", [{"f": 60, "g": 0}, {"f": 1000, "g": 0}, {"f": 12000, "g": 0}])
        self.assertEqual(ecualizador.perfil_de(tres), "Rock")
        self.assertEqual(ecualizador.perfil_de([{"f": 60, "g": 1}, {"f": 1000, "g": 9}]), "Personalizado")


class Letras(Temporal):
    LRC = "[00:01.00] Uno\n[00:02.50][00:06.00] Coro\n[00:04.2] Dos\n"

    def test_limpiar_y_leer(self) -> None:
        self.assertEqual(letras.limpiar("YOASOBI - アイドル (Official Music Video)", "YOASOBI"), "アイドル")
        self.assertEqual(letras.limpiar("Blinding Lights (Official Audio) ft. Nadie", "The Weeknd"), "Blinding Lights")
        self.assertEqual(letras.limpiar_artista("Ado, Vaundy & otros"), "Ado")
        self.assertEqual(letras.limpiar_artista("Aimer - Topic"), "Aimer")
        lineas = letras.leer_lrc(self.LRC)
        self.assertEqual([(l["ms"], l["texto"]) for l in lineas],
                         [(1000, "Uno"), (2500, "Coro"), (4200, "Dos"), (6000, "Coro")])
        self.assertEqual([letras.actual(lineas, ms) for ms in (0, 800, 2400, 5000, 9000)], [-1, 0, 1, 2, 3])

    def test_elegir(self) -> None:
        r = [{"duration": 300, "syncedLyrics": "[00:01.00] lejos"},
             {"duration": 201, "plainLyrics": "plana"},
             {"duration": 199, "syncedLyrics": self.LRC}]
        self.assertEqual(letras.elegir(r, 200)["duration"], 199)       # la que dura lo mismo y viene sincronizada
        self.assertEqual(letras.elegir(r[:2], 200)["duration"], 201)   # si no, la plana
        self.assertIsNone(letras.elegir(r[:1], 200))
        self.assertTrue(letras.de_json(r[2])["sincronizada"])

    def test_de_guarda_y_respaldo(self) -> None:
        pedidos: list[str] = []
        respuesta: dict = {"r": {"duration": 200, "syncedLyrics": self.LRC}}

        def pedir(url: str):
            pedidos.append(url)
            r = respuesta["r"]
            if isinstance(r, Exception):
                raise r
            return r if "/get?" in url else ([r] if r else [])

        viejo = letras.pedir
        letras.pedir = pedir
        try:
            d = self.t / "letras"
            l = letras.de(d, "Canción (Official Video)", "Alguien", 200)
            self.assertTrue(l["sincronizada"])
            self.assertIn("track_name=Canci%C3%B3n", pedidos[0])
            n = len(pedidos)
            self.assertEqual(letras.de(d, "Canción (Official Video)", "Alguien", 200), l)   # la segunda, sin internet
            self.assertEqual(len(pedidos), n)
            # sin LRCLIB: la de YouTube Music (sin tiempos)
            respuesta["r"] = None
            l = letras.de(d, "Otra", "Alguien", 180, "vid123", lambda v: "línea 1\nlínea 2" if v == "vid123" else None)
            self.assertEqual((l["sincronizada"], [x["texto"] for x in l["lineas"]]), (False, ["línea 1", "línea 2"]))
            # no hay en ningún lado: se anota y no se vuelve a buscar
            self.assertIsNone(letras.de(d, "Nada", "Nadie", 100))
            n = len(pedidos)
            self.assertIsNone(letras.de(d, "Nada", "Nadie", 100))
            self.assertEqual(len(pedidos), n)
            # sin internet: OSError y no se anota «nada»
            respuesta["r"] = OSError("sin red")
            with self.assertRaises(OSError):
                letras.de(d, "Sin red", "Nadie", 100)
            respuesta["r"] = {"duration": 100, "plainLyrics": "ya hay"}
            self.assertEqual(letras.de(d, "Sin red", "Nadie", 100)["lineas"][0]["texto"], "ya hay")
        finally:
            letras.pedir = viejo


class Novedades26(Temporal):
    @unittest.skipUnless(HAY_FFMPEG, "falta ffmpeg")
    def test_me_gusta_y_titulos_limpios(self) -> None:
        m = self.t / "Música"
        a = tono(m / "uno.flac", titulo="Uno", artista="Banda")
        b = tono(m / "Vortex Player" / "YOASOBI - アイドル (Official Music Video) [ZRtdQ81jPUQ].opus", "-c:a", "libopus",
                 titulo="YOASOBI - アイドル (Official Music Video)", artista="YOASOBI - Topic")
        bib = biblioteca.Biblioteca()
        bib.escanear()
        self.assertEqual(sorted((c["titulo"], c["artista"]) for c in bib.canciones()), [("Uno", "Banda"), ("アイドル", "YOASOBI")])
        self.assertEqual(bib.id_me_gusta(), -1)
        otra = bib.crear_playlist("Otra")
        self.assertTrue(bib.alternar_me_gusta(str(a)))
        self.assertTrue(bib.alternar_me_gusta(str(b)))
        self.assertEqual(bib.me_gusta(), [str(b), str(a)])   # la última, hasta arriba
        self.assertEqual([(p["nombre"], p["meGusta"]) for p in bib.playlists()], [("Tus me gusta", True), ("Otra", False)])
        self.assertFalse(bib.alternar_me_gusta(str(a)))
        self.assertEqual(bib.me_gusta(), [str(b)])
        # en la nube va con el mismo id que en el teléfono
        self.assertIn(biblioteca.ME_GUSTA, bib.estado_nube().playlists)
        self.assertNotEqual(otra, bib.id_me_gusta())

    def test_titulos_y_recomendadas_sin_repetir(self) -> None:
        t = letras.titulo_limpio
        self.assertEqual(t("YOASOBI「アイドル」Official Music Video", "YOASOBI"), "アイドル")
        self.assertEqual(t("Kick Back - Official Video", "米津玄師"), "Kick Back")
        self.assertEqual(t("Bad Apple!! feat.nomico", "Alstroemeria Records"), "Bad Apple!!")
        self.assertEqual(t("Song Name | Lyric Video", "X"), "Song Name")
        self.assertEqual(t("Video", "X"), "Video")   # no se queda vacío
        self.assertEqual(letras.artista_limpio("AdoVEVO"), "Ado")
        self.assertEqual(letras.artista_limpio("Ado, Vaundy"), "Ado, Vaundy")
        # la misma canción subida como video y como canción de YouTube Music ya no se recomienda dos veces
        self.assertEqual(descargas._clave("YOASOBI - アイドル (Official Music Video)", "YOASOBI - Topic"),
                         descargas._clave("アイドル", "YOASOBI"))
        self.assertTrue(letras.guardada(self.t, "x", "y", 1) is False)


class EcualizadorEnMpv(Temporal):
    """2.5 (Windows): sin EasyEffects, el ecualizador se guarda aparte y suena con filtros de mpv."""

    def setUp(self) -> None:
        super().setUp()   # Temporal deja el entorno como estaba al terminar
        os.environ.update({"VORTEX_PLAYER_EQ_MPV": "1", "VORTEX_PLAYER_CONFIG": str(self.t)})
        os.environ.pop("VORTEX_PLAYER_EQ", None)

    def test_guarda_aparte_y_arma_el_filtro(self):
        self.assertTrue(ecualizador.en_mpv() and ecualizador.disponible())
        self.assertEqual(ecualizador.archivo_estado(), self.t / "ecualizador.json")
        self.assertEqual(ecualizador.filtro_mpv(ecualizador.estado()["bandas"]), "")   # todo en 0: sin filtros
        self.assertTrue(ecualizador.poner_preset("Bass"))
        self.assertEqual(ecualizador.estado()["preset"], "Bass")
        self.assertTrue(ecualizador.poner_bandas([{"f": 31, "g": 3}, {"f": 1000, "g": -2}]))
        self.assertEqual(ecualizador.filtro_mpv(ecualizador.estado()["bandas"]),
                         "lavfi=[volume=volume=-3dB,equalizer=f=31:t=o:w=1:g=3,equalizer=f=1000:t=o:w=1:g=-2]")

    @unittest.skipUnless(shutil.which("mpv"), "sin mpv")
    def test_mpv_acepta_el_filtro_con_el_crossfade(self):
        tono = self.t / "tono.ogg"
        subprocess.run(["ffmpeg", "-v", "quiet", "-y", "-f", "lavfi", "-i", "sine=f=440:d=2", str(tono)], check=True)
        f = ecualizador.filtro_mpv([{"f": 63, "g": 6}, {"f": 8000, "g": -3}])
        r = subprocess.run(["mpv", "--no-config", "--ao=null", "--no-video", "--length=0.5",
                            f"--af={f},lavfi=[afade=t=in:d=1]", str(tono)], capture_output=True, text=True, timeout=20)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)



class Novedades14(Temporal):
    """1.4: la radio sin fin y «Tu resumen»."""

    def test_semana_e_id_de_la_radio(self) -> None:
        import datetime as dt
        jueves = dt.datetime(2026, 10, 29, 15, 30).timestamp()
        desde, hasta = biblioteca.semana(0, jueves)
        self.assertEqual(dt.datetime.fromtimestamp(desde), dt.datetime(2026, 10, 26))
        self.assertEqual(dt.datetime.fromtimestamp(hasta), dt.datetime(2026, 11, 2))
        self.assertEqual(dt.datetime.fromtimestamp(biblioteca.semana(1, jueves)[0]), dt.datetime(2026, 10, 19))
        self.assertEqual(biblioteca.id_de("https://music.youtube.com/watch?v=ZRtdQ81jPUQ"), "ZRtdQ81jPUQ")
        self.assertEqual(biblioteca.id_de("/m/Idol [ZRtdQ81jPUQ].opus"), "ZRtdQ81jPUQ")
        self.assertEqual(biblioteca.id_de("/m/otra.flac"), "")

    def test_resumen(self) -> None:
        b = biblioteca.Biblioteca(self.t / "b.db")
        desde, hasta = biblioteca.semana(0)
        for titulo, artista, seg, dia in (("Idol", "YOASOBI - Topic", 200, 0), ("Idol", "YOASOBI", 100, 1),
                                          ("Racing", "Ado, Otro", 60, 1), ("Fuera", "Nadie", 999, 8)):
            b.anotar_escucha({"ruta": f"/m/{titulo}.opus", "titulo": titulo, "artista": artista}, seg,
                             cuando=desde + dia * 86400 + 60)
        r = b.resumen(desde, hasta)
        self.assertEqual((r["escuchas"], r["minutos"], r["distintas"], r["dias"]), (3, 6, 2, 2))
        self.assertEqual([(c["titulo"], c["veces"]) for c in r["canciones"]], [("Idol", 2), ("Racing", 1)])
        self.assertEqual([(a["artista"], a["minutos"]) for a in r["artistas"]], [("YOASOBI", 5), ("Ado", 1)])
        self.assertEqual(b.resumen(hasta, hasta + 1)["escuchas"], 0)
        b.cerrar()


class Novedades15(Temporal):
    """1.5: tu mezcla del día, lo nuevo de tus artistas y la radio con datos móviles."""

    def test_mas_escuchadas_y_artistas(self) -> None:
        b = biblioteca.Biblioteca(self.t / "b.db")
        for ruta, titulo, artista in (("/m/a.opus", "A", "Ado"), ("/m/b.opus", "B", "YOASOBI - Topic"), ("/m/c.opus", "C", "Ado")):
            b.db.execute("INSERT INTO canciones (ruta, titulo, artista, album, duracion, agregada) VALUES (?,?,?,?,?,?)",
                         (ruta, titulo, artista, "", 200, 0))
        b.db.commit()
        self.assertEqual(b.artistas_mas_escuchados(0), ["Ado", "YOASOBI"])   # sin escuchas: lo que más tienes
        ahora = 1_800_000_000
        for ruta, titulo, artista, veces in (("/m/b.opus", "B", "YOASOBI - Topic", 3), ("/m/a.opus", "A", "Ado", 1),
                                             ("/m/borrada.opus", "X", "Nadie", 5)):
            for _ in range(veces):
                b.anotar_escucha({"ruta": ruta, "titulo": titulo, "artista": artista}, 60, cuando=ahora)
        self.assertEqual([c["titulo"] for c in b.mas_escuchadas(ahora - 10)], ["B", "A"])   # la borrada no
        self.assertEqual(b.mas_escuchadas(ahora + 10), [])
        self.assertEqual(b.artistas_mas_escuchados(ahora - 10, 2), ["Nadie", "YOASOBI"])
        b.cerrar()

    def test_lanzamientos_de_youtube_music(self) -> None:
        def item(bid, titulo, sub):
            return {"musicTwoRowItemRenderer": {
                "title": {"runs": [{"text": titulo}]}, "subtitle": {"runs": [{"text": sub}]},
                "navigationEndpoint": {"browseEndpoint": {"browseId": bid}},
                "thumbnailRenderer": {"musicThumbnailRenderer": {"thumbnail": {"thumbnails": [
                    {"url": "https://lh3.googleusercontent.com/x=w120-h120-l90"}]}}}}}
        pedidos = []

        def pedir(url, cuerpo):
            pedidos.append(cuerpo.get("browseId") or cuerpo.get("query"))
            if "search" in url:
                return {"contents": [{"musicResponsiveListItemRenderer": {
                    "navigationEndpoint": {"browseEndpoint": {"browseId": "UCado"}}}}]}
            if cuerpo["browseId"] == "UCado":
                return {"contents": [{"musicCarouselShelfRenderer": {"contents": [
                    item("MPREb_viejo", "Kyougen", "Álbum • 2022"), item("MPREb_nuevo", "Hello", "Single • 2026"),
                    item("MPREb_ep", "Ep", "EP • 2025"), item("VLPLotra", "Una playlist", "Playlist")]}}]}
            return {"playlistId": "OLAK5uy_abc", "contents": [{"musicResponsiveListItemRenderer": {
                "playlistItemData": {"videoId": "aaaaaaaaaaa"},
                "flexColumns": [{"musicResponsiveListItemFlexColumnRenderer": {"text": {"runs": [{"text": "Hello"}]}}}]}}]}

        self.assertEqual(descargas.artista_id("Ado", pedir), "UCado")
        l = descargas.lanzamientos("UCado", "Ado", pedir)
        self.assertEqual([(x["id"], x["tipo"], x["anio"]) for x in l],
                         [("MPREb_nuevo", "Single", 2026), ("MPREb_ep", "EP", 2025), ("MPREb_viejo", "Álbum", 2022)])
        self.assertTrue(l[0]["portada"].endswith("=w320-h320-l90"))
        self.assertEqual(descargas.canciones_de_lanzamiento("MPREb_nuevo", pedir),
                         ("OLAK5uy_abc", [{"id": "aaaaaaaaaaa", "titulo": "Hello"}]))

    def test_radio_con_datos_moviles(self) -> None:
        from vortex_player.puente import Puente
        todas = [{"ruta": f"/m/{i}", "artista": a} for i, a in enumerate(["Ado", "Otro", "Ado - Topic", "Más", "Ado"])]
        r = Puente._radio_local([{"artista": "Ado"}], {"/m/0"}, todas, n=3)
        self.assertEqual(sorted(c["ruta"] for c in r[:2]), ["/m/2", "/m/4"])   # primero las del mismo artista
        self.assertEqual(len(r), 3)
        self.assertNotIn("/m/0", [c["ruta"] for c in r])                        # sin las que ya están en la cola
        os.environ["VORTEX_PLAYER_MEDIDA"] = "1"
        try:
            self.assertTrue(Puente._con_datos_medidos())
        finally:
            del os.environ["VORTEX_PLAYER_MEDIDA"]

if __name__ == "__main__":
    unittest.main()
