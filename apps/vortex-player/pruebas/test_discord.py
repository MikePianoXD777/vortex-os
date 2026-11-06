"""Pruebas de Discord Rich Presence contra un Discord falso (un QLocalServer en una carpeta temporal).

No tocan el Discord de quien las corre: XDG_RUNTIME_DIR apunta a la carpeta temporal.
"""
from __future__ import annotations

import os
import shutil
import sys
import tempfile
import time
import unittest
from pathlib import Path

os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from PySide6.QtCore import QCoreApplication, QObject, Signal  # noqa: E402
from PySide6.QtNetwork import QLocalServer  # noqa: E402

from vortex_player import presencia_discord as pd  # noqa: E402

YT = "/música/Bohemia [dQw4w9WgXcQ].opus"
app = QCoreApplication.instance() or QCoreApplication([])


def esperar(condicion, ms: int = 3000) -> bool:
    fin = time.monotonic() + ms / 1000
    while time.monotonic() < fin:
        app.processEvents()
        if condicion():
            return True
        time.sleep(0.005)
    return bool(condicion())


class ReproductorFalso(QObject):
    cambioCancion = Signal()
    cambioEstado = Signal()

    def __init__(self):
        super().__init__()
        self.cancion = {}
        self.posicion = 0.0
        self.duracion = 200.0
        self.pausado = False
        self.velocidad = 1.0

    def tocar(self, titulo: str, ruta: str = YT) -> None:
        self.cancion = {"titulo": titulo, "artista": "Artista", "album": "Disco", "ruta": ruta}
        self.posicion = 0.0
        self.cambioCancion.emit()
        self.cambioEstado.emit()

    def pausar(self, si: bool = True) -> None:
        self.pausado = si
        self.cambioEstado.emit()


class DiscordFalso:
    """Contesta el saludo con READY y guarda lo que le llega."""

    def __init__(self, ruta: Path, leer=True):
        QLocalServer.removeServer(str(ruta))
        self.servidor = QLocalServer()
        assert self.servidor.listen(str(ruta)), self.servidor.errorString()
        self.servidor.newConnection.connect(self._nueva)
        self.marcos: list[tuple[int, dict]] = []
        self.conexiones = []
        self.buffer = b""
        self.saludar = True

    def _nueva(self) -> None:
        c = self.servidor.nextPendingConnection()
        self.conexiones.append(c)
        c.readyRead.connect(lambda: self._leer(c))

    def _leer(self, c) -> None:
        marcos, self.buffer = pd.leer_marcos(self.buffer + bytes(c.readAll()))
        for op, datos in marcos:
            self.marcos.append((op, datos))
            if op == pd.SALUDO and self.saludar:
                c.write(pd.marco(pd.MARCO, {"cmd": "DISPATCH", "evt": "READY", "data": {"v": 1}}))
                c.flush()

    def mandar(self, op: int, datos: dict) -> None:
        self.conexiones[-1].write(pd.marco(op, datos))
        self.conexiones[-1].flush()

    def actividades(self) -> list[dict | None]:
        return [d["args"].get("activity") for op, d in self.marcos if op == pd.MARCO and d.get("cmd") == "SET_ACTIVITY"]

    def titulos(self) -> list[str | None]:
        return [a["details"] if a else None for a in self.actividades()]

    def cerrar(self) -> None:
        for c in self.conexiones:
            c.abort()
        self.servidor.close()


class Marcos(unittest.TestCase):
    def test_ida_y_vuelta_y_partido(self) -> None:
        datos = pd.marco(1, {"a": "ñandú"}) + pd.marco(3, {"b": 2})
        self.assertEqual(pd.leer_marcos(datos), ([(1, {"a": "ñandú"}), (3, {"b": 2})], b""))
        marcos, resto = pd.leer_marcos(datos[:-2])
        self.assertEqual(marcos, [(1, {"a": "ñandú"})])
        self.assertEqual(pd.leer_marcos(resto + datos[-2:])[0], [(3, {"b": 2})])

    def test_el_largo_cuenta_bytes_no_letras(self) -> None:
        self.assertEqual(pd.leer_marcos(pd.marco(1, {"t": "日本語"}))[0][0][1]["t"], "日本語")


