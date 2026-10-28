"""Puente entre el motor (Python, sin Qt) y la ventana (QML): `instalador` en Main.qml.

Lo lento (leer el archivo, buscar en Flathub, descomprimir, instalar) corre en un hilo aparte; los
resultados vuelven a la ventana por señales de Qt, que llegan en el hilo de la interfaz.

Estados (`estado`): «vacio» (sin archivo), «leyendo», «listo» (se ve el archivo y sus botones),
«trabajando» (barra de progreso), «hecho» (resultado) y «error».
"""
from __future__ import annotations

import os
import threading
from pathlib import Path
from urllib.parse import unquote, urlparse

from PySide6.QtCore import Property, QObject, Signal, Slot
from PySide6.QtGui import QGuiApplication

from . import comprimidos, flatpak, motor, textos
from .textos import tr

_TRABAJO = {"descomprimir": "trabajando_descomprimir", "ver_contenido": "trabajando_descomprimir",
            "instalar_programa": "trabajando_instalar", "instalar_appimage": "trabajando_instalar",
            "instalar_flatpak": "trabajando_instalar", "flathub": "trabajando_flathub"}


def ruta_de(texto: str) -> str:
    """Acepta una ruta o una URL file:// (lo que llega al arrastrar)."""
    texto = (texto or "").strip()
    if texto.startswith("file:"):
        return unquote(urlparse(texto).path)
    return os.path.abspath(os.path.expanduser(texto)) if texto else ""


