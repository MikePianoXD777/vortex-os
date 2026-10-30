"""Fotos y videos de las cámaras, con ffmpeg y sin recomprimir el video.

La foto se saca del flujo principal (la resolución completa de la cámara, no la del mosaico) y va a
~/Imágenes/Vortexine's Camera; los videos, a ~/Vídeos/Vortexine's Camera, en MP4 fragmentado: si la PC
se apaga a media grabación, lo grabado hasta ahí se puede ver. El audio de las Dahua suele venir en
G.711, que el MP4 no acepta: se pasa a AAC (el video se copia tal cual).
"""
from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
from datetime import datetime
from pathlib import Path

CARPETA = "Vortexine's Camera"


def _carpeta_xdg(tipo: str, respaldo: str) -> Path:
    try:
        r = subprocess.run(["xdg-user-dir", tipo], capture_output=True, text=True, timeout=3)
        ruta = r.stdout.strip()
    except (OSError, subprocess.TimeoutExpired):
        ruta = ""
    base = Path(ruta) if ruta and ruta != str(Path.home()) else Path.home() / respaldo
    return base / CARPETA


def carpeta_fotos() -> Path:
    return _carpeta_xdg("PICTURES", "Imágenes")


def carpeta_videos() -> Path:
    return _carpeta_xdg("VIDEOS", "Vídeos")


def nombre_archivo(nombre: str, extension: str, cuando: datetime | None = None) -> str:
    """«Patio» → «Patio 2026-10-30 14.05.33.jpg» (sin / ni caracteres que estorben)."""
    limpio = re.sub(r'[/\\:*?"<>|\x00-\x1f]+', " ", nombre).strip(" .") or "Cámara"
    return f"{limpio[:60]} {(cuando or datetime.now()).strftime('%Y-%m-%d %H.%M.%S')}.{extension}"


def _libre(ruta: Path) -> Path:
    n, nueva = 2, ruta
    while nueva.exists():
        nueva = ruta.with_name(f"{ruta.stem} ({n}){ruta.suffix}")
        n += 1
    return nueva


def _ffmpeg() -> str:
    ruta = shutil.which("ffmpeg")
    if not ruta:
        raise RuntimeError("Falta ffmpeg (sudo pacman -S ffmpeg).")
    return ruta


def foto(url: str, carpeta: Path, nombre: str, espera: float = 20) -> Path:
    """Un cuadro del video, en JPG. Error (RuntimeError) con un texto para la persona si no se pudo."""
    carpeta.mkdir(parents=True, exist_ok=True)
    destino = _libre(carpeta / nombre_archivo(nombre, "jpg"))
    orden = [_ffmpeg(), "-hide_banner", "-loglevel", "error", "-nostdin", "-rtsp_transport", "tcp",
             "-i", url, "-frames:v", "1", "-q:v", "2", "-y", str(destino)]
    try:
        r = subprocess.run(orden, capture_output=True, text=True, timeout=espera)
    except subprocess.TimeoutExpired:
        destino.unlink(missing_ok=True)
        raise RuntimeError("La cámara no mandó imagen a tiempo.") from None
    if r.returncode != 0 or not destino.exists() or destino.stat().st_size == 0:
        destino.unlink(missing_ok=True)
        raise RuntimeError(explicar(r.stderr))
    return destino


def explicar(error: str) -> str:
    """El error de ffmpeg → algo que se entienda."""
    e = error.lower()
    if "401" in e or "unauthorized" in e:
        return "La cámara no aceptó el código de seguridad."
    if "timed out" in e or "connection refused" in e or "no route" in e or "unreachable" in e:
        return "No se pudo conectar con la cámara."
    if "404" in e or "not found" in e:
        return "La cámara no tiene ese video."
    return "No se pudo leer el video de la cámara."


class Grabacion:
    """Un ffmpeg grabando una cámara. `detener` le manda «q» para que cierre bien el archivo."""

    def __init__(self, url: str, carpeta: Path, nombre: str):
        carpeta.mkdir(parents=True, exist_ok=True)
        self.destino = _libre(carpeta / nombre_archivo(nombre, "mp4"))
        orden = [_ffmpeg(), "-hide_banner", "-loglevel", "error", "-rtsp_transport", "tcp", "-i", url,
                 "-map", "0:v:0", "-map", "0:a:0?", "-c:v", "copy", "-c:a", "aac", "-b:a", "96k",
                 "-movflags", "+frag_keyframe+empty_moov+default_base_moof", "-f", "mp4", str(self.destino)]
        # Los errores van a un archivo y no a una tubería: en horas de grabación una tubería llena
        # dejaría a ffmpeg esperando.
        self._errores = tempfile.TemporaryFile()
        self._proceso = subprocess.Popen(orden, stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                                         stderr=self._errores)

    @property
    def activa(self) -> bool:
        return self._proceso.poll() is None

    def error(self) -> str:
        """Si ffmpeg ya terminó solo (la cámara se cayó, código malo…), por qué."""
        if self.activa or self._errores.closed:
            return ""
        self._errores.seek(0)
        return explicar(self._errores.read().decode("utf-8", "replace"))

    def detener(self, espera: float = 8) -> Path | None:
        """Cierra el archivo. Devuelve la ruta si quedó algo grabado."""
        if self.activa:
            try:
                self._proceso.stdin.write(b"q")
                self._proceso.stdin.flush()
            except (BrokenPipeError, OSError):
                pass
            try:
                self._proceso.wait(espera)
            except subprocess.TimeoutExpired:
                self._proceso.terminate()
                try:
                    self._proceso.wait(3)
                except subprocess.TimeoutExpired:
                    self._proceso.kill()
                    self._proceso.wait()
        if self._proceso.stdin:
            self._proceso.stdin.close()
        self._errores.close()
        if self.destino.exists() and self.destino.stat().st_size > 0:
            return self.destino
        self.destino.unlink(missing_ok=True)
        return None
