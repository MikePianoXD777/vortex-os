"""Dónde vive cada cosa de Miku (código, modelos, configuración, temporales).

Instalada por el paquete vortex-miku, la app está en /usr/share/vortex-miku (código, página web y
modelo 3D) y /usr/lib/vortex-miku/python (onnxruntime, para la voz). Los modelos de voz y de oído NO
vienen en el paquete (pesan ~560 MB): se bajan la primera vez que se abre a Miku a
~/.local/share/vortex/miku/{voz,oido}/. Desde el repo, apps/miku/ tiene la misma forma, y las
variables VORTEX_MIKU_* mueven lo demás (pruebas).
"""

import os
import tempfile
from pathlib import Path

RAIZ = Path(os.environ.get("VORTEX_MIKU_RAIZ") or Path(__file__).resolve().parent.parent)
# Bibliotecas de Python vendorizadas (onnxruntime): fuera de site-packages.
LIBS_PY = Path(os.environ.get("VORTEX_MIKU_LIBS", "/usr/lib/vortex-miku/python"))
WEB = RAIZ / "web"
# Three.js y three-vrm: no están en git; el paquete los instala en web/vendor.
VENDOR = Path(os.environ.get("VORTEX_MIKU_VENDOR") or WEB / "vendor")
PERSONAS = RAIZ / "persona"
CATALOGO_VOCES = Path(os.environ.get("VORTEX_MIKU_CATALOGO") or RAIZ / "voces.json")
LICENCIAS = RAIZ / "licencias"          # textos de las licencias de lo que usa la app (el paquete los copia a /usr/share/licenses)
# La ISO deja aquí el modelo 3D (no va en git, por derechos del personaje).
MODELO_SISTEMA = Path(os.environ.get("VORTEX_MIKU_MODELO_SISTEMA", "/usr/share/vortex-miku/modelo/modelo.vrm"))


def _xdg(variable: str, defecto: str) -> Path:
    valor = os.environ.get(variable)
    return Path(valor) if valor else Path.home() / defecto


CONFIG_DIR = _xdg("XDG_CONFIG_HOME", ".config") / "vortex"
CONFIG = CONFIG_DIR / "miku.json"
DATOS = _xdg("XDG_DATA_HOME", ".local/share") / "vortex" / "miku"
MODELO_USUARIO = DATOS / "modelo.vrm"
VOZ = DATOS / "voz"                    # Supertonic 3 (ONNX) y los estilos de voz; se baja la primera vez
OIDO = DATOS / "oido"                  # el modelo de Whisper para reconocer lo que dices; se baja la primera vez
AJUSTES_SHELL = _xdg("XDG_CONFIG_HOME", ".config") / "serpantinum" / "settings.json"     # solo lectura (general.uiScale)
PALETA_SHELL = _xdg("XDG_STATE_HOME", ".local/state") / "serpantinum" / "qs_colors.json"  # solo lectura (colores del fondo)
# Marca del instalador («Miku con internet», paquete vortex-miku-internet): internet encendido de fábrica.
MARCA_INTERNET = Path(os.environ.get("VORTEX_MIKU_MARCA_INTERNET", "/etc/vortex/miku-internet"))
CEREBRO_CWD = DATOS / "cerebro"        # carpeta neutra y vacía desde donde corre Claude
# Voz natural (opcional, en la GPU o, lenta, en la CPU): PyTorch y Qwen3-TTS en su propio entorno, que pone
# «vortex-miku natural instalar».
NATURAL = Path(os.environ.get("VORTEX_MIKU_NATURAL") or DATOS / "natural")
NATURAL_PYTHON = NATURAL / "venv" / "bin" / "python"
NATURAL_LISTO = NATURAL / "listo"      # lo escribe la instalación al terminar bien (con las versiones)
NATURAL_MODO = NATURAL / "modo"        # «gpu», «cuda» o «cpu»: lo elige la instalación (sin el archivo, gpu: las de la 1.3)
NATURAL_CUANT = NATURAL / "cuantizacion"   # modo cuda: Q8_0 o Q4_K_M (según la VRAM)


def runtime() -> Path:
    """Carpeta del usuario en memoria (tmpfs): candado, PID, registro y audios temporales."""
    base = os.environ.get("XDG_RUNTIME_DIR")
    if not base or not os.path.isdir(base):
        base = os.path.join(tempfile.gettempdir(), f"vortex-miku-{os.getuid()}")
        os.makedirs(base, mode=0o700, exist_ok=True)
    return Path(base)


def hilos() -> int:
    """Hilos para la voz y el oído: los núcleos físicos (máximo 8). Con los lógicos (SMT) los dos hilos de un
    núcleo se pelean por la misma unidad de cálculo, no va más rápido y el resto del sistema se traba."""
    if os.environ.get("VORTEX_MIKU_HILOS", "").isdigit():
        return max(1, int(os.environ["VORTEX_MIKU_HILOS"]))
    nucleos = set()
    for cpu in Path("/sys/devices/system/cpu").glob("cpu[0-9]*/topology"):
        try:
            nucleos.add(((cpu / "physical_package_id").read_text().strip(), (cpu / "core_id").read_text().strip()))
        except OSError:
            pass
    return max(1, min(8, len(nucleos) or os.cpu_count() or 4))


def temporales() -> Path:
    """Audios de la voz mientras se reproducen; se borran en cuanto suenan."""
    d = runtime() / "vortex-miku"
    d.mkdir(mode=0o700, exist_ok=True)
    return d


def ruta_modelo(cfg: dict) -> Path | None:
    """El VRM que se usa: el de miku.json, el del usuario o el que dejó la ISO. None = no hay."""
    candidatos = []
    if cfg.get("modelo"):
        candidatos.append(Path(os.path.expanduser(str(cfg["modelo"]))))
    candidatos += [MODELO_USUARIO, MODELO_SISTEMA]
    for c in candidatos:
        try:
            if c.is_file():
                return c
        except OSError:
            pass
    return None


def leer_credito(modelo: Path | None) -> str:
    """CREDITO.txt junto al modelo (la licencia del modelo pide crédito). Vacío si no hay."""
    if modelo is None:
        return ""
    try:
        return (modelo.parent / "CREDITO.txt").read_text(encoding="utf-8").strip()
    except OSError:
        return ""
