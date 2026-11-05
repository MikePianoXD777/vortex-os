"""Vortexine Connect: se conecta a redes abiertas cuando viajas y cifra lo que mandas con Cloudflare WARP."""
from __future__ import annotations

import os
import sys
from pathlib import Path

APP_ID = "vortex-connect"
NOMBRE = "Vortexine Connect"
VERSION = "1.0.0"
ES_WINDOWS = sys.platform == "win32"


def carpeta_config() -> Path:
    if ES_WINDOWS:
        return Path(os.environ.get("APPDATA") or Path.home()) / "Vortexine" / "Connect"
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config") / "vortex"
