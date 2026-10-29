"""Vortex Player: música a la máxima calidad, para Vortexine's OS.

Toca tus archivos sin pérdida (FLAC, WAV, ALAC, AIFF…) a su frecuencia y profundidad de bits
originales, y baja canciones y playlists de YouTube, YouTube Music y Spotify (de Spotify solo se leen
los nombres; el audio sale de YouTube Music) sin recomprimirlas. Cada canción dice su calidad real.
El ecualizador es el mismo del panel de música del escritorio, y con audífonos Bluetooth muestra el
códec (LDAC, AAC, SBC) y puede pedir LDAC a 990 kbps.

El motor no depende de Qt (calidad.py, biblioteca.py, descargas.py, audio.py, ecualizador.py); la
interfaz (QML) vive en __main__.py, puente.py, reproductor.py, mpris.py y qml/.
"""

VERSION = "1.1"
NOMBRE = "Vortex Player"
APP_ID = "vortex-player"
