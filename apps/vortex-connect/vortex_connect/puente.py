"""El puente entre la ventana (QML) y el motor. Lo lento corre en hilos y avisa con señales."""
from __future__ import annotations

import threading
import webbrowser

from PySide6.QtCore import Property, QObject, QTimer, Signal, Slot

from . import ES_WINDOWS, viaje
from .motor import Candado, Connect, Estado, SIN_RED
from .proteccion import hay_tailscale
from .redes import ErrorRed, Red

CONDICIONES = "https://www.cloudflare.com/application/terms/"
PRIVACIDAD = "https://www.cloudflare.com/application/privacypolicy/"
BAJAR_WARP = "https://1.1.1.1/"


class Puente(QObject):
    mostrarVentana = Signal()
    cambio = Signal()
    _estadoListo = Signal(object)
    _redesListas = Signal(object, str)
    _texto = Signal(str)
    _portal = Signal(str)
    _fin = Signal()

    def __init__(self, parent: QObject | None = None, connect: Connect | None = None):
        super().__init__(parent)
        self._connect = connect or Connect()
        self._connect.avisar = self._texto.emit
        self._connect.abrir_portal = self._portal.emit
        self._candado = Candado()
        self._estado = Estado()
        self._redes: list[Red] = []
        self._mensaje = ""
        self._ocupado = False
        self._buscando = False
        self._revisando = False
        self._portal_url = ""
        self._cancelar = threading.Event()
        self._hay_wifi = True
        self._estadoListo.connect(self._poner_estado)
        self._redesListas.connect(self._poner_redes)
        self._texto.connect(self._poner_mensaje)
        self._portal.connect(self._poner_portal)
        self._fin.connect(self._terminar)
        self._reloj = QTimer(self)
        self._reloj.setInterval(15000)
        self._reloj.timeout.connect(self.actualizar)
        self._reloj.start()
        self.actualizar()
        self.buscar()

    # --- Lo que muestra ---
    estado = Property(str, lambda s: s._estado.estado, notify=cambio)
    redActual = Property(str, lambda s: s._estado.red.ssid, notify=cambio)
    tipoRed = Property(str, lambda s: s._estado.red.tipo, notify=cambio)
    redAbierta = Property(bool, lambda s: s._estado.red.abierta, notify=cambio)
    portal = Property(str, lambda s: s._portal_url or s._estado.portal, notify=cambio)
    mensaje = Property(str, lambda s: s._mensaje, notify=cambio)
    ocupado = Property(bool, lambda s: s._ocupado, notify=cambio)
    buscando = Property(bool, lambda s: s._buscando, notify=cambio)
    hayWifi = Property(bool, lambda s: s._hay_wifi, notify=cambio)
    esWindows = Property(bool, lambda s: ES_WINDOWS, constant=True)
    condiciones = Property(bool, lambda s: s._connect.ajustes.condiciones, notify=cambio)
    modoViaje = Property(bool, lambda s: s._connect.ajustes.modo_viaje, notify=cambio)
    warpInstalado = Property(bool, lambda s: s._connect.warp.instalado(), notify=cambio)
    hayTailscale = Property(bool, lambda s: hay_tailscale(), notify=cambio)
    redes = Property("QVariantList", lambda s: [
        {"ssid": r.ssid, "senal": r.senal, "conectada": r.conectada} for r in s._redes
    ], notify=cambio)

    # --- Hilos ---
    def _en_hilo(self, trabajo) -> None:
        threading.Thread(target=trabajo, daemon=True).start()

    def _trabajar(self, trabajo) -> None:
        """Lo que cambia la red, de uno en uno; también respecto al modo viaje."""
        if self._ocupado:
            return
        if not self._candado.tomar():
            self._poner_mensaje("El modo viaje está trabajando. Prueba en un momento.")
            return
        self._ocupado = True
        self._cancelar = threading.Event()
        self._mensaje = ""
        self._portal_url = ""
        self.cambio.emit()

        def correr() -> None:
            try:
                resultado = trabajo()
                if resultado is not None:
                    self._estadoListo.emit(resultado)
            except ErrorRed as e:
                self._texto.emit(str(e))
            finally:
                self._fin.emit()

        self._en_hilo(correr)

    @Slot()
    def _terminar(self) -> None:
        self._candado.soltar()
        self._ocupado = False
        self._portal_url = ""
        self.cambio.emit()
        self.actualizar()
        self.buscar(False)

    @Slot(object)
    def _poner_estado(self, estado: Estado) -> None:
        self._revisando = False
        self._estado = estado
        self.cambio.emit()

    @Slot(object, str)
    def _poner_redes(self, redes: list[Red], error: str) -> None:
        self._buscando = False
        self._redes = redes
        if error:
            self._mensaje = error
        self.cambio.emit()

    @Slot(str)
    def _poner_mensaje(self, texto: str) -> None:
        self._mensaje = texto
        self.cambio.emit()

    @Slot(str)
    def _poner_portal(self, url: str) -> None:
        self._portal_url = url
        self.cambio.emit()
        webbrowser.open(url)   # lo acabas de pedir tú: aquí sí se abre de una

    # --- Lo que pide la ventana ---
    @Slot()
    def actualizar(self) -> None:
        if self._ocupado or self._revisando:
            return
        self._revisando = True

        def revisar() -> None:
            try:
                self._estadoListo.emit(self._connect.estado())
            except Exception:   # noqa: BLE001 — la ventana no se cae por una vuelta que falló
                self._estadoListo.emit(Estado(SIN_RED))

        self._en_hilo(revisar)

    @Slot()
    @Slot(bool)
    def buscar(self, de_verdad: bool = True) -> None:
        if self._buscando:
            return
        self._buscando = True
        self.cambio.emit()

        def buscar() -> None:
            try:
                self._hay_wifi = self._connect.redes.hay_wifi()
                self._redesListas.emit(self._connect.redes.lista(de_verdad), "")
            except ErrorRed as e:
                self._redesListas.emit([], str(e))

        self._en_hilo(buscar)

    @Slot(str)
    def conectar(self, ssid: str) -> None:
        self._trabajar(lambda: self._connect.conectar(ssid, self._cancelar))

    @Slot()
    def proteger(self) -> None:
        self._trabajar(lambda: self._connect.asegurar(self._cancelar))

    @Slot()
    def quitarProteccion(self) -> None:
        self._trabajar(self._connect.quitar)

    @Slot()
    def cancelar(self) -> None:
        self._cancelar.set()

    @Slot()
    def abrirPortal(self) -> None:
        if self.portal:
            webbrowser.open(self.portal)

    @Slot()
    def verCondiciones(self) -> None:
        webbrowser.open(CONDICIONES)

    @Slot()
    def verPrivacidad(self) -> None:
        webbrowser.open(PRIVACIDAD)

    @Slot()
    def bajarWarp(self) -> None:
        webbrowser.open(BAJAR_WARP)

    @Slot()
    def aceptarCondiciones(self) -> None:
        self._connect.ajustes.condiciones = True
        self._connect.ajustes.guardar()
        self.cambio.emit()

    @Slot(bool)
    def cambiarModoViaje(self, si: bool) -> None:
        self._connect.ajustes.modo_viaje = si
        self._connect.ajustes.guardar()
        if not viaje.prender(si):
            self._connect.ajustes.modo_viaje = False
            self._connect.ajustes.guardar()
            self._mensaje = "No se pudo prender el modo viaje."
        self.cambio.emit()

    def cerrar(self) -> None:
        self._reloj.stop()
        self._cancelar.set()
        self._candado.soltar()
