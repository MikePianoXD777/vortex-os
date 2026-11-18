"""Prueba de la página «Juegos de Android» de la Shop (4.0), con un vortex-android falso y sin ventanas
(«offscreen»): sin preparar, preparando y lista; registra en Google y abre un juego.

    python apps/shop/pruebas/ui_android.py [carpeta_de_capturas]
"""
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("VORTEX_SHOP_COLORES", "/nonexistent/qs_colors.json")
os.environ.setdefault("QT_QUICK_BACKEND", "software")
AQUI = Path(__file__).resolve().parent
os.environ["VORTEX_SHOP_ANDROID"] = str(AQUI / "android_falso")
os.environ["ANDROID_FALSO_DIR"] = tempfile.mkdtemp(prefix="android-falso-")
os.environ.setdefault("ANDROID_FALSO_NVIDIA", "true")
sys.path.insert(0, str(AQUI.parent))
sys.path.insert(0, str(AQUI))

from ui_capturas import AVISOS, _mensajes, esperar, llamar  # noqa: E402
from PySide6.QtCore import qInstallMessageHandler  # noqa: E402

import falso_motor  # noqa: E402
from vortex_shop.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402


def main() -> int:
    carpeta = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(tempfile.mkdtemp(prefix="capturas-android-"))
    carpeta.mkdir(parents=True, exist_ok=True)
    qInstallMessageHandler(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    motor = falso_motor.crear_motor(tempfile.mkdtemp(prefix="vortex-shop-ui-"), duracion=1.0)
    interfaz, puente = cargar_interfaz(app, motor)
    if not interfaz.rootObjects():
        print("FALLO: el QML no cargó", file=sys.stderr)
        return 1
    v = interfaz.rootObjects()[0]
    puente.iniciar()
    fallos: list[str] = []

    def foto(nombre: str, espera: int = 900) -> None:
        esperar(espera)
        ruta = carpeta / f"{nombre}.png"
        assert v.grabWindow().save(str(ruta)), ruta
        print("captura", ruta)

    def revisar(cond: bool, texto: str) -> None:
        if not cond:
            fallos.append(texto)

    llamar(v, "navegar", "android", "")
    foto("android-1-sin-preparar", 1500)
    revisar(puente.androidCargado and not puente.androidEstado["preparado"], "debía verse sin preparar")
    revisar(puente.androidEstado["nvidia"], "debía avisar de NVIDIA")

    puente.prepararAndroid()
    foto("android-2-preparando", 450)
    revisar(puente.androidFase == "preparando", "debía estar preparando")
    esperar(2500)
    foto("android-3-lista", 800)
    revisar(puente.androidEstado.get("preparado") is True, "debía quedar preparado")
    revisar(len(puente.androidApps) == 1, "debía listar Subway Surfers")

    puente.androidPedirId()
    esperar(800)
    revisar(puente.androidId == "3912345678901234567", "debía mostrar el número de Google")
    foto("android-4-numero", 300)
    puente.androidYaRegistre()
    esperar(1200)
    revisar(puente.androidEstado.get("registrado") is True, "debía quedar registrado")
    puente.androidTienda("com.supercell.clashroyale")
    esperar(800)
    foto("android-5-registrado", 300)

    llamadas = (Path(os.environ["ANDROID_FALSO_DIR"]) / "llamadas").read_text()
    revisar("tienda com.supercell.clashroyale" in llamadas, "debía abrir la Play Store en Clash Royale")
    avisos = [a for a in AVISOS if "android" in a.lower() or "PaginaAndroid" in a or "qml" in a.lower()]
    revisar(not avisos, f"avisos de QML: {avisos}")
    for f in fallos:
        print("FALLO:", f, file=sys.stderr)
    print("capturas en", carpeta)
    return 1 if fallos else 0


if __name__ == "__main__":
    sys.exit(main())
