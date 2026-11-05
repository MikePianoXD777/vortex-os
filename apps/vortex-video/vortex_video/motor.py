"""Lo que no necesita ni Qt ni cámara: nombres de archivo, ajustes y elegir el formato de la cámara."""
from __future__ import annotations

import json
import os
from datetime import datetime
from pathlib import Path

EXT_FOTO = (".jpg", ".jpeg", ".png")
EXT_VIDEO = (".mp4", ".mkv", ".webm")
FPS_FLUIDO = 24   # con menos, una resolución se ve a saltos: solo se usa si no hay otra

AJUSTES_POR_DEFECTO = {"modo": "foto", "temporizador": 0, "microfono": True, "cuadricula": False,
                       "camara": "", "resolucion": ""}


def ruta_ajustes() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(base) / "vortex" / "video.json"


def leer_ajustes(ruta: Path | None = None) -> dict:
    ajustes = dict(AJUSTES_POR_DEFECTO)
    try:
        datos = json.loads((ruta or ruta_ajustes()).read_text())
    except (OSError, ValueError):
        return ajustes
    if isinstance(datos, dict):
        for clave, defecto in AJUSTES_POR_DEFECTO.items():
            if type(datos.get(clave)) is type(defecto):
                ajustes[clave] = datos[clave]
    if ajustes["modo"] not in ("foto", "video"):
        ajustes["modo"] = "foto"
    if ajustes["temporizador"] not in (0, 3, 5, 10):
        ajustes["temporizador"] = 0
    return ajustes


def guardar_ajustes(ajustes: dict, ruta: Path | None = None) -> None:
    ruta = ruta or ruta_ajustes()
    try:
        ruta.parent.mkdir(parents=True, exist_ok=True)
        temporal = ruta.with_suffix(".tmp")
        temporal.write_text(json.dumps(ajustes, ensure_ascii=False, indent=1))
        temporal.replace(ruta)
    except OSError:
        pass   # sin ajustes guardados la cámara sigue sirviendo


def nombre_nuevo(carpeta: Path, prefijo: str, extension: str, ahora: datetime | None = None) -> Path:
    """«Foto 2026-11-05 14.03.21.jpg»; si ya existe uno de ese segundo, «… (2).jpg»."""
    sello = (ahora or datetime.now()).strftime("%Y-%m-%d %H.%M.%S")
    ruta = carpeta / f"{prefijo} {sello}{extension}"
    n = 2
    while ruta.exists():
        ruta = carpeta / f"{prefijo} {sello} ({n}){extension}"
        n += 1
    return ruta


def ultima_captura(*carpetas: Path) -> Path | None:
    """La captura más nueva de esas carpetas: se ve en el cuadrito de abajo al abrir la app."""
    mejor, cuando = None, -1.0
    for carpeta in carpetas:
        try:
            archivos = list(carpeta.iterdir())
        except OSError:
            continue
        for a in archivos:
            if a.suffix.lower() in EXT_FOTO + EXT_VIDEO:
                try:
                    t = a.stat().st_mtime
                except OSError:
                    continue
                if t > cuando:
                    mejor, cuando = a, t
    return mejor


def es_video(ruta: str | Path) -> bool:
    return Path(ruta).suffix.lower() in EXT_VIDEO


def resolucion(formato: dict) -> str:
    return f"{int(formato['ancho'])}x{int(formato['alto'])}"


def resoluciones(formatos: list[dict]) -> list[str]:
    """Las que se ven fluidas, de mayor a menor. Si la cámara no tiene ninguna fluida, todas."""
    fluidas = [f for f in formatos if f.get("fps", 0) >= FPS_FLUIDO] or formatos
    unicas = {resolucion(f): int(f["ancho"]) * int(f["alto"]) for f in fluidas}
    return sorted(unicas, key=lambda r: -unicas[r])


def elegir_formato(formatos: list[dict], preferida: str = "") -> int:
    """El índice del formato a usar: la resolución que elegiste, si la cámara la tiene; si no, la mayor
    fluida. Una misma resolución puede venir en varios (a 5 cuadros sin comprimir y a 30 en MJPEG): gana el
    de más cuadros por segundo. -1 si la cámara no dice ninguno."""
    if not formatos:
        return -1
    opciones = resoluciones(formatos)
    objetivo = preferida if preferida in opciones else opciones[0]
    return max((i for i, f in enumerate(formatos) if resolucion(f) == objetivo),
               key=lambda i: formatos[i].get("fps", 0))


def etiqueta_resolucion(texto: str) -> str:
    """«1920x1080» → «1920 × 1080 (Full HD)»."""
    ancho, _, alto = texto.partition("x")
    nombres = {"3840x2160": "4K", "2560x1440": "2K", "1920x1080": "Full HD", "1280x720": "HD"}
    base = f"{ancho} × {alto}"
    return f"{base} ({nombres[texto]})" if texto in nombres else base


def duracion(ms: int) -> str:
    s = max(0, int(ms)) // 1000
    return f"{s // 3600}:{s // 60 % 60:02d}:{s % 60:02d}" if s >= 3600 else f"{s // 60:02d}:{s % 60:02d}"