class Actividad(unittest.TestCase):
    def test_de_youtube(self) -> None:
        a = pd.actividad({"titulo": "Bohemia", "artista": "Queen", "album": "Opera", "ruta": YT}, 10, 100, ahora=1000)
        self.assertEqual(a["type"], 2)
        self.assertEqual((a["details"], a["state"]), ("Bohemia", "Queen"))
        self.assertEqual(a["assets"], {"large_image": "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg",
                                       "large_text": "Opera"})
        self.assertEqual(a["timestamps"], {"start": 990000, "end": 1090000})
        self.assertEqual(a["buttons"][0]["url"], "https://music.youtube.com/watch?v=dQw4w9WgXcQ")
        self.assertLessEqual(len(a["buttons"][0]["label"]), 32)

    def test_archivo_propio(self) -> None:
        a = pd.actividad({"ruta": "/música/mi canción.flac"}, 0, 0)
        self.assertEqual(a["details"], "mi canción")
        self.assertEqual(a["assets"]["large_image"], pd.LOGO)
        for falta in ("state", "buttons", "timestamps"):
            self.assertNotIn(falta, a)

    def test_limites_de_discord(self) -> None:
        a = pd.actividad({"titulo": "x" * 300, "artista": "U", "ruta": "/a.flac"}, 0, 0)
        self.assertEqual(len(a["details"]), 128)
        self.assertEqual(len(a["state"]), 2)

    def test_practicando_no_lleva_tiempo(self) -> None:
        self.assertNotIn("timestamps", pd.actividad({"titulo": "aa", "ruta": YT}, 0, 100, velocidad=0.75))

    def test_misma(self) -> None:
        c = {"titulo": "aa", "ruta": YT}
        a = pd.actividad(c, 0, 100, ahora=1000)
        self.assertTrue(pd._misma(pd.actividad(c, 1, 100, ahora=1001), a))      # el tiempo solo avanzó
        self.assertFalse(pd._misma(pd.actividad(c, 50, 100, ahora=1001), a))    # un salto
        self.assertFalse(pd._misma(pd.actividad({"titulo": "bb", "ruta": YT}, 0, 100, ahora=1000), a))
        self.assertFalse(pd._misma(pd.actividad(c, 0, 100, velocidad=0.5), a))
        self.assertFalse(pd._misma(a, None))


