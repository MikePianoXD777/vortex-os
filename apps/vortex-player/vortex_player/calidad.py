"""La calidad real de cada canción: qué códec, cuántos bits y a qué frecuencia, y su etiqueta.

Sin pérdida (FLAC, ALAC, WAV, AIFF, WavPack, APE) con más de 16 bits o más de 48 kHz es «Hi-Res»;
a 16 bits y 44,1/48 kHz es calidad «CD». Lo demás (Opus, AAC, MP3, Vorbis) dice su códec y sus kbps.
Los datos salen de ffprobe (de ffmpeg), que lee cualquier formato.
"""
from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

SIN_PERDIDA = {"flac", "alac", "wavpack", "ape", "tta", "mlp", "truehd", "dsd"}
EXTENSIONES = {".flac", ".wav", ".wave", ".aif", ".aiff", ".aifc", ".m4a", ".alac", ".mp3", ".ogg",
               ".oga", ".opus", ".wv", ".ape", ".aac", ".webm", ".mka", ".dsf", ".dff", ".tta"}
NOMBRES = {"opus": "Opus", "aac": "AAC", "mp3": "MP3", "vorbis": "Vorbis", "flac": "FLAC", "alac": "ALAC",
           "wavpack": "WavPack", "ape": "APE", "pcm": "WAV", "dsd": "DSD", "tta": "TTA"}


def es_sin_perdida(codec: str) -> bool:
    return codec in SIN_PERDIDA or codec.startswith("pcm_") or codec.startswith("dsd_")


def familia(codec: str) -> str:
    if codec.startswith("pcm_"):
        return "pcm"
    if codec.startswith("dsd_"):
        return "dsd"
    return codec


def _khz(hz: int) -> str:
    k = hz / 1000
    return f"{k:g}".replace(".", ",") if k != int(k) else str(int(k))


def etiqueta(codec: str, bits: int, hz: int, kbps: int) -> str:
    """«Hi-Res 24/96», «CD 16/44,1», «Opus 160 kbps»…"""
    if es_sin_perdida(codec):
        if bits > 16 or hz > 48000:
            return f"Hi-Res {bits}/{_khz(hz)}" if bits else f"Hi-Res {_khz(hz)} kHz"
        return f"CD {bits or 16}/{_khz(hz)}"
    nombre = NOMBRES.get(familia(codec), codec.upper() or "?")
    return f"{nombre} {kbps} kbps" if kbps else nombre


def nivel(codec: str, bits: int, hz: int) -> str:
    """«hires», «cd» o «comprimida»: para el color de la etiqueta."""
    if not es_sin_perdida(codec):
        return "comprimida"
    return "hires" if bits > 16 or hz > 48000 else "cd"


@dataclass
class Info:
    ruta: str
    titulo: str = ""
    artista: str = ""
    album: str = ""
    pista: int = 0
    duracion: float = 0.0
    codec: str = ""
    bits: int = 0
    hz: int = 0
    kbps: int = 0
    portada: bool = False
    extra: dict = field(default_factory=dict)

    @property
    def etiqueta(self) -> str:
        return etiqueta(self.codec, self.bits, self.hz, self.kbps)

    @property
    def nivel(self) -> str:
        return nivel(self.codec, self.bits, self.hz)


def _bits(s: dict) -> int:
    for clave in ("bits_per_raw_sample", "bits_per_sample"):
        try:
            v = int(s.get(clave) or 0)
        except ValueError:
            v = 0
        if v:
            return v
    fmt = s.get("sample_fmt", "")
    return {"s16": 16, "s16p": 16, "s32": 32, "s32p": 32, "flt": 32, "fltp": 32, "dbl": 64, "u8": 8}.get(fmt, 0)


def _etiquetas(*dicts: dict) -> dict[str, str]:
    t: dict[str, str] = {}
    for d in dicts:
        for k, v in (d.get("tags") or {}).items():
            t.setdefault(k.lower(), str(v))
    return t


def de_ffprobe(ruta: str, j: dict) -> Info | None:
    """Lo que interesa de la salida JSON de ffprobe; None si no tiene audio."""
    streams = j.get("streams") or []
    audio = next((s for s in streams if s.get("codec_type") == "audio"), None)
    if audio is None:
        return None
    fmt = j.get("format") or {}
    t = _etiquetas(fmt, audio)
    codec = audio.get("codec_name", "")
    bits = _bits(audio) if es_sin_perdida(codec) else 0
    if codec == "alac" and not bits:
        bits = 16
    try:
        hz = int(audio.get("sample_rate") or 0)
    except ValueError:
        hz = 0
    br = audio.get("bit_rate") or fmt.get("bit_rate") or 0
    try:
        kbps = round(int(br) / 1000)
    except ValueError:
        kbps = 0
    # Opus y Vorbis en .webm/.ogg no traen bit_rate del stream: el del archivo es casi igual
    try:
        duracion = float(fmt.get("duration") or audio.get("duration") or 0)
    except ValueError:
        duracion = 0.0
    pista = t.get("track", "").split("/")[0]
    titulo = t.get("title") or Path(ruta).stem
    return Info(
        ruta=ruta, titulo=titulo, artista=t.get("artist") or t.get("album_artist") or "",
        album=t.get("album", ""), pista=int(pista) if pista.isdigit() else 0, duracion=duracion,
        codec=codec, bits=bits, hz=hz, kbps=kbps,
        portada=any(s.get("codec_type") == "video" for s in streams),
        extra={k: t[k] for k in ("purl", "comment", "description") if k in t},
    )


def leer(ruta: str | Path, ffprobe: str = "ffprobe") -> Info | None:
    try:
        r = subprocess.run([ffprobe, "-v", "quiet", "-print_format", "json", "-show_format", "-show_streams",
                            str(ruta)], capture_output=True, text=True, timeout=20)
    except (OSError, subprocess.TimeoutExpired):
        return None
    if r.returncode != 0:
        return None
    try:
        return de_ffprobe(str(ruta), json.loads(r.stdout or "{}"))
    except (ValueError, TypeError):
        return None
