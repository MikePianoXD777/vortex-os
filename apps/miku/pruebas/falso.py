"""Un Asistente de mentira para probar las ventanas: las mismas señales, sin micrófono, voz ni Claude."""

from PySide6.QtCore import QObject, Signal


class AsistenteFalso(QObject):
    estado = Signal(str, str)
    usuario = Signal(str)
    respuesta = Signal(str, bool)
    aviso = Signal(str)
    nivel = Signal(float)
    boca = Signal(object, int)
    boca_fin = Signal()
    emocion = Signal(str, float)
    sonrojo = Signal(float)
    cerrar = Signal()
    progreso = Signal(str, str, float)
    acciones = Signal(object)

    def __init__(self):
        super().__init__()
        self.estado_actual = "reposo"
        self.registro = []          # lo que la ventana le pidió al asistente

    def enviar(self, t): self.registro.append(("enviar", t))
    def escuchar(self, seguimiento=False): self.registro.append(("escuchar",))
    def cortar_escucha(self): self.registro.append(("cortar",))
    def tocada(self): self.registro.append(("tocada",))
    def accion(self, i): self.registro.append(("accion", i))
