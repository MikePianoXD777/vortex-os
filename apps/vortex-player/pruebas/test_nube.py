"""Pruebas de la nube de Vortex Player (2.4): mezclar, el plan, la biblioteca, sincronizar dos aparatos con
un Drive de mentira e iniciar sesión con un Google de mentira. Sin red y sin tocar tu música ni tu cuenta.

    python3 -m unittest discover -s apps/vortex-player/pruebas -p 'test_nube.py'

Los casos de mezclar son los mismos que `NubeTest.kt` del teléfono: las dos apps tienen que mezclar igual.
"""
from __future__ import annotations

import json
import os
import shutil
import sqlite3
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
import urllib.request
from pathlib import Path

AQUI = Path(__file__).resolve().parent
sys.path.insert(0, str(AQUI.parent))

from vortex_player import cuenta_google, nube, sincronia  # noqa: E402
from vortex_player.biblioteca import Biblioteca, id_de  # noqa: E402


def c(vid: str, t: int) -> dict:
    return {"titulo": "T" + vid, "artista": "A", "album": "", "duracion": 200, "agregada": t}


def doc(**partes) -> dict:
    return nube.desde(nube.Local(**partes))


class Mezclar(unittest.TestCase):
    def test_junta_las_canciones_de_los_dos(self) -> None:
        pc = doc(canciones={"aaaaaaaaaaa": c("a", 10), "bbbbbbbbbbb": c("b", 20)})
        tel = doc(canciones={"ccccccccccc": c("c", 30)})
        m = nube.mezclar(pc, tel)
        self.assertEqual({"aaaaaaaaaaa", "bbbbbbbbbbb", "ccccccccccc"}, set(m["canciones"]))
        self.assertTrue(nube.iguales(m, nube.mezclar(tel, pc)))

    def test_lo_quitado_se_queda_quitado_salvo_que_se_vuelva_a_bajar(self) -> None:
        base = doc(canciones={"aaaaaaaaaaa": c("a", 10), "bbbbbbbbbbb": c("b", 10)})
        tel = doc(quitadas={"aaaaaaaaaaa": 50, "bbbbbbbbbbb": 50})
        otra = doc(canciones={"bbbbbbbbbbb": c("b", 90)})
        m = nube.mezclar(nube.mezclar(base, tel), otra)
        self.assertEqual(["bbbbbbbbbbb"], list(m["canciones"]))
        self.assertEqual({"aaaaaaaaaaa": 50}, m["quitadas"])

    def test_de_la_playlist_gana_la_ultima_que_cambio(self) -> None:
        vieja = {"nombre": "Anime", "url": "", "canciones": ["aaaaaaaaaaa"], "modificada": 10}
        nueva = {"nombre": "Anime 2026", "url": "", "canciones": ["bbbbbbbbbbb", "aaaaaaaaaaa"], "modificada": 20}
        m = nube.mezclar(doc(playlists={"u1": nueva}), doc(playlists={"u1": vieja}))
        self.assertEqual({"u1": nueva}, m["playlists"])
        borrada = nube.mezclar(m, doc(borradas={"u1": 30}))
        self.assertEqual({}, borrada["playlists"])
        revive = nube.mezclar(borrada, doc(playlists={"u1": {**nueva, "modificada": 40}}))
        self.assertEqual(1, len(revive["playlists"]))

    def test_el_plan_baja_lo_que_falta_y_quita_lo_borrado_en_otro_lado(self) -> None:
        d = nube.mezclar(doc(canciones={"aaaaaaaaaaa": c("a", 10), "ccccccccccc": c("c", 10)},
                             playlists={"u1": {"nombre": "Chill", "url": "https://youtube.com/playlist?list=PL1",
                                               "canciones": ["aaaaaaaaaaa"], "modificada": 10}}),
                         doc(quitadas={"bbbbbbbbbbb": 50}, borradas={"u2": 50}))
        aqui = nube.Local(canciones={"aaaaaaaaaaa": c("a", 5), "bbbbbbbbbbb": c("b", 40)},
                          playlists={"u2": {"nombre": "Vieja", "url": "", "canciones": [], "modificada": 30}})
        p = nube.plan(d, aqui)
        self.assertEqual(["ccccccccccc"], [v for v, _ in p.bajar])
        self.assertEqual(["bbbbbbbbbbb"], p.borrar)
        self.assertEqual(["u1"], [u for u, _ in p.playlists])
        self.assertEqual(["u2"], p.quitar_playlists)
        cambiada = nube.Local(playlists={"u1": {"nombre": "Mía", "url": "", "canciones": [], "modificada": 99}})
        self.assertEqual([], nube.plan(d, cambiada).playlists)

    def test_lee_lo_que_escribe_el_telefono(self) -> None:
        # Así lo guarda Nube.kt (org.json: sin espacios, enteros largos)
        tel = ('{"formato":1,"canciones":{"aaaaaaaaaaa":{"titulo":"Idol","artista":"YOASOBI","album":"","duracion":213,'
               '"agregada":1793472533000}},"quitadas":{},"playlists":{"u1":{"nombre":"Anime","url":"",'
               '"canciones":["aaaaaaaaaaa"],"modificada":1793472533000}},"borradas":{}}')
        d = nube.leer(tel)
        self.assertEqual("YOASOBI", d["canciones"]["aaaaaaaaaaa"]["artista"])
        self.assertTrue(nube.iguales(nube.leer("basura"), nube.vacio()))
        self.assertTrue(nube.iguales(nube.leer("[1, 2]"), nube.vacio()))


