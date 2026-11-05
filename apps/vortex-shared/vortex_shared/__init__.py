"""Vortexine Shared: pasa archivos a otra compu con Vortexine's OS, cerca o lejos.

Se escribe la IP (o el nombre en Tailscale) y el usuario de la otra compu; a ella le sale un aviso para
aceptar. El archivo va cifrado de punta a punta con sendme (iroh), directo si se puede y por un relay si no.
Sin Tailscale ni misma red, se crea un código y lo pegas en la otra.

motor.py no depende de Qt, servicio.py escucha los avisos (lo arranca un socket de systemd, así que en
reposo no ocupa nada) y la ventana vive en puente.py y qml/.
"""

VERSION = "1.0.0"
NOMBRE = "Vortexine Shared"
APP_ID = "vortex-shared"
