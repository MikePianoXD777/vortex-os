"""Vortexine's Runner: abre programas y juegos de Windows (.exe) con Proton, como Heroic Games Launcher.

Usa umu-run (el mismo lanzador de Heroic y Lutris) con el Proton de CachyOS de serie; también GE-Proton
o cualquier Proton que tengas. Cada programa va en su propio prefijo (su «disco C:»), así no se estorban.

El motor no depende de Qt: rutas.py, pe.py, protones.py, biblioteca.py y correr.py. La interfaz (QML)
vive en __main__.py, puente.py, colores.py y qml/.
"""

VERSION = "1.0.0"
__version__ = VERSION
NOMBRE = "Vortexine's Runner"
APP_ID = "vortex-runner"