class DriveFalso:
    """La carpeta oculta de Drive, en memoria: lo que comparten la PC y el teléfono."""

    def __init__(self) -> None:
        self.archivo: str | None = None
        self.escrituras = 0

    def buscar(self):
        return "f1" if self.archivo is not None else None

    def leer(self, fid):
        return self.archivo.encode()

    def crear(self, texto):
        self.archivo = texto
        self.escrituras += 1
        return "f1"

    def escribir(self, fid, texto):
        self.archivo = texto
        self.escrituras += 1


class Temporal(unittest.TestCase):
    def setUp(self) -> None:
        self.t = Path(tempfile.mkdtemp(prefix="vortex-nube-"))
        self.entorno = dict(os.environ)
        os.environ.update({"VORTEX_PLAYER_MUSICA": str(self.t / "Música"), "VORTEX_PLAYER_DATOS": str(self.t / "datos"),
                           "VORTEX_PLAYER_CONFIG": str(self.t / "config"), "VORTEX_PLAYER_CACHE": str(self.t / "cache"),
                           "XDG_CONFIG_HOME": str(self.t / "xdg")})
        self.abiertas: list[Biblioteca] = []

    def tearDown(self) -> None:
        for b in self.abiertas:
            b.cerrar()
        os.environ.clear()
        os.environ.update(self.entorno)
        shutil.rmtree(self.t, ignore_errors=True)

    def bib(self, nombre: str | Path) -> Biblioteca:
        b = Biblioteca(nombre if isinstance(nombre, Path) else self.t / nombre / "biblioteca.db")
        self.abiertas.append(b)
        return b

    @staticmethod
    def bajada(b: Biblioteca, vid: str, titulo: str, cuando: float | None = None) -> str:
        """Como si Vortex Player la hubiera bajado (sin ffprobe ni archivo de verdad)."""
        ruta = f"/musica/Vortex Player/Artista/{titulo} [{vid}].opus"
        b.db.execute("INSERT OR REPLACE INTO canciones (ruta, mtime, titulo, artista, album, duracion, codec, origen, agregada) "
                     "VALUES (?,?,?,?,?,?,?,?,?)", (ruta, 0, titulo, "Artista", "", 200.4, "opus", "youtube", cuando or time.time()))
        b.db.commit()
        return ruta


class BibliotecaNube(Temporal):
    def test_la_biblioteca_vieja_se_pone_al_dia(self) -> None:
        archivo = self.t / "vieja.db"
        db = sqlite3.connect(archivo)
        db.executescript("CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL, url TEXT DEFAULT '', creada REAL);"
                         "INSERT INTO playlists (nombre, url, creada) VALUES ('Chill piano', 'https://x', 100);")
        db.commit()
        db.close()
        b = self.bib(archivo)
        local = b.estado_nube()
        (u, p), = local.playlists.items()
        self.assertEqual(("Chill piano", 100000), (p["nombre"], p["modificada"]))
        self.assertEqual(local.playlists, b.estado_nube().playlists)   # el uuid se queda

    def test_lo_que_borras_queda_anotado_y_volver_a_bajarla_lo_quita(self) -> None:
        b = self.bib("pc")
        pid = b.crear_playlist("Anime")
        u = next(iter(b.estado_nube().playlists))
        b.anotar_cancion_borrada("aaaaaaaaaaa")
        b.borrar_playlist(pid)
        local = b.estado_nube()
        self.assertIn("aaaaaaaaaaa", local.quitadas)
        self.assertIn(u, local.borradas)
        self.assertEqual("aaaaaaaaaaa", id_de("/m/Idol [aaaaaaaaaaa].opus"))
        self.assertEqual("", id_de("/m/Idol.flac"))


