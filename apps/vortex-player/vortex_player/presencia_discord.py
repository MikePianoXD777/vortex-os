"""Discord Rich Presence: «Escuchando Vortex Player», con la canción, el artista, la portada y el tiempo.

Habla con la app de escritorio de Discord por su canal local (un socket de Unix en Linux, una tubería con
nombre en Windows; QLocalSocket abre los dos). Sin Discord abierto no hace nada y vuelve a intentar cada
15 s mientras suena música. En pausa se borra, y al cerrar la app Discord la quita solo.
"""
from __future__ import annotations

import json
import os
import struct
import sys
import time
import uuid
from pathlib import Path

from PySide6.QtCore import QObject, QTimer
from PySide6.QtNetwork import QLocalSocket

from . import NOMBRE, biblioteca, plataforma

# El Application ID de discord.com/developers/applications. No es secreto: lo lleva cualquier app con presencia.
CLIENT_ID = "1568275063319437433"

# Discord no acepta SVG, y una imagen local no le sirve: tiene que ser un enlace.
LOGO = ("https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/"
        "apps/vortex-player/empaquetado/logo-discord.png")

SALUDO, MARCO, CIERRE, PING, PONG = 0, 1, 2, 3, 4
ESCUCHANDO = 2              # tipo de actividad: «Escuchando…», como Spotify
ENTRE_ENVIOS = 4.0          # Discord permite 5 actualizaciones cada 20 s
TOLERANCIA_MS = 3000        # si el final se movió más que esto, hubo un salto en la canción


def marco(op: int, datos: dict) -> bytes:
    cuerpo = json.dumps(datos, ensure_ascii=False).encode()
    return struct.pack("<II", op, len(cuerpo)) + cuerpo


def leer_marcos(buffer: bytes) -> tuple[list[tuple[int, dict]], bytes]:
    """Los marcos completos que hay en `buffer` y lo que sobra (un marco puede llegar partido)."""
    marcos = []
    while len(buffer) >= 8:
        op, largo = struct.unpack_from("<II", buffer)
        if len(buffer) < 8 + largo:
            break
        try:
            marcos.append((op, json.loads(buffer[8:8 + largo])))
        except ValueError:
            pass
        buffer = buffer[8 + largo:]
    return marcos, buffer


def rutas_socket() -> list[str]:
    """Dónde puede estar escuchando Discord. En Linux, solo los sockets que existen."""
    if plataforma.WINDOWS:
        return [f"discord-ipc-{i}" for i in range(10)]
    raiz = Path(os.environ.get("XDG_RUNTIME_DIR") or "/tmp")
    carpetas = ("", "app/com.discordapp.Discord", ".flatpak/dev.vencord.Vesktop/xdg-run", "snap.discord")
    rutas = (raiz / c / f"discord-ipc-{i}" for c in carpetas for i in range(10))
    return [str(r) for r in rutas if r.exists()]


def _texto(t: str) -> str:
    """Discord pide de 2 a 128 caracteres; con menos o con más rechaza la actividad entera."""
    t = " ".join(str(t or "").split())[:128]
    return t + "​" if len(t) == 1 else t


def actividad(c: dict, posicion: float, duracion: float, velocidad: float = 1.0, ahora: float | None = None) -> dict:
    """Lo que se le manda a Discord de la canción `c`."""
    ruta = c.get("ruta") or ""
    vid = biblioteca.id_de(ruta)
    a = {"type": ESCUCHANDO,
         "details": _texto(c.get("titulo") or Path(ruta).stem or NOMBRE),
         "assets": {"large_image": f"https://i.ytimg.com/vi/{vid}/mqdefault.jpg" if vid else LOGO,
                    "large_text": _texto(c.get("album") or NOMBRE)}}
    artista = _texto(c.get("artista"))
    if artista:
        a["state"] = artista
    if duracion > 0 and velocidad == 1:     # practicando más lento, el tiempo de Discord estaría mal
        inicio = (time.time() if ahora is None else ahora) - posicion
        a["timestamps"] = {"start": int(inicio * 1000), "end": int((inicio + duracion) * 1000)}
    if vid:
        a["buttons"] = [{"label": "Escuchar en YouTube Music", "url": f"https://music.youtube.com/watch?v={vid}"}]
    return a


def _misma(a: dict, b: dict | None) -> bool:
    """Igual canción y sin saltos: el tiempo avanza solo en Discord, no hace falta volver a mandarla."""
    if b is None:
        return False
    fin_a = a.get("timestamps", {}).get("end")
    fin_b = b.get("timestamps", {}).get("end")
    if (fin_a is None) != (fin_b is None) or (fin_a is not None and abs(fin_a - fin_b) > TOLERANCIA_MS):
        return False
    return {k: v for k, v in a.items() if k != "timestamps"} == {k: v for k, v in b.items() if k != "timestamps"}


