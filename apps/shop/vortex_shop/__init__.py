"""Vortexine's Shop: la tienda de aplicaciones de Vortexine's OS.

Las apps salen de Flathub (Flatpak) y se instalan para tu usuario, sin contraseña y sin terminal:
no ocupan lugar en la ISO. Este paquete es el motor (catalogo.py: qué hay en Flathub; flatpak.py:
instalar, quitar y actualizar); la interfaz (QML) vive en __main__.py, puente.py y qml/.
"""

VERSION = "1.1"
NOMBRE = "Vortexine's Shop"
# Lo que la tienda dice ser en la red (cabecera User-Agent de las peticiones a Flathub).
AGENTE = "vortexines-shop/" + VERSION