class DosAparatos(Temporal):
    def test_la_pc_y_el_telefono_quedan_iguales(self) -> None:
        drive = DriveFalso()
        pc, tel = self.bib("pc"), self.bib("tel")
        r1 = self.bajada(pc, "aaaaaaaaaaa", "Idol")
        self.bajada(pc, "bbbbbbbbbbb", "Yoru ni Kakeru")
        pid = pc.crear_playlist("Anime")
        pc.agregar_a_playlist(pid, r1)
        local_tel = self.bajada(tel, "ccccccccccc", "Solo del teléfono")

        borradas: list[str] = []
        _, plan = sincronia.sincronizar(pc, drive, lambda vid, ruta: borradas.append(vid))
        self.assertEqual([], plan.bajar)
        _, plan = sincronia.sincronizar(tel, drive, lambda vid, ruta: borradas.append(vid))
        self.assertEqual({"aaaaaaaaaaa", "bbbbbbbbbbb"}, {v for v, _ in plan.bajar})
        # La playlist ya existe en el teléfono, vacía hasta que se bajen sus canciones
        self.assertEqual(["Anime"], [p["nombre"] for p in tel.playlists()])
        for vid, datos in plan.bajar:
            self.bajada(tel, vid, datos["titulo"])
        sincronia.sincronizar(tel, drive, lambda vid, ruta: borradas.append(vid))
        (p,) = tel.playlists()
        self.assertEqual(["Idol"], [x["titulo"] for x in tel.canciones_de_playlist(p["id"])])

        # La del teléfono llega a la PC
        _, plan = sincronia.sincronizar(pc, drive, lambda vid, ruta: borradas.append(vid))
        self.assertEqual(["ccccccccccc"], [v for v, _ in plan.bajar])

        # Borrar en el teléfono → se borra en la PC; renombrar en la PC → se renombra en el teléfono
        tel.anotar_cancion_borrada("bbbbbbbbbbb")
        tel.quitar(tel.rutas_por_id()["bbbbbbbbbbb"])
        sincronia.sincronizar(tel, drive, lambda vid, ruta: None)
        pc.renombrar_playlist(pid, "Anime 2026")
        sincronia.sincronizar(pc, drive, lambda vid, ruta: (borradas.append(vid), pc.quitar(ruta)))
        self.assertEqual(["bbbbbbbbbbb"], borradas)
        sincronia.sincronizar(tel, drive, lambda vid, ruta: None)
        self.assertEqual(["Anime 2026"], [p["nombre"] for p in tel.playlists()])
        self.assertIn(local_tel, [x["ruta"] for x in tel.canciones()])

        # Sin cambios no se vuelve a escribir en Drive
        antes = drive.escrituras
        sincronia.sincronizar(pc, drive, lambda vid, ruta: None)
        sincronia.sincronizar(tel, drive, lambda vid, ruta: None)
        self.assertEqual(antes, drive.escrituras)


