#!/usr/bin/env python3
"""Vortexine Video con una cámara de verdad (o la falsa de la VM), sin ventana: toma una foto, graba 3 segundos
y revisa con ffprobe que los archivos sirvan. Guarda en una carpeta temporal y no toca tus ajustes.

    pruebas/vortex-video-camara.py [--sin-microfono]
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent / "apps" / "vortex-video"
if not (RAIZ / "vortex_video").is_dir():
    RAIZ = Path("/usr/share/vortex-video")
sys.path.insert(0, str(RAIZ))
T = tempfile.mkdtemp(prefix="vortex-video-")
os.environ.update(QT_QPA_PLATFORM=os.environ.get("QT_QPA_PLATFORM", "offscreen"), VORTEX_VIDEO_CARPETA=T, XDG_CONFIG_HOME=T, XDG_CACHE_HOME=T)

from PySide6.QtCore import QMetaObject, QObject, Qt, QTimer  # noqa: E402
from vortex_video.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402

app = crear_aplicacion(["vv"])
motor, puente = cargar_interfaz(app)
v = motor.rootObjects()[0]
if "--sin-microfono" in sys.argv:
    v.setProperty("microfono", False)
fallos: list[str] = []
pasos = 0


def archivos(sufijo: str) -> list[Path]:
    return sorted(Path(T).rglob(f"*{sufijo}"))


def revisar(ruta: Path) -> dict:
    r = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "stream=codec_type,codec_name,width,height:format=duration",
                        "-of", "json", str(ruta)], capture_output=True, text=True)
    return json.loads(r.stdout or "{}") if r.returncode == 0 else {}


def disparar() -> None:
    QMetaObject.invokeMethod(v, "disparar", Qt.ConnectionType.DirectConnection)


def esperar_lista() -> None:
    global pasos
    pasos += 1
    if v.property("lista") and v.property("formatosPlanos"):
        print("cámara:", v.findChild(QObject, "panelAjustes").property("camaras"))
        print("resoluciones:", v.property("resoluciones"))
        QTimer.singleShot(2000, foto)   # que lleguen cuadros
    elif pasos > 40:
        fallos.append(f"la cámara no quedó lista: {v.property('errorCamara')!r}, hay cámara: {v.property('hayCamara')}")
        terminar()
    else:
        QTimer.singleShot(250, esperar_lista)


def foto() -> None:
    v.setProperty("modo", "foto")
    disparar()
    QTimer.singleShot(2500, revisar_foto)


def revisar_foto() -> None:
    fotos = archivos(".jpg")
    if not fotos:
        fallos.append("no se guardó la foto")
    else:
        info = revisar(fotos[0])
        print("foto:", fotos[0].name, info.get("streams"))
        if not info.get("streams"):
            fallos.append("la foto no abre")
        if puente.ultima != str(fotos[0]):
            fallos.append("la foto no quedó como última captura")
    v.setProperty("modo", "video")
    disparar()
    QTimer.singleShot(3500, parar)


def parar() -> None:
    if not v.property("grabando"):
        fallos.append("no estaba grabando")
    disparar()
    QTimer.singleShot(3000, revisar_video)


def revisar_video() -> None:
    videos = archivos(".mp4")
    if not videos:
        fallos.append("no se guardó el video")
    else:
        info = revisar(videos[0])
        duracion = float(info.get("format", {}).get("duration", 0))
        print("video:", videos[0].name, f"{duracion:.1f} s", info.get("streams"))
        if duracion < 2:
            fallos.append(f"el video dura {duracion:.1f} s")
        if not any(s.get("codec_type") == "video" for s in info.get("streams", [])):
            fallos.append("el video no tiene imagen")
        if not puente.ultimaEsVideo:
            fallos.append("el video no quedó como última captura")
    terminar()


def terminar() -> None:
    shutil.rmtree(T, ignore_errors=True)
    print("MAL: " + "; ".join(fallos) if fallos else "Todo bien: foto y video.")
    sys.stdout.flush()
    os._exit(1 if fallos else 0)


QTimer.singleShot(0, esperar_lista)
QTimer.singleShot(60000, lambda: (fallos.append("se pasó del tiempo"), terminar()))
app.exec()