class PresenciaDiscord(QObject):
    def __init__(self, reproductor, activa=lambda: True, cliente: str | None = None,
                 entre_envios: float = ENTRE_ENVIOS, espera_ms: int = 1000):
        super().__init__()
        self.espera_ms = espera_ms
        self.r = reproductor
        self.activa = activa                    # el interruptor de Ajustes
        self.cliente = CLIENT_ID if cliente is None else cliente
        self.entre_envios = entre_envios
        self._buffer = b""
        self._candidatos: list[str] = []
        self._conectada = False
        self._lista = False                     # Discord ya contestó el saludo
        self._ultima: dict | None = None        # lo que Discord está mostrando
        self._ultimo_envio = -entre_envios
        self._s = QLocalSocket(self)
        self._s.connected.connect(self._al_conectar)
        self._s.disconnected.connect(self._al_desconectar)
        self._s.errorOccurred.connect(self._al_fallar)
        self._s.readyRead.connect(self._leer)
        self._rebote = QTimer(self, singleShot=True)
        self._rebote.timeout.connect(self._publicar)
        self._pulso = QTimer(self, interval=15000)  # reconecta y nota los saltos; solo corre mientras suena
        self._pulso.timeout.connect(self._publicar)
        reproductor.cambioCancion.connect(self.actualizar)
        reproductor.cambioEstado.connect(self.actualizar)

    def actualizar(self) -> None:
        """Algo cambió. Se espera un momento: al cambiar de canción llegan varias señales juntas."""
        if not self._rebote.isActive():
            self._rebote.start(self.espera_ms)

    def _sonando(self) -> bool:
        return bool(self.cliente and self.r.cancion and not self.r.pausado and self.activa())

    def _publicar(self) -> None:
        if not self._sonando():
            self._pulso.stop()
            if self._lista and self._ultima is not None:
                self._mandar_actividad(None)
            return
        if not self._pulso.isActive():
            self._pulso.start()
        if not self._lista:
            self._conectar()
            return
        a = actividad(self.r.cancion, self.r.posicion, self.r.duracion or float(self.r.cancion.get("duracion") or 0),
                      self.r.velocidad)
        if not _misma(a, self._ultima):
            self._mandar_actividad(a)

    def _mandar_actividad(self, a: dict | None) -> None:
        falta = self._ultimo_envio + self.entre_envios - time.monotonic()
        if falta > 0:
            self._rebote.start(int(falta * 1000) + 1)
            return
        args = {"pid": os.getpid()}
        if a is not None:
            args["activity"] = a
        self._escribir(MARCO, {"cmd": "SET_ACTIVITY", "args": args, "nonce": str(uuid.uuid4())})
        self._ultima = a
        self._ultimo_envio = time.monotonic()

    def _escribir(self, op: int, datos: dict) -> None:
        self._s.write(marco(op, datos))
        self._s.flush()

    # --- la conexión -----------------------------------------------------------------------------------

    def _conectar(self) -> None:
        if self._s.state() == QLocalSocket.LocalSocketState.UnconnectedState and not self._candidatos:
            self._candidatos = rutas_socket()
            self._probar_siguiente()

    def _probar_siguiente(self) -> None:
        if self._candidatos and self._s.state() == QLocalSocket.LocalSocketState.UnconnectedState:
            self._s.connectToServer(self._candidatos.pop(0))

    def _al_conectar(self) -> None:
        self._conectada = True
        self._candidatos = []
        self._escribir(SALUDO, {"v": 1, "client_id": self.cliente})

    def _al_fallar(self, _error) -> None:
        if not self._conectada:     # este canal no era: al siguiente, ya fuera de la señal de error
            QTimer.singleShot(0, self._probar_siguiente)

    def _al_desconectar(self) -> None:  # Discord se cerró: el pulso vuelve a buscarlo
        self._buffer = b""
        self._conectada = self._lista = False
        self._ultima = None

    def _leer(self) -> None:
        marcos, self._buffer = leer_marcos(self._buffer + bytes(self._s.readAll()))
        for op, datos in marcos:
            if op == PING:
                self._escribir(PONG, datos)
            elif op == CIERRE:
                print("Vortex Player: Discord cerró la conexión:", datos.get("message"), file=sys.stderr)
                self._s.abort()
            elif datos.get("evt") == "READY":
                self._lista = True
                self._publicar()
            elif datos.get("evt") == "ERROR":
                print("Vortex Player: Discord rechazó la actividad:", datos.get("data"), file=sys.stderr)