class Conexion(unittest.TestCase):
    def setUp(self) -> None:
        self.t = Path(tempfile.mkdtemp(prefix="vp-dc-", dir="/tmp"))
        self.entorno = os.environ.get("XDG_RUNTIME_DIR")
        os.environ["XDG_RUNTIME_DIR"] = str(self.t)
        self.r = ReproductorFalso()
        self.activa = True
        self.falsos: list[DiscordFalso] = []

    def tearDown(self) -> None:
        for f in self.falsos:
            f.cerrar()
        if self.entorno is None:
            del os.environ["XDG_RUNTIME_DIR"]
        else:
            os.environ["XDG_RUNTIME_DIR"] = self.entorno
        shutil.rmtree(self.t, ignore_errors=True)

    def discord(self, sub: str = "") -> DiscordFalso:
        d = self.t / sub
        d.mkdir(parents=True, exist_ok=True)
        f = DiscordFalso(d / "discord-ipc-0")
        self.falsos.append(f)
        return f

    def presencia(self, cliente: str = "123", entre_envios: float = 0.0) -> pd.PresenciaDiscord:
        p = pd.PresenciaDiscord(self.r, lambda: self.activa, cliente, entre_envios, espera_ms=1)
        p._pulso.setInterval(30)
        return p

    def test_saluda_con_su_id_y_manda_la_cancion(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]), f.marcos)
        self.assertEqual(f.marcos[0], (pd.SALUDO, {"v": 1, "client_id": "123"}))
        self.assertEqual(f.marcos[1][1]["args"]["pid"], os.getpid())
        del p

    def test_no_repite_si_nada_cambio(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        esperar(lambda: False, 200)     # varios pulsos
        self.assertEqual(f.titulos(), ["Uno"])
        del p

    def test_pausa_cambio_y_salto(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        self.r.pausar()
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno", None]), f.titulos())
        self.assertFalse(p._pulso.isActive())
        self.r.pausar(False)
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno", None, "Uno"]), f.titulos())
        self.r.tocar("Dos")
        self.assertTrue(esperar(lambda: f.titulos()[-1] == "Dos"))
        n = len(f.titulos())
        self.r.posicion = 150.0         # un salto no manda señal de estado: lo nota el pulso
        self.assertTrue(esperar(lambda: len(f.titulos()) == n + 1), f.titulos())

    def test_el_interruptor(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        self.activa = False
        p.actualizar()
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno", None]))
        self.r.tocar("Dos")
        esperar(lambda: False, 100)
        self.assertEqual(f.titulos(), ["Uno", None])
        self.activa = True
        p.actualizar()
        self.assertTrue(esperar(lambda: f.titulos()[-1] == "Dos"))

    def test_apagado_no_se_conecta(self) -> None:
        f = self.discord()
        self.activa = False
        p = self.presencia()
        self.r.tocar("Uno")
        esperar(lambda: False, 100)
        self.assertEqual(f.conexiones, [])
        del p

    def test_sin_id_no_se_conecta(self) -> None:
        f = self.discord()
        p = self.presencia(cliente="")
        self.r.tocar("Uno")
        esperar(lambda: False, 100)
        self.assertEqual(f.conexiones, [])
        self.assertFalse(p._pulso.isActive())

    def test_discord_abre_despues(self) -> None:
        p = self.presencia()
        self.r.tocar("Uno")
        esperar(lambda: p._pulso.isActive(), 500)
        esperar(lambda: False, 80)
        f = self.discord()
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]), f.marcos)

    def test_se_cierra_y_vuelve(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        f.cerrar()
        self.assertTrue(esperar(lambda: not p._lista))
        g = self.discord()
        self.assertTrue(esperar(lambda: g.titulos() == ["Uno"]), g.marcos)

    def test_socket_muerto_pasa_al_siguiente(self) -> None:
        (self.t / "discord-ipc-0").touch()      # quedó de un Discord que tronó: nadie escucha
        f = DiscordFalso(self.t / "discord-ipc-1")
        self.falsos.append(f)
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]), f.marcos)
        del p

    def test_flatpak(self) -> None:
        f = self.discord("app/com.discordapp.Discord")
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]), f.marcos)
        del p

    def test_ping_y_error(self) -> None:
        f = self.discord()
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        f.mandar(pd.PING, {"n": 7})
        self.assertTrue(esperar(lambda: (pd.PONG, {"n": 7}) in f.marcos))
        f.mandar(pd.MARCO, {"evt": "ERROR", "data": {"code": 4000, "message": "de prueba"}})
        esperar(lambda: False, 30)
        self.assertTrue(p._lista)

    def test_cierre_por_id_malo(self) -> None:
        f = self.discord()
        f.saludar = False
        p = self.presencia()
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.marcos and f.conexiones))
        f.mandar(pd.CIERRE, {"code": 4000, "message": "Invalid Client ID"})
        self.assertTrue(esperar(lambda: not p._conectada or len(f.conexiones) > 1))
        self.assertEqual(f.titulos(), [])

    def test_respeta_el_limite_de_discord(self) -> None:
        f = self.discord()
        p = self.presencia(entre_envios=0.3)
        self.r.tocar("Uno")
        self.assertTrue(esperar(lambda: f.titulos() == ["Uno"]))
        inicio = time.monotonic()
        self.r.tocar("Dos")
        esperar(lambda: False, 30)
        self.r.tocar("Tres")
        self.assertTrue(esperar(lambda: f.titulos()[-1] == "Tres"), f.titulos())
        self.assertGreaterEqual(time.monotonic() - inicio, 0.2)
        self.assertNotIn("Dos", f.titulos())    # brincada antes de mandarla
        del p


class SinDiscord(unittest.TestCase):
    def test_solo_existentes_y_en_orden(self) -> None:
        with tempfile.TemporaryDirectory() as t:
            antes = os.environ.get("XDG_RUNTIME_DIR")
            os.environ["XDG_RUNTIME_DIR"] = t
            try:
                self.assertEqual(pd.rutas_socket(), [])
                (Path(t) / "discord-ipc-3").touch()
                self.assertEqual(pd.rutas_socket(), [t + "/discord-ipc-3"])
            finally:
                if antes is None:
                    del os.environ["XDG_RUNTIME_DIR"]
                else:
                    os.environ["XDG_RUNTIME_DIR"] = antes


if __name__ == "__main__":
    unittest.main()
