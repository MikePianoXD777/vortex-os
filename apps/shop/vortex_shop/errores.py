"""Errores de la tienda. Todos llevan un mensaje en español listo para mostrárselo a la persona."""


class SinRed(Exception):
    """No hay forma de hablar con Flathub y tampoco hay nada guardado en la caché."""

    def __init__(self, mensaje="No hay conexión con Flathub. Revisa tu internet e inténtalo de nuevo."):
        super().__init__(mensaje)


class ErrorFlatpak(Exception):
    """Flatpak no pudo hacer lo que se le pidió (instalar, quitar, actualizar, abrir...)."""


class FlatpakSinRed(ErrorFlatpak, SinRed):
    """Una operación de Flatpak falló por falta de internet. Es a la vez ErrorFlatpak y SinRed:
    lo atrapa cualquiera de los dos manejadores."""

    def __init__(self, mensaje="No hay conexión a internet. Revísala e inténtalo de nuevo."):
        super().__init__(mensaje)


class AppNoEncontrada(ErrorFlatpak):
    """Flathub no tiene esa app (o el identificador no es válido). Es un ErrorFlatpak para que
    cualquier manejador de errores de la interfaz muestre su mensaje."""
