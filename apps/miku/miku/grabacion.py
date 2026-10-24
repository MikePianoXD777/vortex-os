"""Grabar la pantalla (una partida) con gpu-screen-recorder, solo cuando la persona se lo pide a Miku.

La grabación es un proceso aparte (sesión propia): sigue aunque cierres a Miku, y se para con «Miku, ya
para» o con `vortex-miku grabar parar`. Graba el monitor enfocado a 60 fps con el sonido del sistema (no
el micrófono) en ~/Vídeos/Miku. gpu-screen-recorder guarda el archivo al recibir SIGINT.

Solo biblioteca estándar: lo usan acciones_mcp.py (con python -I) y el comando vortex-miku.
"""

import json
import os
import shutil
import signal
import subprocess
import time
from datetime import datetime
from pathlib import Path

PROGRAMA = "gpu-screen-recorder"
FPS = "60"
ESPERA_ARRANQUE = 1.5      # segundos para ver que no se cayó al empezar (monitor o permisos)
ESPERA_CIERRE = 20.0       # segundos para que termine de escribir el archivo


class ErrorGrabacion(Exception):
    pass


def _runtime() -> Path:
    d = Path(os.environ.get("XDG_RUNTIME_DIR") or f"/tmp/vortex-miku-{os.getuid()}") / "vortex-miku"
    d.mkdir(parents=True, exist_ok=True, mode=0o700)
    return d


ESTADO = "grabacion.json"
REGISTRO = "grabacion.log"


def carpeta_videos() -> Path:
    """~/Vídeos/Miku (la carpeta de vídeos de tu idioma)."""
    if os.environ.get("VORTEX_MIKU_VIDEOS"):            # pruebas
        return Path(os.environ["VORTEX_MIKU_VIDEOS"])
    base = ""
    if shutil.which("xdg-user-dir"):
        try:
            base = subprocess.run(["xdg-user-dir", "VIDEOS"], capture_output=True, text=True, timeout=5,
                                  stdin=subprocess.DEVNULL).stdout.strip()
        except (OSError, subprocess.TimeoutExpired):
            base = ""
    if not base or Path(base) == Path.home():
        base = str(Path.home() / "Vídeos")
    return Path(base) / "Miku"


def _es_grabadora(pid: int) -> bool:
    try:
        cmd = Path(f"/proc/{pid}/cmdline").read_bytes().split(b"\0")
    except OSError:
        return False
    # El programa, o un script que lo envuelve (sh gpu-screen-recorder …, como en las pruebas).
    return any(os.path.basename(c.decode(errors="replace")) == PROGRAMA for c in cmd[:2])


def grabando() -> dict | None:
    """{"pid", "archivo", "inicio"} de la grabación en curso, o None (y borra un estado viejo)."""
    ruta = _runtime() / ESTADO
    try:
        estado = json.loads(ruta.read_text(encoding="utf-8"))
        pid = int(estado["pid"])
    except (OSError, ValueError, KeyError, TypeError):
        return None
    if _es_grabadora(pid):
        return estado
    ruta.unlink(missing_ok=True)
    return None


def _monitor() -> str:
    """El monitor enfocado de Hyprland (p. ej. DP-2), o «screen» (el primero que encuentre)."""
    if shutil.which("hyprctl"):
        try:
            r = subprocess.run(["hyprctl", "monitors", "-j"], capture_output=True, text=True, timeout=5,
                               stdin=subprocess.DEVNULL)
            for m in json.loads(r.stdout or "[]"):
                if m.get("focused") and m.get("name"):
                    return str(m["name"])
        except (OSError, ValueError, subprocess.TimeoutExpired):
            pass
    return "screen"


def ultima_grabacion() -> Path | None:
    videos = [p for p in carpeta_videos().glob("*.mp4") if p.is_file()]
    return max(videos, key=lambda p: p.stat().st_mtime) if videos else None


def empezar() -> Path:
    if grabando():
        raise ErrorGrabacion("Ya estoy grabando. Para pararla: «Miku, ya para» o vortex-miku grabar parar.")
    if not shutil.which(PROGRAMA):
        raise ErrorGrabacion("Falta gpu-screen-recorder (sudo pacman -S gpu-screen-recorder).")
    carpeta = carpeta_videos()
    carpeta.mkdir(parents=True, exist_ok=True)
    archivo = carpeta / f"Partida {datetime.now():%Y-%m-%d %H.%M.%S}.mp4"
    monitor = _monitor()
    registro = _runtime() / REGISTRO
    with open(registro, "wb") as log:
        proc = subprocess.Popen([PROGRAMA, "-w", monitor, "-f", FPS, "-a", "default_output", "-c", "mp4",
                                 "-o", str(archivo)],
                                stdin=subprocess.DEVNULL, stdout=log, stderr=log, start_new_session=True,
                                close_fds=True)
    time.sleep(ESPERA_ARRANQUE)
    if proc.poll() is not None:
        try:
            ultimo = registro.read_text(encoding="utf-8", errors="replace").strip().splitlines()[-1]
        except (OSError, IndexError):
            ultimo = ""
        raise ErrorGrabacion(f"No pude empezar a grabar ({monitor})" + (f": {ultimo}" if ultimo else "."))
    (_runtime() / ESTADO).write_text(json.dumps({"pid": proc.pid, "archivo": str(archivo),
                                                 "inicio": time.time()}, ensure_ascii=False), encoding="utf-8")
    return archivo


def parar() -> tuple[Path, float]:
    """(archivo, segundos grabados)."""
    estado = grabando()
    if not estado:
        raise ErrorGrabacion("No estoy grabando nada.")
    pid = int(estado["pid"])
    try:
        os.kill(pid, signal.SIGINT)
    except ProcessLookupError:
        pass
    limite = time.monotonic() + ESPERA_CIERRE
    while _es_grabadora(pid) and time.monotonic() < limite:
        time.sleep(0.2)
    (_runtime() / ESTADO).unlink(missing_ok=True)
    archivo = Path(estado["archivo"])
    duracion = max(0.0, time.time() - float(estado.get("inicio") or time.time()))
    if not archivo.is_file() or archivo.stat().st_size == 0:
        raise ErrorGrabacion("Paré la grabación, pero no quedó ningún video.")
    return archivo, duracion


def duracion_texto(segundos: float) -> str:
    m, s = divmod(int(round(segundos)), 60)
    h, m = divmod(m, 60)
    return f"{h} h {m} min" if h else (f"{m} min {s} s" if m else f"{s} s")
