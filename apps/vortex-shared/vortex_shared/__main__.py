"""Vortexine Shared (python -m vortex_shared).

    vortex-shared                         la ventana
    vortex-shared ventana [ARCHIVOS]      la ventana, con esos archivos ya puestos (clic derecho de Thunar)
    vortex-shared enviar DESTINO USUARIO ARCHIVOS...
    vortex-shared codigo ARCHIVOS...      crea un código; sigue mandando hasta que lo cortes
    vortex-shared recibir CODIGO
    vortex-shared abrir-puerto            abre el puerto en ufw, si hace falta (pide la contraseña)
    vortex-shared accion                  pone «Enviar con Vortexine Shared» en el clic derecho de Thunar
    vortex-shared escuchar                lo arranca vortex-shared.socket

Qt solo se carga para la ventana: escuchar un aviso no necesita 100 MB.
"""
from __future__ import annotations

import sys


def _cli_enviar(argv: list[str]) -> int:
    from . import motor
    if len(argv) < 3:
        print("Uso: vortex-shared enviar DESTINO USUARIO ARCHIVOS...", file=sys.stderr)
        return 2
    envio = None
    try:
        envio = motor.Envio(argv[2:])
        print("Preparando…", flush=True)
        envio.preparar()
        print("Esperando a que acepten…", flush=True)
        motor.mandar(argv[0], argv[1], envio)
        print("Listo: ya llegó.")
        return 0
    except motor.ErrorShared as e:
        print(e, file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 130
    finally:
        if envio:
            envio.terminar()


def _cli_codigo(argv: list[str]) -> int:
    import time

    from . import motor
    envio = None
    try:
        envio = motor.Envio(argv)
        print("Preparando…", flush=True)
        print(f"\n{envio.preparar()}\n\nEn la otra compu: vortex-shared recibir CODIGO. Ctrl+C corta el envío.", flush=True)
        while True:
            time.sleep(3600)
    except motor.ErrorShared as e:
        print(e, file=sys.stderr)
        return 1
    except KeyboardInterrupt:
        return 0
    finally:
        if envio:
            envio.terminar()


def _cli_recibir(argv: list[str]) -> int:
    from . import motor
    if len(argv) != 1:
        print("Uso: vortex-shared recibir CODIGO", file=sys.stderr)
        return 2
    try:
        for ruta in motor.recibir(argv[0]):
            print(ruta)
        return 0
    except motor.ErrorShared as e:
        print(e, file=sys.stderr)
        return 1


def _ventana(argv: list[str]) -> int:
    from .ventana import main
    return main(argv)


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    orden = argv[0] if argv else "ventana"
    resto = argv[1:]
    if orden == "--version":
        from . import NOMBRE, VERSION
        print(NOMBRE, VERSION)
        return 0
    if orden == "escuchar":
        from .servicio import main as escuchar
        return escuchar()
    if orden == "enviar":
        return _cli_enviar(resto)
    if orden == "codigo":
        return _cli_codigo(resto)
    if orden == "recibir":
        return _cli_recibir(resto)
    if orden == "abrir-puerto":
        from . import motor
        return 0 if motor.abrir_puerto() == "listo" else 1
    if orden == "accion":
        from .thunar import poner_accion
        poner_accion()
        return 0
    return _ventana(resto if orden == "ventana" else argv)


if __name__ == "__main__":
    sys.exit(main())
