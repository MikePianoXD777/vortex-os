"""Vortex App Installer: instala o descomprime lo que bajaste de internet, sin terminal.

Le das doble clic (o lo arrastras a la ventana) a un archivo y dice qué es y qué puede hacer con él:
descomprimirlo, instalar un AppImage o un programa .tar.gz, instalar un Flatpak, buscar en Flathub
el mismo programa que un .deb o un .rpm, mandar un mod de Minecraft a su instancia… Todo lo que
instala se puede deshacer desde la pestaña «Instalado».

El motor no depende de Qt (detectar.py, comprimidos.py, instalar.py, motor.py…); la interfaz (QML)
vive en __main__.py, puente.py y qml/.
"""

VERSION = "1.0"
NOMBRE = "Vortex App Installer"
APP_ID = "vortex-app-installer"
AGENTE = APP_ID + "/" + VERSION   # cabecera User-Agent de las búsquedas en Flathub
