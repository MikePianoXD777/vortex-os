"""El reproductor: mpv sin ventana, manejado por su socket JSON (IPC).

mpv toca cada archivo a su frecuencia y bits originales por PipeWire (sin remuestrear, si PipeWire
deja cambiar de frecuencia: ver audio.config_hires) y sin normalizar el volumen. La cola, el aleatorio
y el repetir los lleva Vortex Player; mpv solo toca la canción de turno.

2.4: la cola «A continuación» (tocar después / agregar a la cola), el temporizador para dormir y el
crossfade: los últimos segundos de una canción suenan en otro mpv que se va apagando (afade) mientras la
siguiente entra subiendo, en el mpv principal.

1.4: la radio sin fin (al acabarse la cola avisa con `seAcaba` y el puente le agrega parecidas de YouTube
Music, que suenan sin bajarlas) y cuánto escuchaste de cada canción (`escuchada`) para «Tu resumen».

Para pruebas: VORTEX_PLAYER_AO=null no saca sonido y VORTEX_PLAYER_MPV cambia el mpv que se usa.
"""
from __future__ import annotations

import json
import os
import random
import tempfile
import time
from pathlib import Path

from PySide6.QtCore import Property, QObject, QProcess, QTimer, Signal, Slot
from PySide6.QtNetwork import QLocalSocket

from . import plataforma

OBSERVAR = {1: "time-pos", 2: "duration", 3: "pause", 4: "idle-active"}
CLIENTE = "extractor-args=youtube:player_client=visionos"   # el mismo que el avance (avance.CLIENTE)


def es_stream(ruta: str) -> bool:
    """1.4: una canción de la radio, que suena de YouTube sin estar en tu biblioteca."""
    return ruta.startswith(("http://", "https://"))


