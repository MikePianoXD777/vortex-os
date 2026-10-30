"""Vortexine Task Manager: el administrador de tareas de Vortexine's OS.

Como el de Windows (Ctrl+Shift+Esc), con el diseño de Vortexine's: las apps abiertas con todo lo que
usan, lo de segundo plano y lo del sistema; gráficas de CPU, memoria, discos, red y tarjeta de video;
y los servicios, con su interruptor de «Al iniciar».

El motor no depende de Qt (sistema.py, servicios.py); la interfaz (QML) vive en __main__.py,
puente.py, modelo.py y qml/.
"""

VERSION = "1.0.0"
NOMBRE = "Vortexine Task Manager"
APP_ID = "vortex-task-manager"
