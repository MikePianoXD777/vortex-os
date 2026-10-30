"""Vortexine's Camera: tus cámaras de seguridad en mosaico, como Imou Life en la PC, pero en Linux.

Hecha para cámaras Imou (y Dahua, que es la misma marca por dentro): las agregas con el número de serie
y el código de seguridad de la etiqueta, la app las encuentra sola en tu red y las ve directo por RTSP,
sin la nube de Imou ni sus límites. Las de otras casas llegan por Tailscale (escribes su IP).

No viene de fábrica: se instala desde Vortexine's Shop o con «vortex camara instalar».

El motor no depende de Qt (camaras.py, descubrir.py, ptz.py, grabar.py); la interfaz (QML) vive en
__main__.py, puente.py y qml/.
"""

VERSION = "1.0.0"
NOMBRE = "Vortexine's Camera"
APP_ID = "vortex-camera"
