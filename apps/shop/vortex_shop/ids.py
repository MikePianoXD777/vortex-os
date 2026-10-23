"""Identificadores de aplicación de Flathub (org.mozilla.firefox, com.valvesoftware.Steam...).

Todo id que viene de fuera (de la interfaz, de una búsqueda, de un archivo) pasa por aquí antes de
usarse en una URL o en un comando: nada de espacios, comillas, «;», «$», barras ni nada que un shell
o una URL puedan interpretar. (De todos modos nunca se usa shell=True.)
"""
import re

# Reverse-DNS: segmentos de letras, números, «_» y «-» separados por puntos (al menos dos), sin
# empezar por «-» (que un programa tomaría por una opción). Es más estricto que el de Flatpak a
# propósito: los ids de Flathub son todos así.
_ID = re.compile(r"[A-Za-z0-9_][A-Za-z0-9_-]*(\.[A-Za-z0-9_][A-Za-z0-9_-]*)+\Z")
_MAXIMO = 255


def es_id_valido(app_id):
    """True si app_id tiene la forma de un id de Flathub."""
    return isinstance(app_id, str) and len(app_id) <= _MAXIMO and _ID.match(app_id) is not None


def validar_id(app_id):
    """Devuelve app_id si es válido; si no, ValueError con un mensaje en español."""
    if not es_id_valido(app_id):
        mostrado = repr(app_id)[:60] if isinstance(app_id, str) else type(app_id).__name__
        raise ValueError("«%s» no es un identificador de aplicación válido." % mostrado)
    return app_id
