"""El puente entre la ventana (QML) y el resto: dónde se guarda, los ajustes y la última captura."""
from __future__ import annotations

import os
import shutil
import subprocess
import threading
from pathlib import Path

from PySide6.QtCore import Property, QObject, QStandardPaths, QUrl, Signal, Slot
from PySide6.QtGui import QDesktopServices

from . import motor


def _carpeta(ubicacion: QStandardPaths.StandardLocation) -> Path:
    # Para pruebas: VORTEX_VIDEO_CARPETA reemplaza a Imágenes y Vídeos.
    prueba = os.environ.get("VORTEX_VIDEO_CARPETA")
    if prueba:
        return Path(prueba) / ubicacion.name / "Cámara"
    return Path(QStandardPaths.writableLocation(ubicacion)) / "Cámara"


class Puente(QObject):
    mostrarVentana = Signal()
    ultimaCambio = Signal()
    miniaturaLista = Signal(str)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._fotos = _carpeta(QStandardPaths.StandardLocation.PicturesLocation)
        self._videos = _carpeta(QStandardPaths.StandardLocation.MoviesLocation)
        self._ajustes = motor.leer_ajustes()
        self._ultima = ""
        self._miniatura = ""
        self.miniaturaLista.connect(self._poner_miniatura)
        ultima = motor.ultima_captura(self._fotos, self._videos)
        if ultima:
            self._fijar_ultima(str(ultima))

    # --- Ajustes ---
    @Slot(str, result="QVariant")
    def ajuste(self, clave: str):
        return self._ajustes.get(clave)

    @Slot(str, "QVariant")
    def guardar(self, clave: str, valor) -> None:
        if clave in self._ajustes and self._ajustes[clave] != valor:
            self._ajustes[clave] = valor
            motor.guardar_ajustes(self._ajustes)

    # --- Formatos de la cámara ---
    @Slot("QVariantList", str, result=int)
    def elegirFormato(self, formatos: list, preferida: str) -> int:
        return motor.elegir_formato(formatos, preferida)

    @Slot("QVariantList", result="QStringList")
    def resoluciones(self, formatos: list) -> list[str]:
        return motor.resoluciones(formatos) if formatos else []

    @Slot(str, result=str)
    def etiquetaResolucion(self, texto: str) -> str:
        return motor.etiqueta_resolucion(texto)

    @Slot(int, result=str)
    def duracion(self, ms: int) -> str:
        return motor.duracion(ms)

    # --- Capturas ---
    @Slot(result=str)
    def rutaFoto(self) -> str:
        return self._nueva(self._fotos, "Foto", ".jpg")

    @Slot(result=str)
    def urlVideo(self) -> str:
        return QUrl.fromLocalFile(self._nueva(self._videos, "Video", ".mp4")).toString()

    def _nueva(self, carpeta: Path, prefijo: str, extension: str) -> str:
        try:
            carpeta.mkdir(parents=True, exist_ok=True)
        except OSError:
            return ""
        return str(motor.nombre_nuevo(carpeta, prefijo, extension))

    @Slot(str)
    def registrar(self, ruta: str) -> None:
        """Una captura recién hecha: acepta ruta o file://."""
        if ruta.startswith("file:"):
            ruta = QUrl(ruta).toLocalFile()
        if ruta and os.path.exists(ruta):
            self._fijar_ultima(ruta)

    def _fijar_ultima(self, ruta: str) -> None:
        self._ultima = ruta
        if motor.es_video(ruta):
            self._miniatura = ""
            threading.Thread(target=self._hacer_miniatura, args=(ruta,), daemon=True).start()
        else:
            self._miniatura = QUrl.fromLocalFile(ruta).toString()
        self.ultimaCambio.emit()

    def _hacer_miniatura(self, ruta: str) -> None:
        """Un cuadro del video con ffmpeg; sin ffmpeg, el cuadrito muestra solo el ícono de video."""
        if not shutil.which("ffmpeg"):
            return
        base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
        salida = Path(base) / "vortex-video" / "miniatura.jpg"
        try:
            salida.parent.mkdir(parents=True, exist_ok=True)
            r = subprocess.run(["ffmpeg", "-y", "-v", "error", "-i", ruta, "-frames:v", "1", "-vf", "scale=160:-2",
                                str(salida)], capture_output=True, timeout=20)
        except (OSError, subprocess.TimeoutExpired):
            return
        if r.returncode == 0 and ruta == self._ultima:
            self.miniaturaLista.emit(str(salida))

    @Slot(str)
    def _poner_miniatura(self, ruta: str) -> None:
        # Mismo archivo cada vez: el «?» hace que QML no use la imagen vieja de su caché.
        self._miniatura = f"{QUrl.fromLocalFile(ruta).toString()}?{os.stat(ruta).st_mtime_ns}"
        self.ultimaCambio.emit()

    ultima = Property(str, lambda s: s._ultima, notify=ultimaCambio)
    miniatura = Property(str, lambda s: s._miniatura, notify=ultimaCambio)
    ultimaEsVideo = Property(bool, lambda s: motor.es_video(s._ultima) if s._ultima else False, notify=ultimaCambio)

    @Slot()
    def abrirUltima(self) -> None:
        if self._ultima:
            QDesktopServices.openUrl(QUrl.fromLocalFile(self._ultima))

    @Slot(str)
    def abrirCarpeta(self, modo: str) -> None:
        carpeta = self._videos if modo == "video" else self._fotos
        try:
            carpeta.mkdir(parents=True, exist_ok=True)
        except OSError:
            return
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(carpeta)))

    # Para pruebas: hacer de cuenta que no hay cámara, sin importar lo que tenga conectado este equipo.
    sinCamara = Property(bool, lambda s: bool(os.environ.get("VORTEX_VIDEO_SIN_CAMARA")), constant=True)
