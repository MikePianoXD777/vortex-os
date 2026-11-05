"""Vortexine Connect (python -m vortex_connect).

    vortex-connect                   la ventana
    vortex-connect estado            dice en qué red estás y si va cifrado
    vortex-connect redes             las redes abiertas cerca
    vortex-connect vigilar           el modo viaje (lo arranca su servicio)
    vortex-connect --prueba-humo     carga la ventana y se cierra (para el .exe recién armado)

Qt solo se carga para la ventana: el modo viaje no necesita 100 MB.
"""
from __future__ import annotations

import sys


def _cli_estado() -> int:
    from .motor import Connect
    e = Connect().estado()
    print(e.estado, e.red.tipo or "-", e.red.ssid or "-")
    return 0


def _cli_redes() -> int:
    from .motor import Connect
    for red in Connect().redes.lista():
        print(f"{red.senal:3d} %  {'*' if red.conectada else ' '} {red.ssid}")
    return 0


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else argv
    orden = argv[0] if argv else "ventana"
    if orden == "--version":
        from . import NOMBRE, VERSION
        print(NOMBRE, VERSION)
        return 0
    if orden == "--prueba-humo":
        from .ventana import prueba_humo
        return prueba_humo()
    if orden == "estado":
        return _cli_estado()
    if orden == "redes":
        return _cli_redes()
    if orden == "vigilar":
        from .viaje import main as vigilar
        return vigilar()
    from .ventana import main as ventana
    return ventana()


if __name__ == "__main__":
    sys.exit(main())