class GoogleFalso(Temporal):
    """Iniciar sesión de punta a punta: el «navegador» va al puerto local como lo haría Google."""

    def setUp(self) -> None:
        super().setUp()
        os.environ.update({"VORTEX_PLAYER_GOOGLE_ID": "id-prueba", "VORTEX_PLAYER_GOOGLE_SECRETO": "secreto-prueba"})
        self.pedidos: list[tuple[str, str, dict]] = []
        self.original = cuenta_google.pedir
        cuenta_google.olvidar_token()

        def pedir(metodo, url, cuerpo=None, cabeceras=None):
            datos = dict(urllib.parse.parse_qsl(cuerpo.decode())) if cuerpo and b"=" in cuerpo and b"{" not in cuerpo else {}
            self.pedidos.append((metodo, url, datos))
            if url == cuenta_google.TOKEN and datos.get("grant_type") == "authorization_code":
                return 200, json.dumps({"access_token": "acceso1", "refresh_token": "renovar1", "expires_in": 3600}).encode()
            if url == cuenta_google.TOKEN and datos.get("grant_type") == "refresh_token":
                return 200, json.dumps({"access_token": "acceso2", "expires_in": 3600}).encode()
            if "/about" in url:
                return 200, json.dumps({"user": {"displayName": "Mike", "emailAddress": "mike@ejemplo.com"}}).encode()
            if url == cuenta_google.REVOCAR:
                return 200, b"{}"
            return 404, b""
        cuenta_google.pedir = pedir

    def tearDown(self) -> None:
        cuenta_google.pedir = self.original
        cuenta_google.olvidar_token()
        super().tearDown()

    def test_iniciar_y_cerrar_sesion(self) -> None:
        def navegador(url: str) -> None:
            q = dict(urllib.parse.parse_qsl(urllib.parse.urlparse(url).query))
            self.assertEqual("https://www.googleapis.com/auth/drive.appdata", q["scope"])
            self.assertEqual("S256", q["code_challenge_method"])
            regreso = f"{q['redirect_uri']}/?code=codigo1&state={q['state']}"
            threading.Thread(target=lambda: urllib.request.urlopen(regreso, timeout=5).read(), daemon=True).start()

        d = cuenta_google.iniciar_sesion(navegador, esperar=10)
        self.assertEqual({"correo": "mike@ejemplo.com", "nombre": "Mike"}, d)
        archivo = self.t / "config" / "google.json"
        self.assertEqual(0o600, archivo.stat().st_mode & 0o777)
        self.assertEqual("renovar1", cuenta_google.sesion()["refresh_token"])
        intercambio = next(p for m, u, p in self.pedidos if p.get("grant_type") == "authorization_code")
        self.assertIn("code_verifier", intercambio)

        self.assertEqual("acceso1", cuenta_google.token())
        cuenta_google.olvidar_token()
        self.assertEqual("acceso2", cuenta_google.token())

        cuenta_google.cerrar_sesion()
        self.assertFalse(archivo.exists())
        self.assertEqual({}, cuenta_google.sesion())
        with self.assertRaises(cuenta_google.SinPermiso):
            cuenta_google.token()

    def test_un_estado_que_no_coincide_no_inicia_sesion(self) -> None:
        def navegador(url: str) -> None:
            q = dict(urllib.parse.parse_qsl(urllib.parse.urlparse(url).query))
            regreso = f"{q['redirect_uri']}/?code=codigo1&state=otro"
            threading.Thread(target=lambda: urllib.request.urlopen(regreso, timeout=5).read(), daemon=True).start()

        with self.assertRaises(RuntimeError):
            cuenta_google.iniciar_sesion(navegador, esperar=10)
        self.assertEqual({}, cuenta_google.sesion())

    def test_drive_sube_y_lee_el_archivo(self) -> None:
        guardado: dict = {}

        def pedir(metodo, url, cuerpo=None, cabeceras=None):
            self.assertEqual("Bearer t", cabeceras["Authorization"])
            if "files?spaces=appDataFolder" in url:
                return 200, json.dumps({"files": [{"id": "f9"}] if guardado else []}).encode()
            if metodo == "POST" and "uploadType=multipart" in url:
                self.assertIn('"parents": ["appDataFolder"]', cuerpo.decode())
                guardado["texto"] = cuerpo.decode().split("\r\n\r\n")[2].split("\r\n--")[0]
                return 200, b'{"id": "f9"}'
            if metodo == "PATCH":
                guardado["texto"] = cuerpo.decode()
                return 200, b"{}"
            if url.endswith("/files/f9?alt=media"):
                return 200, guardado["texto"].encode()
            return 401, b""
        cuenta_google.pedir = pedir
        dr = cuenta_google.Drive("t")
        self.assertIsNone(dr.buscar())
        dr.crear('{"formato": 1}')
        self.assertEqual("f9", dr.buscar())
        dr.escribir("f9", '{"formato": 2}')
        self.assertEqual(b'{"formato": 2}', dr.leer("f9"))
        with self.assertRaises(cuenta_google.SinPermiso):
            dr.usuario()


if __name__ == "__main__":
    unittest.main()