class Instalador(QObject):
    cambio = Signal()
    progresoCambio = Signal()
    instaladosCambio = Signal()
    aviso = Signal(str, str)   # texto, «info» o «error»
    mostrarVentana = Signal()

    _analizado = Signal(object, int)
    _terminado = Signal(object, int)
    _avance = Signal(float, int)
    _deshecho = Signal(object)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._estado = "vacio"
        self._vista: dict = {}
        self._info = None
        self._progreso = -1.0
        self._trabajo = ""
        self._resultado: dict = {}
        self._error: dict = {}
        self._pregunta: dict = {}
        self._contrasena: dict = {}
        self._pendiente: tuple[str, dict] | None = None
        self._instalados: list = []
        self._cancelar = threading.Event()
        self._turno = 0   # cada archivo o trabajo nuevo invalida lo que llegue tarde del anterior
        self._analizado.connect(self._al_analizar)
        self._terminado.connect(self._al_terminar)
        self._avance.connect(self._al_avanzar)
        self._deshecho.connect(self._al_deshacer)
        self.recargarInstalados()

    # ------------------------------------------------------------------ propiedades

    estado = Property(str, lambda s: s._estado, notify=cambio)
    archivo = Property("QVariantMap", lambda s: s._vista, notify=cambio)
    progreso = Property(float, lambda s: s._progreso, notify=progresoCambio)
    trabajo = Property(str, lambda s: s._trabajo, notify=cambio)
    resultado = Property("QVariantMap", lambda s: s._resultado, notify=cambio)
    error = Property("QVariantMap", lambda s: s._error, notify=cambio)
    pregunta = Property("QVariantMap", lambda s: s._pregunta, notify=cambio)
    contrasena = Property("QVariantMap", lambda s: s._contrasena, notify=cambio)
    instalados = Property("QVariantList", lambda s: s._instalados, notify=instaladosCambio)
    t = Property("QVariantMap", lambda s: textos.todos(), constant=True)
    ocupado = Property(bool, lambda s: s._estado == "trabajando", notify=cambio)

    def _poner(self, estado: str, **cambios) -> None:
        self._estado = estado
        for k, v in cambios.items():
            setattr(self, "_" + k, v)
        self.cambio.emit()

    # ------------------------------------------------------------------ abrir un archivo

    @Slot(str)
    def abrir(self, texto: str) -> None:
        ruta = ruta_de(texto)
        if not ruta or self._estado == "trabajando":
            return
        self._turno += 1
        turno = self._turno
        self._poner("leyendo", vista={"nombre": os.path.basename(ruta), "ruta": ruta}, pregunta={}, contrasena={},
                    error={}, resultado={})
        threading.Thread(target=self._analizar, args=(ruta, None, turno), daemon=True).start()

    @Slot("QVariantList")
    def abrirVarios(self, urls) -> None:
        rutas_ = [ruta_de(str(u.toString() if hasattr(u, "toString") else u)) for u in urls]
        rutas_ = [r for r in rutas_ if r]
        if rutas_:
            self.abrir(rutas_[0])
            if len(rutas_) > 1:
                self.aviso.emit(tr("soltar_varios"), "info")

    def _analizar(self, ruta: str, contrasena: str | None, turno: int) -> None:
        try:
            info = motor.analizar(ruta, contrasena)
        except Exception as e:  # noqa: BLE001 (nunca dejar la ventana en «leyendo»)
            info = e
        self._analizado.emit(info, turno)

    def _al_analizar(self, info, turno: int) -> None:
        if turno != self._turno:
            return
        if isinstance(info, Exception):
            self._poner("error", error={"titulo": tr("no_se_pudo_abrir"), "texto": str(info), "comando": ""})
            return
        self._info = info
        self._poner("listo", vista=motor.para_mostrar(info))

    @Slot()
    def cerrarArchivo(self) -> None:
        if self._estado == "trabajando":
            return
        self._turno += 1
        self._info = None
        self._poner("vacio", vista={}, resultado={}, error={}, pregunta={}, contrasena={})

    @Slot()
    def volver(self) -> None:
        """Del error o del resultado, de vuelta al archivo (si sigue ahí) o a la pantalla vacía."""
        if self._info is not None and os.path.exists(self._info.ruta) and self._estado == "error":
            self._poner("listo", error={}, pregunta={}, contrasena={})
        else:
            self.cerrarArchivo()

    # ------------------------------------------------------------------ acciones

    @Slot(str, "QVariantMap")
    def ejecutar(self, accion: str, opciones) -> None:
        if self._info is None or self._estado == "trabajando":
            return
        opciones = dict(opciones or {})
        if opciones.get("carpeta"):   # del selector de carpetas llega como file:///…
            opciones["carpeta"] = ruta_de(str(opciones["carpeta"]))
        if accion == "abrir_carpeta":
            try:
                motor.ejecutar(self._info, accion, opciones)
            except motor.Problema as e:
                self.aviso.emit(str(e), "error")
            return
        self._turno += 1
        self._cancelar = threading.Event()
        self._pendiente = (accion, opciones)
        self._progreso = -1.0
        self._poner("trabajando", trabajo=tr(_TRABAJO.get(accion, "trabajando")), pregunta={}, contrasena={})
        self.progresoCambio.emit()
        threading.Thread(target=self._hacer, args=(accion, opciones, self._turno, self._cancelar), daemon=True).start()

    def _hacer(self, accion: str, opciones: dict, turno: int, cancelar: threading.Event) -> None:
        def progreso(f: float) -> None:
            self._avance.emit(float(f), turno)
        try:
            r = motor.ejecutar(self._info, accion, opciones, progreso, cancelar)
        except Exception as e:  # noqa: BLE001 (se decide qué mostrar en el hilo de la interfaz)
            r = e
        self._terminado.emit(r, turno)

    def _al_avanzar(self, f: float, turno: int) -> None:
        if turno == self._turno and abs(f - self._progreso) >= 0.005:
            self._progreso = f
            self.progresoCambio.emit()

    def _al_terminar(self, r, turno: int) -> None:
        if turno != self._turno:
            return
        accion, opciones = self._pendiente or ("", {})
        if isinstance(r, comprimidos.Conflicto):
            instalacion = accion in ("instalar_appimage", "instalar_programa")
            nombre = self._vista.get("nombre", "") if instalacion else r.ruta.name
            self._poner("listo", pregunta={
                "titulo": tr("reemplazar_titulo" if instalacion else "conflicto_titulo", nombre=nombre),
                "texto": tr("reemplazar_texto") if instalacion else tr("conflicto_texto", sugerido=r.sugerida.name),
                "aceptar": tr("boton_reemplazar") if instalacion else tr("boton_otro_nombre", sugerido=r.sugerida.name),
                "tipo": "peligro" if instalacion else "primario",
                "accion": accion,
                "opciones": dict(opciones, **({"reemplazar": True} if instalacion else {"nombre": r.sugerida.name})),
            })
        elif isinstance(r, comprimidos.NecesitaContrasena):
            self._poner("listo", contrasena={"accion": accion, "opciones": {k: v for k, v in opciones.items()
                                                                            if k != "contrasena"},
                                             "incorrecta": r.incorrecta})
        elif isinstance(r, (comprimidos.Cancelado, flatpak.Cancelado)):
            self._poner("listo")
        elif isinstance(r, motor.Problema):
            self._poner("error", error={"titulo": tr("no_se_pudo_abrir") if not r.comando else tr("falta_titulo"),
                                        "texto": str(r), "comando": r.comando})
        elif isinstance(r, Exception):
            self._poner("error", error={"titulo": tr("no_se_pudo_abrir"),
                                        "texto": tr("error_general", detalle=type(r).__name__), "comando": ""})
        else:
            self._poner("hecho", resultado=r or {})
            self.recargarInstalados()

    @Slot()
    def cancelar(self) -> None:
        self._cancelar.set()

    @Slot()
    def responder(self) -> None:
        """«Sí» a la pregunta (otro nombre o reemplazar): se vuelve a intentar con esa opción."""
        p = self._pregunta
        if p:
            self._pregunta = {}
            self.ejecutar(p["accion"], p["opciones"])

    @Slot()
    def descartarPregunta(self) -> None:
        self._poner(self._estado, pregunta={}, contrasena={})

    @Slot(str)
    def enviarContrasena(self, texto: str) -> None:
        c = self._contrasena
        if c and texto:
            self._contrasena = {}
            self.ejecutar(c["accion"], dict(c["opciones"], contrasena=texto))

    # ------------------------------------------------------------------ resultado

    @Slot()
    def abrirResultado(self) -> None:
        r = self._resultado
        try:
            if r.get("id"):
                motor.abrir_instalado(r["id"])
            elif r.get("flatpak"):
                flatpak.abrir(r["flatpak"])
            elif r.get("carpeta"):
                motor.instalar.abrir_carpeta(Path(r["carpeta"]))
        except (motor.Problema, motor.instalar.ErrorInstalar, OSError) as e:
            self.aviso.emit(str(e), "error")

    @Slot(str)
    def abrirCarpeta(self, ruta: str) -> None:
        try:
            motor.instalar.abrir_carpeta(Path(ruta))
        except motor.instalar.ErrorInstalar as e:
            self.aviso.emit(str(e), "error")

    @Slot(str)
    def copiar(self, texto: str) -> None:
        QGuiApplication.clipboard().setText(texto)

    # ------------------------------------------------------------------ «Instalado»

    @Slot()
    def recargarInstalados(self) -> None:
        self._instalados = motor.instalados()
        self.instaladosCambio.emit()

    @Slot(str)
    def deshacer(self, id_: str) -> None:
        threading.Thread(target=lambda: self._deshecho.emit(self._deshacer(id_)), daemon=True).start()

    @staticmethod
    def _deshacer(id_: str):
        try:
            return ("ok", motor.deshacer(id_))
        except motor.Problema as e:
            return ("error", str(e))

    def _al_deshacer(self, r) -> None:
        self.recargarInstalados()
        self.aviso.emit(r[1], "info" if r[0] == "ok" else "error")

    @Slot(str)
    def abrirInstalado(self, id_: str) -> None:
        try:
            motor.abrir_instalado(id_)
        except (motor.Problema, motor.instalar.ErrorInstalar) as e:
            self.aviso.emit(str(e), "error")
