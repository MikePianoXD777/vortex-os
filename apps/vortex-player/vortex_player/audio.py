"""La salida de audio: por dónde suena, con qué códec Bluetooth y a qué frecuencia.

- LDAC: PipeWire elige el códec al conectar los audífonos. Si los audífonos lo soportan, «forzar_ldac»
  cambia el perfil de la tarjeta a LDAC, y la regla de WirePlumber de `config_ldac` lo deja siempre
  a 990 kbps («hq»; el «auto» de fábrica baja hasta 330 kbps con poca señal).
- Frecuencia nativa: de fábrica PipeWire pasa todo a 48 kHz. Con `config_hires` deja cambiar a la
  frecuencia de cada canción (44,1, 88,2, 96, 176,4 o 192 kHz), sin remuestrear.
Las dos cosas son archivos de la persona (~/.config), sin root, y se notan al volver a arrancar
PipeWire (`reiniciar_audio`, que corta el sonido un segundo) o al volver a entrar.
"""
from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

CODECS = {"ldac": "LDAC", "aac": "AAC", "sbc": "SBC", "sbc_xq": "SBC XQ", "aptx": "aptX", "aptx_hd": "aptX HD",
          "aptx_ll": "aptX LL", "lc3": "LC3", "opus_05": "Opus", "faststream": "FastStream", "msbc": "mSBC", "cvsd": "CVSD"}
FRECUENCIAS = [44100, 48000, 88200, 96000, 176400, 192000]


def _config() -> Path:
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")


def archivo_hires() -> Path:
    return _config() / "pipewire" / "pipewire.conf.d" / "90-vortex-player-hires.conf"


def archivo_ldac() -> Path:
    return _config() / "wireplumber" / "wireplumber.conf.d" / "90-vortex-player-ldac.conf"


TEXTO_HIRES = """# Vortex Player: cada canción suena a su frecuencia original (sin pasar todo a 48 kHz).
# Se quita borrando este archivo.
context.properties = {
    default.clock.allowed-rates = [ %s ]
}
""" % " ".join(str(f) for f in FRECUENCIAS)

TEXTO_LDAC = """# Vortex Player: con audífonos LDAC, siempre a 990 kbps (la calidad más alta) en vez de «auto».
# Se quita desde Vortex Player (Ajustes) o borrando este archivo.
monitor.bluez.rules = [
  {
    matches = [ { device.name = "~bluez_card.*" } ]
    actions = { update-props = { bluez5.a2dp.ldac.quality = "hq" } }
  }
]
"""


def _escribir(ruta: Path, texto: str) -> bool:
    """True si cambió algo."""
    try:
        if ruta.read_text() == texto:
            return False
    except OSError:
        pass
    ruta.parent.mkdir(parents=True, exist_ok=True)
    ruta.write_text(texto)
    return True


def config_hires() -> bool:
    return _escribir(archivo_hires(), TEXTO_HIRES)


def config_ldac(activar: bool) -> bool:
    if activar:
        return _escribir(archivo_ldac(), TEXTO_LDAC)
    if archivo_ldac().exists():
        archivo_ldac().unlink()
        return True
    return False


def _pactl(*args: str, pactl: str = "pactl") -> str:
    try:
        r = subprocess.run([pactl, *args], capture_output=True, text=True, timeout=5)
    except (OSError, subprocess.TimeoutExpired):
        return ""
    return r.stdout if r.returncode == 0 else ""


def _hz_de(spec: str) -> int:
    for parte in spec.split():
        if parte.endswith("Hz") and parte[:-2].isdigit():
            return int(parte[:-2])
    return 0


def salida(pactl: str = "pactl") -> dict:
    """La salida por defecto: {nombre, bluetooth, codec, codecTexto, hz, tarjeta, ldac (si los audífonos
    lo soportan)}. Vacío si no hay PipeWire."""
    defecto = _pactl("get-default-sink", pactl=pactl).strip()
    try:
        sinks = json.loads(_pactl("-f", "json", "list", "sinks", pactl=pactl) or "[]")
    except ValueError:
        sinks = []
    sink = next((s for s in sinks if s.get("name") == defecto), sinks[0] if sinks else None)
    if not sink:
        return {}
    props = sink.get("properties") or {}
    bt = props.get("device.api") == "bluez5" or str(sink.get("name", "")).startswith("bluez_output")
    codec = props.get("api.bluez5.codec", "") if bt else ""
    tarjeta = props.get("device.name", "")
    d = {"nombre": props.get("device.description") or sink.get("description") or sink.get("name", ""),
         "bluetooth": bt, "codec": codec, "codecTexto": CODECS.get(codec, codec.upper()),
         "hz": _hz_de(sink.get("sample_specification", "")), "tarjeta": tarjeta, "ldac": False}
    if bt and tarjeta:
        d["ldac"] = "a2dp-sink-ldac" in perfiles(tarjeta, pactl)
    return d


def perfiles(tarjeta: str, pactl: str = "pactl") -> list[str]:
    try:
        cards = json.loads(_pactl("-f", "json", "list", "cards", pactl=pactl) or "[]")
    except ValueError:
        return []
    for c in cards:
        if c.get("name") == tarjeta:
            p = c.get("profiles") or {}
            return [n for n, v in p.items() if not isinstance(v, dict) or v.get("available", True)]
    return []


def forzar_ldac(tarjeta: str, pactl: str = "pactl") -> bool:
    if "a2dp-sink-ldac" not in perfiles(tarjeta, pactl):
        return False
    try:
        return subprocess.run([pactl, "set-card-profile", tarjeta, "a2dp-sink-ldac"],
                              capture_output=True, timeout=5).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def reiniciar_audio() -> bool:
    """Vuelve a arrancar PipeWire y WirePlumber para que lean los archivos de arriba."""
    try:
        return subprocess.run(["systemctl", "--user", "restart", "pipewire.service", "pipewire-pulse.service",
                               "wireplumber.service"], capture_output=True, timeout=20).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False