class Reproductor(QObject):
    cambioCancion = Signal()
    cambioEstado = Signal()
    cambioPosicion = Signal()
    cambioCola = Signal()
    quiereSonar = Signal()      # le diste play (o tocar, siguiente…): el avance de una canción se calla (2.1.1)
    cambioDormir = Signal()
    error = Signal(str)
    seAcaba = Signal()                      # 1.4: suena la última de la cola (o ya terminó): pide radio
    escuchada = Signal("QVariantMap", float)   # 1.4: (canción, segundos que de verdad sonó)

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
        self._origen = ""               # 2.4: de dónde suena («Tu biblioteca» o el nombre de la playlist)
        self._despues = 0               # 2.4: cuántas «tocar después» van detrás de la actual
        self._fundido = 0.0             # 2.4: segundos de crossfade (0 = sin)
        self._eq = ""                   # 2.5: el ecualizador dentro de mpv (Windows), como `af`
        self._cola_fundido: list[QProcess] = []
        self._final = False             # 1.4: la cola se acabó y quedó parada en la última
        self._sonando: dict = {}        # 1.4: la canción a la que se le cuentan los segundos
        self._escuchado = 0.0
        self._sin_cliente = False       # 1.4: la de YouTube ya falló con el cliente visionos
        self._cargada = 0.0             # cuándo se cargó la canción (time.monotonic)
        self._dormir_hasta = 0.0        # 2.4: temporizador (time.monotonic); 0 = apagado
        self._dormir_al_final = False
        self._apagando = False
        self._reloj_dormir = QTimer(self, interval=500)
        self._reloj_dormir.timeout.connect(self._revisar_dormir)
        self._pendientes: list[bytes] = []
        self._buffer = b""
        carpeta = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
        self._ipc, self._socket_ruta = plataforma.canal_mpv(f"vortex-player-mpv-{os.getpid()}", carpeta)
        if len(self._socket_ruta.encode()) > 100:   # los sockets de Unix no aceptan rutas de más de 108 bytes
            self._ipc, self._socket_ruta = plataforma.canal_mpv(
                f"vortex-player-mpv-{plataforma.usuario()}-{os.getpid()}", tempfile.gettempdir())
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
        ao = plataforma.salida_audio()
        self._mpv.start(mpv, ["--idle=yes", "--no-video", "--no-terminal", "--no-config", "--audio-display=no",
                              f"--ao={ao}", "--volume-max=100", f"--volume={self._volumen}",
                              "--replaygain=no", "--audio-pitch-correction=no", "--gapless-audio=weak",
                              "--audio-client-name=Vortex Player", "--pause=yes",
                              "--ytdl-format=bestaudio",   # 1.4: la radio suena de YouTube sin bajarla
                              f"--input-ipc-server={self._ipc}"])
        self._intentos = 0
        self._reintento.start()

    def _conectar(self) -> None:
        if self._socket.state() != QLocalSocket.LocalSocketState.UnconnectedState:
            return
        self._intentos += 1
        if not plataforma.WINDOWS and not Path(self._socket_ruta).exists():   # la tubería de Windows no es archivo
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
        if self._eq:
            self._enviar({"command": ["set_property", "af", self._eq]})
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
        self._cerrar_escucha()
        self._parar_fundido()
        self._enviar({"command": ["quit"]})
        self._socket.flush()
        if not self._mpv.waitForFinished(1500):
            self._mpv.kill()
        if not plataforma.WINDOWS:
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
                    c = self.cancion
                    if es_stream(c.get("ruta", "")) and not self._sin_cliente:   # como el avance: otra vez sin cliente
                        self._sin_cliente = True
                        self._cargar()
                        continue
                    self.error.emit("No se pudo tocar: " + (c.get("titulo") or Path(c.get("ruta", "")).name))
                    QTimer.singleShot(300, self._termino_cancion)

    def _propiedad(self, nombre: str, valor) -> None:
        if nombre == "time-pos":
            nueva = float(valor or 0)
            if not self._pausado and 0 < nueva - self._posicion < 3:   # lo que sí sonó (no los saltos)
                self._escuchado += nueva - self._posicion
            self._posicion = nueva
            self.cambioPosicion.emit()
            self._revisar_fundido()
        elif nombre == "duration":
            self._duracion = float(valor or 0)
            self.cambioPosicion.emit()
        elif nombre == "pause":
            self._pausado = bool(valor)
            if self._pausado:
                self._parar_fundido()
            self.cambioEstado.emit()

    def _termino_cancion(self) -> None:
        if self._dormir_al_final:   # 2.4: «al terminar la canción»: queda la siguiente, en pausa
            self._dormir_al_final = False
            self._reloj_dormir.stop()
            self.cambioDormir.emit()
            if self._i + 1 < len(self._orden):
                self._i += 1
                self._despues = max(0, self._despues - 1)
            self._cargar(pausado=True)
            return
        if self._repetir == "una":
            self._cargar()
        elif self._i + 1 < len(self._orden):
            self._i += 1
            self._despues = max(0, self._despues - 1)
            self._cargar()
        elif self._repetir == "todo" and self._orden:
            if self._aleatorio:
                random.shuffle(self._orden)
            self._i = 0
            self._cargar()
        else:
            self._pausado = True
            self._posicion = 0.0
            self._final = True
            self._cerrar_escucha()
            self.cambioEstado.emit()
            self.seAcaba.emit()

    def _cerrar_escucha(self) -> None:
        """1.4: cuenta como escuchada si sonó 30 s (o la mitad, si es más corta)."""
        c, s = self._sonando, self._escuchado
        self._sonando, self._escuchado = {}, 0.0
        dur = float(c.get("duracion") or 0) or self._duracion
        if c and s >= min(30.0, dur * 0.5 if dur > 0 else 30.0):
            self.escuchada.emit(dict(c), round(s, 1))

    def _cargar(self, pausado: bool = False, desde: float = 0.0, entra: float = 0.0) -> None:
        """`entra`: segundos en que sube desde el silencio (la que entra con crossfade)."""
        c = self._cancion()
        if not c:
            return
        if c.get("ruta") != self._sonando.get("ruta"):
            self._sin_cliente = False
        self._cerrar_escucha()
        self._sonando = c
        self._final = False
        self._posicion = desde
        self._duracion = float(c.get("duracion") or 0)
        self._cargada = time.monotonic()
        opciones = {}   # mpv 0.38+: loadfile <url> <flags> <índice> <opciones> (solo para este archivo)
        if desde > 0:
            opciones["start"] = f"{desde:.2f}"
        if entra > 0:
            opciones["af"] = ",".join(x for x in (self._eq, f"lavfi=[afade=t=in:d={entra:.1f}]") if x)
        if es_stream(c["ruta"]) and not self._sin_cliente:
            opciones["ytdl-raw-options"] = CLIENTE
        if opciones:
            self._enviar({"command": ["loadfile", c["ruta"], "replace", -1, opciones]})
        else:
            self._enviar({"command": ["loadfile", c["ruta"], "replace"]})
        self._enviar({"command": ["set_property", "pause", pausado]})
        self._pausado = pausado
        if not pausado:
            self.quiereSonar.emit()
        self.cambioCancion.emit()
        self.cambioCola.emit()   # la cola «A continuación» empieza en otro lado
        self.cambioEstado.emit()
        self.cambioPosicion.emit()
        if self._repetir == "no" and self._i + 1 >= len(self._orden):
            self.seAcaba.emit()

    # --- para la interfaz y MPRIS ----------------------------------------------------------------

    @Slot("QVariantList", int)
    def tocar(self, canciones: list, inicio: int = 0) -> None:
        """Toca la lista desde la canción `inicio` (con aleatorio, esa primero y luego las demás revueltas)."""
        self._cola = [dict(c) for c in canciones if c and c.get("ruta")]
        if not self._cola:
            return
        inicio = max(0, min(inicio, len(self._cola) - 1))
        self._despues = 0
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

    # --- 2.4: seguir donde te quedaste ----------------------------------------------------------------

    def sesion(self) -> dict:
        """Lo que hace falta para volver a abrir la app y seguir aquí."""
        return {"rutas": [c["ruta"] for c in self._cola], "orden": list(self._orden), "i": self._i,
                "posicion": round(self._posicion, 2), "aleatorio": self._aleatorio, "repetir": self._repetir,
                "origen": self._origen}

    def restaurar(self, cola: list[dict], s: dict) -> bool:
        """Pone la cola guardada en pausa, en la canción y el segundo donde ibas. `cola`: las canciones de
        s["rutas"] que siguen en tu biblioteca (en el mismo orden; las que ya no están se brincan)."""
        if not cola:
            return False
        rutas = [c["ruta"] for c in cola]
        guardadas = s.get("rutas") or []
        orden_viejo = s.get("orden") or list(range(len(guardadas)))
        i = int(s.get("i", 0))
        actual = guardadas[orden_viejo[i]] if 0 <= i < len(orden_viejo) and orden_viejo[i] < len(guardadas) else None
        # el orden guardado (con aleatorio) traducido a la cola nueva
        orden = [rutas.index(guardadas[k]) for k in orden_viejo if k < len(guardadas) and guardadas[k] in rutas]
        if sorted(orden) != list(range(len(cola))):
            orden = list(range(len(cola)))
        self._cola, self._orden = [dict(c) for c in cola], orden
        self._i = next((k for k, j in enumerate(orden) if cola[j]["ruta"] == actual), 0)
        self._aleatorio = bool(s.get("aleatorio", False))
        self._repetir = s.get("repetir") if s.get("repetir") in ("no", "todo", "una") else "no"
        self._origen = str(s.get("origen") or "")
        self.cambioCola.emit()
        self._cargar(pausado=True, desde=float(s.get("posicion") or 0) if self.cancion.get("ruta") == actual else 0.0)
        return True

    @Slot(str)
    def ponerOrigen(self, origen: str) -> None:
        self._origen = origen
        self.cambioCola.emit()

    @Slot()
    def alternar(self) -> None:
        if not self.cancion:
            return
        if self._pausado:
            self.quiereSonar.emit()
        self._enviar({"command": ["cycle", "pause"]})

    @Slot()
    def pausar(self) -> None:
        self._enviar({"command": ["set_property", "pause", True]})

    @Slot()
    def seguir(self) -> None:
        if self.cancion:
            self.quiereSonar.emit()
            self._enviar({"command": ["set_property", "pause", False]})

    @Slot()
    def siguiente(self) -> None:
        if self._i + 1 < len(self._orden):
            self._i += 1
            self._despues = max(0, self._despues - 1)
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
            self._despues = 0
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

    # --- 2.4: la cola «A continuación» --------------------------------------------------------------

    @Slot("QVariantMap")
    def tocarDespues(self, c: dict) -> None:
        """Suena justo después de la actual (y de las que ya pusiste con «tocar después»)."""
        if not c or not c.get("ruta"):
            return
        if not self._orden:
            self.tocar([c], 0)
            return
        self._cola.append(dict(c))
        self._orden.insert(self._i + 1 + self._despues, len(self._cola) - 1)
        self._despues += 1
        self.cambioCola.emit()

    @Slot("QVariantMap")
    def agregarACola(self, c: dict) -> None:
        """Al final de la cola."""
        if not c or not c.get("ruta"):
            return
        if not self._orden:
            self.tocar([c], 0)
            return
        self._cola.append(dict(c))
        self._orden.append(len(self._cola) - 1)
        self.cambioCola.emit()

    @Slot("QVariantList")
    def agregarRadio(self, canciones: list) -> None:
        """1.4: la radio sin fin: al final de la cola; si ya se había acabado, sigue sonando con la primera."""
        nuevas = [dict(c) for c in canciones if c and c.get("ruta")]
        if not nuevas or not self._orden:
            return
        for c in nuevas:
            self._cola.append(c)
            self._orden.append(len(self._cola) - 1)
        self.cambioCola.emit()
        if self._final:
            self._i += 1
            self._cargar()

    @Slot(int)
    def saltarA(self, k: int) -> None:
        """Toca la canción k de la cola (k: su lugar en `cola`)."""
        if 0 <= k < len(self._orden):
            self._i, self._despues = k, 0
            self._cargar()

    @Slot(int)
    def quitarDeColaEn(self, k: int) -> None:
        """Quita de la cola la k (una que viene, no la que suena)."""
        if self._i < k < len(self._orden):
            if k <= self._i + self._despues:
                self._despues -= 1
            del self._orden[k]
            self.cambioCola.emit()

    # --- 2.4: temporizador para dormir ------------------------------------------------------------

    @Slot(int)
    def dormir(self, minutos: int) -> None:
        """Pausa en `minutos` (bajando el volumen los últimos 10 s); -1: al terminar la canción; 0: apagado."""
        if self._apagando:
            self._enviar({"command": ["set_property", "volume", self._volumen]})
        self._apagando = False
        self._dormir_al_final = minutos < 0
        self._dormir_hasta = time.monotonic() + minutos * 60 if minutos > 0 else 0.0
        if minutos:
            self._reloj_dormir.start()
        else:
            self._reloj_dormir.stop()
        self.cambioDormir.emit()

    def _revisar_dormir(self) -> None:
        if not self._dormir_hasta:
            return
        falta = self._dormir_hasta - time.monotonic()
        if falta <= 0:
            self._dormir_hasta = 0.0
            self._reloj_dormir.stop()
            self.pausar()
            self._enviar({"command": ["set_property", "volume", self._volumen]})
            self._apagando = False
        elif falta <= 10 and not self._pausado:
            self._apagando = True
            self._enviar({"command": ["set_property", "volume", round(self._volumen * falta / 10)]})
        self.cambioDormir.emit()

    def _dormir_texto(self) -> str:
        if self._dormir_al_final:
            return "Al terminar la canción"
        if self._dormir_hasta:
            falta = max(0, self._dormir_hasta - time.monotonic())
            return f"{int(falta // 60)}:{int(falta % 60):02d}"
        return ""

    # --- 2.4: crossfade ---------------------------------------------------------------------------

    @Slot(float)
    def ponerEq(self, filtro: str) -> None:
        """2.5: el ecualizador dentro de mpv (`af`; «» = sin filtros)."""
        if filtro == self._eq:
            return
        self._eq = filtro
        self._enviar({"command": ["set_property", "af", filtro]})

    def ponerFundido(self, segundos: float) -> None:
        self._fundido = max(0.0, min(12.0, float(segundos)))
        self.cambioEstado.emit()

    def _siguiente_indice(self) -> int:
        if self._i + 1 < len(self._orden):
            return self._i + 1
        return 0 if self._repetir == "todo" and len(self._orden) > 1 else -1

    def _revisar_fundido(self) -> None:
        """A `_fundido` segundos del final: la que se va sigue en otro mpv apagándose y la siguiente entra."""
        n = self._fundido
        if (n <= 0 or self._pausado or self._repetir == "una" or self._dormir_al_final or self._duracion < n * 3
                or time.monotonic() - self._cargada < 2 or self._duracion - self._posicion > n):
            return
        k = self._siguiente_indice()
        c = self.cancion
        if k < 0 or not c:
            return
        if es_stream(c["ruta"]) or es_stream(self._cola[self._orden[k]]["ruta"]):
            return   # 1.4: la de YouTube tarda en abrir: se pasa sin crossfade
        desde, resto = self._posicion, max(0.5, self._duracion - self._posicion)
        mpv = os.environ.get("VORTEX_PLAYER_MPV", "mpv")
        ao = plataforma.salida_audio()
        cola = QProcess(self)
        cola.finished.connect(lambda *_: cola in self._cola_fundido and self._cola_fundido.remove(cola))
        cola.start(mpv, ["--no-video", "--no-terminal", "--no-config", "--audio-display=no", f"--ao={ao}",
                         f"--volume={self._volumen}", "--volume-max=100", "--replaygain=no",
                         "--audio-client-name=Vortex Player", f"--start={desde:.2f}",
                         "--af=" + ",".join(x for x in (self._eq, f"lavfi=[afade=t=out:st={desde:.2f}:d={resto:.2f}]") if x),
                         c["ruta"]])
        self._cola_fundido.append(cola)
        if k == 0 and self._aleatorio:
            random.shuffle(self._orden)
        self._i = k
        self._despues = max(0, self._despues - 1)
        self._cargar(entra=min(n, resto))

    def _parar_fundido(self) -> None:
        for p in list(self._cola_fundido):
            p.kill()
        self._cola_fundido.clear()

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
    hayAnterior = Property(bool, lambda s: bool(s._orden), notify=cambioCola)
    haySiguiente = Property(bool, lambda s: s._i + 1 < len(s._orden) or (s._repetir != "no" and bool(s._orden)),
                            notify=cambioCola)
    cola = Property("QVariantList", lambda s: [s._cola[i] for i in s._orden], notify=cambioCola)
    dormirTexto = Property(str, _dormir_texto, notify=cambioDormir)
    dormirActivo = Property(bool, lambda s: bool(s._dormir_hasta or s._dormir_al_final), notify=cambioDormir)
    fundido = Property(float, lambda s: s._fundido, notify=cambioEstado)
    indice = Property(int, lambda s: s._i, notify=cambioCola)
    origen = Property(str, lambda s: s._origen, notify=cambioCola)
