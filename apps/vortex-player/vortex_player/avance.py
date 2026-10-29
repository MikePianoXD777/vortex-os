"""El avance (2.1.1): 30 s de una canción de YouTube Music antes de bajarla, desde un poco antes de la mitad.

Es un mpv aparte: no toca la cola ni la canción de turno. Mientras suena, Vortex Player pausa tu música y
al terminar la sigue (eso lo hace el puente con `empezo` y `termino`).

yt-dlp saca el audio con el cliente «visionos» de YouTube: con el de siempre YouTube pide esperar 5 s (un
anuncio) antes de dar el audio. Si ese cliente deja de servir, se intenta una vez con el de siempre.
Para pruebas: VORTEX_PLAYER_AO y VORTEX_PLAYER_MPV, como en el reproductor.
"""
from __future__ import annotations

import os

from PySide6.QtCore import Property, QObject, QProcess, Signal

DURA = 30                       # segundos
CLIENTE = "extractor-args=youtube:player_client=visionos"


class Avance(QObject):
    cambio = Signal()
    empezo = Signal()            # se va a abrir mpv (el puente pausa tu música)
    termino = Signal()           # se acabó o lo paraste (el puente la sigue)
    error = Signal(str)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._id = ""
        self._sonando = False
        self._args: list[str] = []
        self._con_cliente = True
        self._reemplazando = False
        self._p = QProcess(self)
        self._p.setProcessChannelMode(QProcess.ProcessChannelMode.MergedChannels)
        self._p.readyRead.connect(self._leer)
        self._p.finished.connect(self._fin)

    def tocar(self, vid: str, url: str, duracion: float, volumen: int) -> None:
        """Toca el avance de `vid` (si ya sonaba otro, lo cambia sin que tu música se cuele en medio)."""
        if self._id:
            self._reemplazando = True
            self.parar()
            self._reemplazando = False
        else:
            self.empezo.emit()
        inicio = int(duracion * 0.35) if duracion >= 90 else 0
        self._args = ["--no-video", "--no-config", "--msg-level=all=error,cplayer=info", "--term-status-msg=",
                      f"--ao={os.environ.get('VORTEX_PLAYER_AO', 'pipewire,pulse,alsa')}", "--volume-max=100",
                      f"--volume={max(0, min(100, int(volumen)))}", "--ytdl-format=bestaudio",
                      f"--start={inicio}", f"--end={inicio + DURA}", "--audio-client-name=Vortex Player", url]
        self._id, self._sonando, self._con_cliente = vid, False, True
        self._arrancar()
        self.cambio.emit()

    def _arrancar(self) -> None:
        extra = [f"--ytdl-raw-options={CLIENTE}"] if self._con_cliente else []
        self._p.start(os.environ.get("VORTEX_PLAYER_MPV", "mpv"), extra + self._args)

    def parar(self) -> None:
        if self._p.state() != QProcess.ProcessState.NotRunning:
            self._p.kill()
            self._p.waitForFinished(1500)

    def _leer(self) -> None:
        # mpv dice «AO: [pipewire] 48000Hz…» cuando abre la salida de audio: ya suena
        if not self._sonando and b"AO:" in bytes(self._p.readAll()):
            self._sonando = True
            self.cambio.emit()

    def _fin(self, codigo: int, estado: QProcess.ExitStatus) -> None:
        if self._reemplazando:
            return
        fallo = not self._sonando and estado == QProcess.ExitStatus.NormalExit and codigo != 0
        if fallo and self._con_cliente:
            self._con_cliente = False
            self._arrancar()
            return
        if fallo:
            self.error.emit("No pude escuchar el avance")
        self._id, self._sonando = "", False
        self.cambio.emit()
        self.termino.emit()

    idActual = Property(str, lambda s: s._id, notify=cambio)
    sonando = Property(bool, lambda s: s._sonando, notify=cambio)
