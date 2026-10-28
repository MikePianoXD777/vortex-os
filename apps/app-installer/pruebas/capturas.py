"""Capturas de la ventana sin abrirla en el escritorio (plataforma «offscreen») y en un HOME temporal.

    python3 pruebas/capturas.py CARPETA_DE_CAPTURAS ARCHIVO[::ACCION] … [--accion ID] [--en] [--ancho 760]

Por cada archivo: lo abre, saca captura; si tiene acción («archivo.zip::descomprimir», o --accion para
todos), la ejecuta (la primera acción con ese id),
espera a que termine y saca otra captura; al final, una de la pestaña «Instalado». Falla si QML avisó
de algún error. HOME, XDG_DATA_HOME y los colores del fondo apuntan a lo temporal: nada real se toca.
"""
from __future__ import annotations

import argparse
import os
import sys
import tempfile
import time
from pathlib import Path

os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ.setdefault("QT_QUICK_BACKEND", "software")
os.environ["VORTEX_APP_INSTALLER_COLORES"] = "/nonexistent/qs_colors.json"   # la paleta fija, no la del fondo
if "VORTEX_PRUEBA_HOME" not in os.environ:
    casa = tempfile.mkdtemp(prefix="vortex-app-installer-home-")
    os.environ["HOME"] = casa
    os.environ.pop("XDG_DATA_HOME", None)
    os.environ.pop("XDG_CONFIG_HOME", None)
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from PySide6.QtCore import QEventLoop, QTimer, QtMsgType, qInstallMessageHandler  # noqa: E402

from vortex_app_installer.__main__ import cargar_interfaz, crear_aplicacion  # noqa: E402

AVISOS: list[str] = []


def _mensajes(tipo, contexto, texto):
    if tipo in (QtMsgType.QtWarningMsg, QtMsgType.QtCriticalMsg, QtMsgType.QtFatalMsg) and "propagateSizeHints" not in texto:
        AVISOS.append(texto)
    print(f"[qt] {texto}", file=sys.stderr)


def esperar(ms: int) -> None:
    bucle = QEventLoop()
    QTimer.singleShot(ms, bucle.quit)
    bucle.exec()


def esperar_estado(instalador, estados: tuple[str, ...], limite: float = 120) -> None:
    fin = time.time() + limite
    while instalador.property("estado") not in estados and time.time() < fin:
        esperar(100)


def captura(ventana, ruta: Path) -> None:
    esperar(450)   # animaciones de entrada
    ventana.grabWindow().save(str(ruta))
    print("captura:", ruta)


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("carpeta")
    p.add_argument("archivos", nargs="*")
    p.add_argument("--accion", default="")
    p.add_argument("--contrasena", default="")
    p.add_argument("--en", action="store_true")
    p.add_argument("--ancho", type=int, default=760)
    p.add_argument("--alto", type=int, default=720)
    args = p.parse_args()
    if args.en:
        os.environ["VORTEX_APP_INSTALLER_IDIOMA"] = "en"
    carpeta = Path(args.carpeta)
    carpeta.mkdir(parents=True, exist_ok=True)
    qInstallMessageHandler(_mensajes)
    app = crear_aplicacion(sys.argv[:1])
    interfaz, instalador = cargar_interfaz(app)
    ventana = interfaz.rootObjects()[0]
    ventana.setWidth(args.ancho)
    ventana.setHeight(args.alto)
    sufijo = "-en" if args.en else ""
    captura(ventana, carpeta / f"00-vacio{sufijo}.png")
    for n, pedido in enumerate(args.archivos, 1):
        archivo, _, id_accion = pedido.partition("::")
        id_accion = id_accion or args.accion
        base = f"{n:02d}-{Path(archivo).name}{sufijo}"
        instalador.abrir(archivo)
        esperar_estado(instalador, ("listo", "error"))
        captura(ventana, carpeta / f"{base}-a.png")
        if id_accion:
            accion = next((a for a in instalador.property("archivo").get("acciones", []) if a["id"] == id_accion), None)
            if accion is None:
                print("sin la acción", id_accion, "para", archivo)
                continue
            instalador.ejecutar(accion["id"], accion.get("datos") or {})
            fin = time.time() + 5   # el progreso, en cuanto pase del 15 %
            while instalador.property("estado") == "trabajando" and instalador.property("progreso") < 0.15 \
                    and time.time() < fin:
                esperar(30)
            if instalador.property("estado") == "trabajando":
                esperar(250)   # que termine la animación de la barra
                ventana.grabWindow().save(str(carpeta / f"{base}-b-trabajando.png"))
            esperar_estado(instalador, ("hecho", "error", "listo"))
            if instalador.property("contrasena").get("accion") and args.contrasena:
                captura(ventana, carpeta / f"{base}-c-contrasena.png")
                instalador.enviarContrasena(args.contrasena)
                esperar(200)
                esperar_estado(instalador, ("hecho", "error", "listo"))
            captura(ventana, carpeta / f"{base}-d-resultado.png")
            instalador.cerrarArchivo()
    ventana.setProperty("pestana", "instalado")
    captura(ventana, carpeta / f"99-instalado{sufijo}.png")
    print("HOME de la prueba:", os.environ["HOME"])
    if AVISOS:
        print("AVISOS DE QML:", *AVISOS, sep="\n  ")
        return 1
    return 0


if __name__ == "__main__":
    code = main()
    sys.stdout.flush()
    os._exit(code)
