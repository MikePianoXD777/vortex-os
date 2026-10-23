"""Prueba de «una sola tienda por usuario»: la segunda instancia le avisa a la primera (que se trae al
frente) y sale con 0 sin abrir ventana. Usa un nombre de socket de prueba, el motor falso y «offscreen».

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
    primera = subprocess.Popen([sys.executable, "-c", codigo], stderr=subprocess.PIPE, text=True)
    try:
        time.sleep(4)
        vive = primera.poll() is None
        print(("ok    " if vive else "FALLA ") + "la primera instancia queda corriendo")
        t0 = time.monotonic()
        segunda = subprocess.run([sys.executable, "-c", codigo], capture_output=True, text=True, timeout=30)
        rapido = time.monotonic() - t0 < 15
        print(("ok    " if segunda.returncode == 0 else "FALLA ") + f"la segunda sale con 0 (salió con {segunda.returncode})")
        print(("ok    " if rapido and primera.poll() is None else "FALLA ") + "la segunda sale sola y la primera sigue viva")
        bien = vive and segunda.returncode == 0 and rapido and primera.poll() is None
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
