#!/usr/bin/env python3
"""Prueba del respaldo de la voz natural: si su proceso no arranca, Miku sigue con la F1 sin perder la frase.

Sin GPU ni PyTorch: VORTEX_MIKU_NATURAL apunta a una carpeta con un «python» falso que (a) avisa un error
al cargar o (b) se muere sin decir nada. Hace falta el modelo de Supertonic (la F1) en el HOME de prueba
(como en las demás pruebas, con env.sh) y PySide6. Sin ventanas (QT_QPA_PLATFORM=offscreen).

  python3 -I apps/miku/pruebas/natural.py
"""

import json
import os
import sys
import tempfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(RAIZ))
FALLOS = 0


def probar(nombre: str, ok: bool) -> None:
    global FALLOS
    print(f"  {'ok ' if ok else 'MAL'} {nombre}")
    FALLOS += 0 if ok else 1


def armar(caso: str) -> Path:
    d = Path(tempfile.mkdtemp(prefix=f"vortex-miku-natural-{caso}."))
    (d / "venv/bin").mkdir(parents=True)
    py = d / "venv/bin/python"
    if caso == "error":
        py.write_text('#!/bin/sh\necho \'{"error": "RuntimeError: no hay una GPU NVIDIA disponible"}\'\nexit 1\n')
    else:
        py.write_text("#!/bin/sh\nexit 3\n")
    py.chmod(0o755)
    (d / "listo").write_text("prueba\n")
    return d


def main() -> int:
    os.environ.setdefault("QT_QPA_PLATFORM", "offscreen")
    casos = {c: armar(c) for c in ("error", "muere")}
    from PySide6.QtCore import QCoreApplication, QTimer
    app = QCoreApplication(sys.argv)
    for caso, carpeta in casos.items():
        os.environ["VORTEX_MIKU_NATURAL"] = str(carpeta)
        import importlib

        from miku import rutas, voces
        importlib.reload(rutas)
        importlib.reload(voces)
        from miku import asistente
        importlib.reload(asistente)
        print(f"== La voz natural {'avisa un error' if caso == 'error' else 'se muere'} al cargar")
        probar("la voz natural cuenta como instalada", voces.disponible(voces.buscar("miku-natural")))
        cfg = {"avatar": False, "voz": "miku-natural", "modelo": "", "seguir_escuchando": False, "silencio": 1.5}
        salida = carpeta / "sono"                   # VORTEX_MIKU_SALIDA: lo que «suena» se copia aquí
        os.environ["VORTEX_MIKU_SALIDA"] = str(salida)
        a = asistente.Asistente(cfg, avatar=False)
        avisos = []
        a.aviso.connect(avisos.append)
        a._arrancar_voz()
        a._decir("Hola, ¿cómo estás?")
        QTimer.singleShot(15000, app.quit)
        reloj = QTimer()
        reloj.timeout.connect(lambda: app.quit() if salida.is_dir() and any(salida.glob("*.wav")) else None)
        reloj.start(100)
        app.exec()
        reloj.stop()
        probar("cambia a la F1", getattr(a, "_voz_id", "") == voces.VOZ_DEFECTO and a._voz_lista)
        probar("avisa una vez que usa la voz de siempre", sum("voz de siempre" in t for t in avisos) == 1)
        probar("la frase pendiente igual sale con la F1", salida.is_dir() and len(list(salida.glob("*.wav"))) == 1)
        a.cerrar_todo()
    print("\nTodo bien" if not FALLOS else f"\nFALLARON {FALLOS} pruebas")
    return 1 if FALLOS else 0


if __name__ == "__main__":
    sys.exit(main())
