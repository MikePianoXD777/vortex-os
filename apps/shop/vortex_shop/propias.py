"""Las apps propias de Vortexine's que no vienen de fábrica (por ahora, Vortexine's Camera).

No son de Flathub: las instala, quita y abre el comando `vortex` («vortex camara instalar»…), para tu
usuario y sin contraseña. La tienda solo pregunta si están (su carpeta en ~/.local/share) y llama al
comando en un hilo.

Para pruebas: VORTEX_SHOP_VORTEX=/ruta/a/un/vortex falso cambia el comando que se llama.
"""
from __future__ import annotations

import os
import subprocess
from pathlib import Path

QML = Path(__file__).resolve().parent / "qml"

APPS: list[dict] = [
    {
        "id": "vortex-camera",
        "nombre": "Vortexine's Camera",
        "resumen": "Tus cámaras Imou en mosaico, como Imou Life en la PC: en vivo, grabaciones de la microSD, "
                   "fotos, video y las que giran. Sin nube y sin límites.",
        "subcomando": "camara",
        "carpeta": "vortex-camera",
        "paquete": "vortex_camera",
        "icono": "propias/vortex-camera.svg",
    },
]


class ErrorPropia(Exception):
    """No se pudo instalar o quitar; el texto es para la persona."""


def _datos() -> Path:
    return Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local" / "share")


def _vortex() -> str:
    return os.environ.get("VORTEX_SHOP_VORTEX") or "vortex"


def app(app_id: str) -> dict:
    a = next((a for a in APPS if a["id"] == app_id), None)
    if a is None:
        raise ErrorPropia("Esa app no es de Vortexine's.")
    return a


def instalada(app_id: str) -> bool:
    a = app(app_id)
    return (_datos() / a["carpeta"] / a["paquete"] / "__init__.py").is_file()


def lista() -> list[dict]:
    """Para QML: cada app con su ícono (URL de archivo) y si ya está."""
    return [{"id": a["id"], "nombre": a["nombre"], "resumen": a["resumen"], "desarrollador": "Vortexine's",
             "icono": (QML / a["icono"]).as_uri(), "instalada": instalada(a["id"])} for a in APPS]


def _correr(app_id: str, accion: str) -> None:
    a = app(app_id)
    try:
        r = subprocess.run([_vortex(), a["subcomando"], accion], capture_output=True, text=True, timeout=300)
    except FileNotFoundError:
        raise ErrorPropia("No encuentro el comando vortex.") from None
    except subprocess.TimeoutExpired:
        raise ErrorPropia(f"{a['nombre']} tardó demasiado.") from None
    if r.returncode != 0:
        detalle = (r.stderr.strip().splitlines() or [""])[-1].removeprefix(f"vortex {a['subcomando']}: ")
        verbo = "instalar" if accion == "instalar" else "quitar"
        raise ErrorPropia(f"No se pudo {verbo} {a['nombre']}" + (f": {detalle}" if detalle else "."))


def instalar(app_id: str) -> None:
    _correr(app_id, "instalar")


def quitar(app_id: str) -> None:
    _correr(app_id, "quitar")


def abrir(app_id: str) -> None:
    a = app(app_id)
    try:
        subprocess.Popen([_vortex(), a["subcomando"], "abrir"], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, start_new_session=True)
    except OSError:
        raise ErrorPropia(f"No se pudo abrir {a['nombre']}.") from None
