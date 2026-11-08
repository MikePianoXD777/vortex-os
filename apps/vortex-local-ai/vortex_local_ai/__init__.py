"""Vortexine's Local AI: modelos de IA que corren en tu PC (Ollama) y un vault de notas como Obsidian.

Tres partes: el chat con los modelos locales, el catálogo de modelos de Vortexine (bajar, ver cuáles te
caben, borrar) y el vault de notas en Markdown, al que la IA le puede preguntar («Preguntar a mi vault»).

El motor no depende de Qt: rutas.py, ollama.py, catalogo.py, vault.py, memoria.py y conversaciones.py.
La interfaz (QML) vive en __main__.py, puente.py, colores.py y qml/. El contrato entre los dos está en
API.md.
"""

__version__ = "1.0.0"
VERSION = __version__
NOMBRE = "Vortexine's Local AI"
APP_ID = "vortex-local-ai"
