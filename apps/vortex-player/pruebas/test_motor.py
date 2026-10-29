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

from vortex_player import audio, biblioteca, calidad, descargas, ecualizador  # noqa: E402

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


if __name__ == "__main__":
    unittest.main()
