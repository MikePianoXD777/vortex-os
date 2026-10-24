"""Prueba de «una sola tienda por usuario»: la segunda instancia le avisa a la primera (que se trae al
frente y le pide a Hyprland que enfoque su ventana) y sale con 0 sin abrir ventana. Usa un nombre de
socket de prueba, el motor falso, «offscreen» y un hyprctl falso que anota lo que le piden.

    python apps/shop/pruebas/ui_instancia.py
"""
from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

AQUI = Path(__file__).resolve().parent
TIENDA = AQUI.parent

CODIGO = """
import os, sys
os.environ["QT_QPA_PLATFORM"] = "offscreen"
os.environ["VORTEX_SHOP_COLORES"] = "/nonexistent/qs_colors.json"
sys.path[:0] = [{tienda!r}, {aqui!r}]
import falso_motor
from vortex_shop.__main__ import main
motor = falso_motor.crear_motor({datos!r}, latencia=0.05)
sys.exit(main([sys.argv[0]], motor=motor, instancia={nombre!r}))
"""


def main() -> int:
    nombre = f"vortexines-shop-prueba-{os.getpid()}"
    codigo = CODIGO.format(tienda=str(TIENDA), aqui=str(AQUI), datos=tempfile.mkdtemp(prefix="vortex-shop-inst-"), nombre=nombre)
    # hyprctl falso: anota cada orden; la sintaxis Lua falla, como en un Hyprland con config de antes.
    binarios = Path(tempfile.mkdtemp(prefix="vortex-shop-hypr-"))
    anotado = binarios / "ordenes.txt"
    falso = binarios / "hyprctl"
    falso.write_text(f'#!/bin/sh\nprintf "%s\\n" "$*" >> "{anotado}"\ncase "$*" in *hl.dsp*) exit 7 ;; esac\n')
    falso.chmod(0o755)
    entorno = dict(os.environ, PATH=f"{binarios}:{os.environ.get('PATH', '')}", HYPRLAND_INSTANCE_SIGNATURE="prueba")
    primera = subprocess.Popen([sys.executable, "-c", codigo], stderr=subprocess.PIPE, text=True, env=entorno)
    try:
        time.sleep(4)
        vive = primera.poll() is None
        print(("ok    " if vive else "FALLA ") + "la primera instancia queda corriendo")
        t0 = time.monotonic()
        segunda = subprocess.run([sys.executable, "-c", codigo], capture_output=True, text=True, timeout=30)
        rapido = time.monotonic() - t0 < 15
        print(("ok    " if segunda.returncode == 0 else "FALLA ") + f"la segunda sale con 0 (salió con {segunda.returncode})")
        print(("ok    " if rapido and primera.poll() is None else "FALLA ") + "la segunda sale sola y la primera sigue viva")
        ordenes = anotado.read_text().splitlines() if anotado.exists() else []
        esperadas = [f'dispatch hl.dsp.focus({{ window = "pid:{primera.pid}" }})', f"dispatch focuswindow pid:{primera.pid}"]
        enfoca = ordenes == esperadas
        print(("ok    " if enfoca else "FALLA ") + f"la primera le pide a Hyprland enfocar su ventana (Lua y, si falla, la de antes): {ordenes}")
        bien = vive and segunda.returncode == 0 and rapido and primera.poll() is None and enfoca
    finally:
        primera.terminate()
        try:
            primera.wait(timeout=10)
        except subprocess.TimeoutExpired:
            primera.kill()
    print("\nTODO BIEN" if bien else "\nFALLÓ")
    return 0 if bien else 1


if __name__ == "__main__":
    sys.exit(main())
