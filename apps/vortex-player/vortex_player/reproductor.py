"""El reproductor: mpv sin ventana, manejado por su socket JSON (IPC).

mpv toca cada archivo a su frecuencia y bits originales por PipeWire (sin remuestrear, si PipeWire
deja cambiar de frecuencia: ver audio.config_hires) y sin normalizar el volumen. La cola, el aleatorio
y el repetir los lleva Vortex Player; mpv solo toca la canción de turno.

Para pruebas: VORTEX_PLAYER_AO=null no saca sonido y VORTEX_PLAYER_MPV cambia el mpv que se usa.
"""
from __future__ import annotations

import json
import os
import random
import tempfile
from pathlib import Path

from PySide6.QtCore import Property, QObject, QProcess, QTimer, Signal, Slot
from PySide6.QtNetwork import QLocalSocket

OBSERVAR = {1: "time-pos", 2: "duration", 3: "pause", 4: "idle-active"}


class Reproductor(QObject):
    cambioCancion = Signal()
    cambioEstado = Signal()
    cambioPosicion = Signal()
    cambioCola = Signal()
    error = Signal(str)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._cola: list[dict] = []
        self._orden: list[int] = []     # el orden en que se tocan (cambia con aleatorio)
        self._i = -1                    # posición en _orden
        self._posicion = 0.0
        self._duracion = 0.0
        self._pausado = True
        self._volumen = 80
        self._aleatorio = False
        self._repetir = "no"            # «no», «todo» o «una»
        self._pendientes: list[bytes] = []
        self._buffer = b""
        carpeta = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
        self._socket_ruta = str(Path(carpeta) / f"vortex-player-mpv-{os.getpid()}.sock")
        if len(self._socket_ruta.encode()) > 100:   # los sockets de Unix no aceptan rutas de más de 108 bytes
            self._socket_ruta = str(Path(tempfile.gettempdir()) / f"vortex-player-mpv-{os.getuid()}-{os.getpid()}.sock")
        self._mpv = QProcess(self)
        self._mpv.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
        self._mpv.finished.connect(self._mpv_termino)
        self._socket = QLocalSocket(self)
        self._socket.connected.connect(self._conectado)
        self._socket.readyRead.connect(self._leer)
        self._reintento = QTimer(self, singleShot=True, interval=150)
        self._reintento.timeout.connect(self._conectar)
        self._intentos = 0
        self._arrancar()

    # --- mpv ------------------------------------------------------------------------------------

    def _arrancar(self) -> None:
        mpv = os.environ.get("VORTEX_PLAYER_MPV", "mpv")
        ao = os.environ.get("VORTEX_PLAYER_AO", "pipewire,pulse,alsa")
        self._mpv.start(mpv, ["--idle=yes", "--no-video", "--no-terminal", "--no-config", "--audio-display=no",
                              f"--ao={ao}", "--volume-max=100", f"--volume={self._volumen}",
                              "--replaygain=no", "--audio-pitch-correction=no", "--gapless-audio=weak",
                              "--audio-client-name=Vortex Player", "--pause=yes",
                              f"--input-ipc-server={self._socket_ruta}"])
        self._intentos = 0
        self._reintento.start()

    def _conectar(self) -> None:
        if self._socket.state() != QLocalSocket.LocalSocketState.UnconnectedState:
            return
        self._intentos += 1
        if not Path(self._socket_ruta).exists():
            if self._intentos < 60:
                self._reintento.start()
            else:
                self.error.emit("No pude arrancar mpv (¿está instalado?)")
            return
        self._socket.connectToServer(self._socket_ruta)
        if not self._socket.waitForConnected(500) and self._intentos < 60:
            self._reintento.start()

    def _conectado(self) -> None:
        for pid, nombre in OBSERVAR.items():
            self._enviar({"command": ["observe_property", pid, nombre]})
        for linea in self._pendientes:
            self._socket.write(linea)
        self._pendientes.clear()

    def _mpv_termino(self) -> None:
        # mpv se cerró solo (se cayó): se vuelve a levantar sin perder la cola
        self._socket.abort()
        if not getattr(self, "_cerrando", False):
            self._pausado = True
            self.cambioEstado.emit()
            QTimer.singleShot(500, self._arrancar)

    def cerrar(self) -> None:
        self._cerrando = True
        self._enviar({"command": ["quit"]})
        self._socket.flush()
        if not self._mpv.waitForFinished(1500):
            self._mpv.kill()
        try:
            os.unlink(self._socket_ruta)
        except OSError:
            pass

    def _enviar(self, msg: dict) -> None:
        linea = (json.dumps(msg) + "\n").encode()
        if self._socket.state() == QLocalSocket.LocalSocketState.ConnectedState:
            self._socket.write(linea)
        else:
            self._pendientes.append(linea)

    def _leer(self) -> None:
        self._buffer += bytes(self._socket.readAll())
        *lineas, self._buffer = self._buffer.split(b"\n")
        for linea in lineas:
            try:
                ev = json.loads(linea)
            except ValueError:
                continue
            if ev.get("event") == "property-change":
                self._propiedad(ev.get("name"), ev.get("data"))
            elif ev.get("event") == "end-file":
                if ev.get("reason") == "eof":
                    self._termino_cancion()
                elif ev.get("reason") == "error":
                    self.error.emit("No se pudo tocar: " + Path(self.cancion.get("ruta", "")).name)
                    QTimer.singleShot(300, self._termino_cancion)

    def _propiedad(self, nombre: str, valor) -> None:
        if nombre == "time-pos":
            self._posicion = float(valor or 0)
            self.cambioPosicion.emit()
        elif nombre == "duration":
            self._duracion = float(valor or 0)
            self.cambioPosicion.emit()
        elif nombre == "pause":
            self._pausado = bool(valor)
            self.cambioEstado.emit()

    def _termino_cancion(self) -> None:
        if self._repetir == "una":
            self._cargar()
        elif self._i + 1 < len(self._orden):
            self._i += 1
            self._cargar()
        elif self._repetir == "todo" and self._orden:
            if self._aleatorio:
                random.shuffle(self._orden)
            self._i = 0
            self._cargar()
        else:
            self._pausado = True
            self._posicion = 0.0
            self.cambioEstado.emit()

    def _cargar(self, pausado: bool = False) -> None:
        c = self.cancion
        if not c:
            return
        self._posicion = 0.0
        self._duracion = float(c.get("duracion") or 0)
        self._enviar({"command": ["loadfile", c["ruta"], "replace"]})
        self._enviar({"command": ["set_property", "pause", pausado]})
        self._pausado = pausado
        self.cambioCancion.emit()
        self.cambioEstado.emit()
        self.cambioPosicion.emit()

    # --- para la interfaz y MPRIS ----------------------------------------------------------------

    @Slot("QVariantList", int)
    def tocar(self, canciones: list, inicio: int = 0) -> None:
        """Toca la lista desde la canción `inicio` (con aleatorio, esa primero y luego las demás revueltas)."""
        self._cola = [dict(c) for c in canciones if c and c.get("ruta")]
        if not self._cola:
            return
        inicio = max(0, min(inicio, len(self._cola) - 1))
        self._orden = list(range(len(self._cola)))
        if self._aleatorio:
            resto = [i for i in self._orden if i != inicio]
            random.shuffle(resto)
            self._orden = [inicio] + resto
            self._i = 0
        else:
            self._i = inicio
        self.cambioCola.emit()
        self._cargar()

    @Slot()
    def alternar(self) -> None:
        if not self.cancion:
            return
        self._enviar({"command": ["cycle", "pause"]})

    @Slot()
    def pausar(self) -> None:
        self._enviar({"command": ["set_property", "pause", True]})

    @Slot()
    def seguir(self) -> None:
        if self.cancion:
            self._enviar({"command": ["set_property", "pause", False]})

    @Slot()
    def siguiente(self) -> None:
        if self._i + 1 < len(self._orden):
            self._i += 1
            self._cargar()
        elif self._repetir != "no" and self._orden:
            self._i = 0
            self._cargar()

    @Slot()
    def anterior(self) -> None:
        # Como en todos lados: pasados 3 s vuelve al inicio de la misma
        if self._posicion > 3 or self._i <= 0:
            self.buscar(0)
        else:
            self._i -= 1
            self._cargar()

    @Slot(float)
    def buscar(self, segundos: float) -> None:
        self._enviar({"command": ["seek", max(0.0, segundos), "absolute"]})
        self._posicion = max(0.0, segundos)
        self.cambioPosicion.emit()

    @Slot(int)
    def ponerVolumen(self, v: int) -> None:
        self._volumen = max(0, min(100, int(v)))
        self._enviar({"command": ["set_property", "volume", self._volumen]})
        self.cambioEstado.emit()

    @Slot()
    def alternarAleatorio(self) -> None:
        self._aleatorio = not self._aleatorio
        if self._orden:
            actual = self._orden[self._i]
            if self._aleatorio:
                resto = [i for i in range(len(self._cola)) if i != actual]
                random.shuffle(resto)
                self._orden, self._i = [actual] + resto, 0
            else:
                self._orden, self._i = list(range(len(self._cola))), actual
        self.cambioEstado.emit()

    @Slot()
    def alternarRepetir(self) -> None:
        self._repetir = {"no": "todo", "todo": "una", "una": "no"}[self._repetir]
        self.cambioEstado.emit()

    @Slot(str)
    def quitarDeCola(self, ruta: str) -> None:
        """Si se borra una canción que está en la cola (no la que suena)."""
        actual = self.cancion.get("ruta")
        if ruta == actual:
            return
        self._cola = [c for c in self._cola if c["ruta"] != ruta]
        self._orden = list(range(len(self._cola)))
        self._i = next((i for i, c in enumerate(self._cola) if c["ruta"] == actual), -1)
        self.cambioCola.emit()

    def _cancion(self) -> dict:
        if 0 <= self._i < len(self._orden):
            return self._cola[self._orden[self._i]]
        return {}

    cancion = Property("QVariantMap", _cancion, notify=cambioCancion)
    posicion = Property(float, lambda s: s._posicion, notify=cambioPosicion)
    duracion = Property(float, lambda s: s._duracion, notify=cambioPosicion)
    pausado = Property(bool, lambda s: s._pausado, notify=cambioEstado)
    volumen = Property(int, lambda s: s._volumen, notify=cambioEstado)
    aleatorio = Property(bool, lambda s: s._aleatorio, notify=cambioEstado)
    repetir = Property(str, lambda s: s._repetir, notify=cambioEstado)
    hayAnterior = Property(bool, lambda s: bool(s._orden), notify=cambioCancion)
    haySiguiente = Property(bool, lambda s: s._i + 1 < len(s._orden) or (s._repetir != "no" and bool(s._orden)),
                            notify=cambioCancion)
    cola = Property("QVariantList", lambda s: [s._cola[i] for i in s._orden], notify=cambioCola)
    indice = Property(int, lambda s: s._i, notify=cambioCancion)
