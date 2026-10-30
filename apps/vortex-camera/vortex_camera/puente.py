"""Puente entre el motor (camaras.py, descubrir.py, ptz.py, grabar.py) y QML.

Lo que tarda (buscar en la red, revisar qué cámaras contestan, fotos, mover las PTZ) va en hilos aparte
y el resultado vuelve al hilo de la interfaz con `_enPrincipal`. El código de seguridad nunca pasa a
QML salvo dentro de la URL del video, que QML pide con `urlVivo`/`urlGrabado` cuando la va a usar.
"""
from __future__ import annotations

import re
import sys
import traceback
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta
from typing import Any, Callable

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QDesktopServices

from . import descubrir, grabar, ptz
from .camaras import DIVISIONES, Almacen, limpiar_serie

DIRECCION = re.compile(r"^[A-Za-z0-9.\-:\[\]]+$")   # IPv4, IPv6 o nombre de Tailscale (casa-juanita)
REVISAR_CADA_MS = 30_000


class Puente(QObject):
    aviso = Signal(str, bool)          # texto, y si es un error
    mostrarVentana = Signal()
    camarasCambio = Signal()
    ventanasCambio = Signal()
    estadosCambio = Signal()
    busquedaCambio = Signal()
    grabandoCambio = Signal()
    _enPrincipal = Signal(object)

    def __init__(self, parent: QObject | None = None, almacen: Almacen | None = None, revisar: bool = True):
        super().__init__(parent)
        self._almacen = almacen if almacen is not None else Almacen()
        self._trabajos = ThreadPoolExecutor(max_workers=4, thread_name_prefix="camara")
        self._ptz = ThreadPoolExecutor(max_workers=1, thread_name_prefix="ptz")   # en orden: mover y luego parar
        self._estados: dict[str, str] = {}
        self._grabaciones: dict[str, grabar.Grabacion] = {}
        self._buscando = False
        self._encontradas: list[dict] = []
        self._revisando = False
        self._enPrincipal.connect(lambda f: f())

        self._reloj = QTimer(self)
        self._reloj.setInterval(REVISAR_CADA_MS)
        self._reloj.timeout.connect(self.revisarEstados)
        self._vigia = QTimer(self)            # una grabación que se cortó sola (la cámara se cayó)
        self._vigia.setInterval(3000)
        self._vigia.timeout.connect(self._vigilar_grabaciones)
        self._vigia.start()
        if revisar:
            self._reloj.start()
            QTimer.singleShot(0, self.revisarEstados)
            QTimer.singleShot(0, lambda: self._buscar(silencioso=True))

    # --- hilos ---

    def _lanzar(self, funcion: Callable[[], Any], ok: Callable[[Any], None] | None = None,
                err: Callable[[BaseException], None] | None = None, cola: ThreadPoolExecutor | None = None) -> None:
        def correr() -> None:
            try:
                valor = funcion()
            except BaseException as e:  # noqa: BLE001 - todo error vuelve a la interfaz
                if err is None:
                    traceback.print_exception(type(e), e, e.__traceback__, file=sys.stderr)
                self._enPrincipal.emit(lambda: err(e) if err else self.aviso.emit("Algo salió mal.", True))
                return
            if ok is not None:
                self._enPrincipal.emit(lambda: ok(valor))
        (cola or self._trabajos).submit(correr)

    # --- propiedades ---

    camaras = Property("QVariantList", lambda s: [c.publica() for c in s._almacen.camaras], notify=camarasCambio)
    ventanas = Property("QVariantList", lambda s: [dict(v, celdas=v["celdas"][:v["division"]]) for v in s._almacen.ventanas],
                        notify=ventanasCambio)
    divisiones = Property("QVariantList", lambda s: list(DIVISIONES), constant=True)
    estados = Property("QVariantMap", lambda s: dict(s._estados), notify=estadosCambio)
    grabando = Property("QVariantMap", lambda s: {k: True for k in s._grabaciones}, notify=grabandoCambio)
    buscando = Property(bool, lambda s: s._buscando, notify=busquedaCambio)
    encontradas = Property("QVariantList", lambda s: s._encontradas, notify=busquedaCambio)

    @Slot(str, result="QVariantMap")
    def camara(self, ident: str) -> dict:
        c = self._almacen.camara(ident)
        return c.publica() if c else {}

    # --- video ---

    @Slot(str, bool, result=str)
    def urlVivo(self, ident: str, principal: bool) -> str:
        c = self._almacen.camara(ident)
        return c.url_vivo(principal) if c else ""

    @Slot(str, str, str, result=str)
    def urlGrabado(self, ident: str, fecha: str, hora: str) -> str:
        """Lo grabado en la microSD desde «AAAA-MM-DD» «HH:MM» y hasta 24 horas después (la cámara
        manda lo que tenga y salta los huecos)."""
        c = self._almacen.camara(ident)
        inicio = leer_fecha(fecha, hora)
        if c is None or inicio is None:
            return ""
        return c.url_grabado(inicio, inicio + timedelta(hours=24))

    @Slot(str, str, int, result="QVariantList")
    def moverHora(self, fecha: str, hora: str, minutos: int) -> list:
        """«2026-10-30» «23:55» + 10 min → ["2026-10-31", "00:05"], para los botones de saltar."""
        inicio = leer_fecha(fecha, hora) or datetime.now().replace(second=0, microsecond=0)
        nueva = inicio + timedelta(minutes=minutos)
        return [nueva.strftime("%Y-%m-%d"), nueva.strftime("%H:%M")]

    # --- cámaras ---

    def _validar(self, datos: dict, nueva: bool) -> tuple[dict, str]:
        limpio = {k: datos[k] for k in ("nombre", "serie", "codigo", "ip", "puerto", "canal") if k in datos}
        limpio = {k: v.strip() if isinstance(v, str) else v for k, v in limpio.items()}
        if not limpio.get("nombre"):
            return limpio, "Ponle un nombre a la cámara."
        if nueva and not limpio.get("codigo"):
            return limpio, "Falta el código de seguridad: viene en la etiqueta de la cámara."
        if not nueva and not limpio.get("codigo"):
            limpio.pop("codigo", None)                # en blanco al editar: se queda el que tenía
        if not limpio.get("ip") and not limpio_serie(limpio.get("serie", "")):
            return limpio, "Escribe el número de serie o la IP de la cámara."
        if limpio.get("ip") and not DIRECCION.match(limpio["ip"]):
            return limpio, "Esa IP no se ve bien (algo como 192.168.1.20)."
        serie = limpio_serie(limpio.get("serie", ""))
        otra = self._almacen.por_serie(serie)
        if serie and otra is not None and (nueva or otra.id != datos.get("id")):
            return limpio, f"Esa cámara ya está agregada como «{otra.nombre}»."
        return limpio, ""

    @Slot("QVariantMap", result=str)
    def agregarCamara(self, datos: dict) -> str:
        limpio, error = self._validar(dict(datos), nueva=True)
        if error:
            return error
        c = self._almacen.agregar(limpio)
        self.camarasCambio.emit()
        self.ventanasCambio.emit()
        if not c.ip:
            self._buscar(silencioso=False, nueva=c.id)
        else:
            self._revisar([c.id])
        return ""

    @Slot(str, "QVariantMap", result=str)
    def editarCamara(self, ident: str, datos: dict) -> str:
        limpio, error = self._validar({**dict(datos), "id": ident}, nueva=False)
        if error:
            return error
        c = self._almacen.actualizar(ident, limpio)
        if c is None:
            return "Esa cámara ya no existe."
        self.camarasCambio.emit()
        self.ventanasCambio.emit()          # para que los cuadros vuelvan a pedir su URL
        if c.ip:
            self._revisar([c.id])
        else:
            self._buscar(silencioso=False, nueva=c.id)
        return ""

    @Slot(str)
    def quitarCamara(self, ident: str) -> None:
        if ident in self._grabaciones:
            self._detener_grabacion(ident)
        self._almacen.quitar(ident)
        self._estados.pop(ident, None)
        self.camarasCambio.emit()
        self.ventanasCambio.emit()
        self.estadosCambio.emit()

    # --- buscar en la red y revisar quién contesta ---

    @Slot()
    def buscarEnRed(self) -> None:
        self._buscar(silencioso=False)

    def _buscar(self, silencioso: bool, nueva: str = "") -> None:
        if self._buscando:
            return
        self._buscando = True
        self.busquedaCambio.emit()

        def trabajo() -> list[dict]:
            halladas = descubrir.buscar()
            if not halladas and not silencioso:
                halladas = descubrir.escanear()
            return halladas

        def listo(halladas: list[dict]) -> None:
            self._buscando = False
            cambiaron = []
            for h in halladas:     # la cámara cambió de IP (el módem le dio otra): se corrige sola
                c = self._almacen.por_serie(h["serie"])
                if c is not None and h["ip"] and c.ip != h["ip"]:
                    self._almacen.actualizar(c.id, {"ip": h["ip"]})
                    cambiaron.append(c.id)
            agregadas = {c.serie for c in self._almacen.camaras if c.serie} | {c.ip for c in self._almacen.camaras if c.ip}
            self._encontradas = [dict(h, agregada=bool(h["serie"] in agregadas or h["ip"] in agregadas)) for h in halladas]
            self.busquedaCambio.emit()
            if cambiaron:
                self.camarasCambio.emit()
                self.ventanasCambio.emit()
                self._revisar(cambiaron)
            if nueva:
                c = self._almacen.camara(nueva)
                if c is not None and not c.ip:
                    self._estados[nueva] = "sin_ip"
                    self.estadosCambio.emit()
                    self.aviso.emit(f"No encontré «{c.nombre}» en tu red. Revisa que esté prendida y conectada, "
                                    "o escribe su IP en Editar.", True)
            elif not silencioso and not halladas:
                self.aviso.emit("No encontré cámaras en tu red.", True)

        def fallo(e: BaseException) -> None:
            self._buscando = False
            self.busquedaCambio.emit()
            if not silencioso:
                self.aviso.emit("No se pudo buscar en la red.", True)

        self._lanzar(trabajo, listo, fallo)

    @Slot()
    def revisarEstados(self) -> None:
        self._revisar([c.id for c in self._almacen.camaras])

    def _revisar(self, ids: list[str]) -> None:
        camaras = [c for c in (self._almacen.camara(i) for i in ids) if c is not None]
        if not camaras:
            return
        for c in camaras:
            if not c.ip:
                self._estados[c.id] = "sin_ip"
            elif c.id not in self._estados or self._estados[c.id] == "sin_ip":
                self._estados[c.id] = "revisando"
        self.estadosCambio.emit()
        con_ip = [(c.id, c.ip, c.puerto, f"/cam/realmonitor?channel={c.canal}&subtype=1", c.usuario, c.codigo)
                  for c in camaras if c.ip]
        nombres = {"ok": "en_linea", "codigo": "codigo", "sin_conexion": "sin_conexion", "error": "en_linea"}

        def trabajo() -> dict[str, str]:
            with ThreadPoolExecutor(max_workers=16) as pool:
                res = pool.map(lambda x: descubrir.probar_rtsp(*x[1:]), con_ip)
                return {x[0]: nombres[r] for x, r in zip(con_ip, res)}

        def listo(resultado: dict[str, str]) -> None:
            for ident, estado in resultado.items():
                if self._almacen.camara(ident) is not None:
                    self._estados[ident] = estado
            self.estadosCambio.emit()

        if con_ip:
            self._lanzar(trabajo, listo, lambda e: None)

    # --- fotos y videos ---

    @Slot(str)
    def foto(self, ident: str) -> None:
        c = self._almacen.camara(ident)
        if c is None or not c.ip:
            self.aviso.emit("Esa cámara no tiene IP todavía.", True)
            return
        url, nombre = c.url_vivo(principal=True), c.nombre
        self._lanzar(lambda: grabar.foto(url, grabar.carpeta_fotos(), nombre),
                     lambda ruta: self.aviso.emit(f"Foto guardada: {ruta.name}", False),
                     lambda e: self.aviso.emit(str(e) if isinstance(e, RuntimeError) else "No se pudo sacar la foto.", True))

    @Slot(str)
    def alternarGrabar(self, ident: str) -> None:
        if ident in self._grabaciones:
            self._detener_grabacion(ident)
            return
        c = self._almacen.camara(ident)
        if c is None or not c.ip:
            self.aviso.emit("Esa cámara no tiene IP todavía.", True)
            return
        try:
            self._grabaciones[ident] = grabar.Grabacion(c.url_vivo(principal=True), grabar.carpeta_videos(), c.nombre)
        except (OSError, RuntimeError) as e:
            self.aviso.emit(str(e) if isinstance(e, RuntimeError) else "No se pudo empezar a grabar.", True)
            return
        self.grabandoCambio.emit()

    def _detener_grabacion(self, ident: str) -> None:
        g = self._grabaciones.pop(ident, None)
        self.grabandoCambio.emit()
        if g is None:
            return
        self._lanzar(g.detener, lambda ruta: self.aviso.emit(
            f"Video guardado: {ruta.name}" if ruta else "No se grabó nada.", ruta is None))

    def _vigilar_grabaciones(self) -> None:
        for ident, g in list(self._grabaciones.items()):
            if not g.activa:
                self._grabaciones.pop(ident)
                self.grabandoCambio.emit()
                motivo = g.error()
                ruta = g.detener()
                c = self._almacen.camara(ident)
                self.aviso.emit(f"Se cortó la grabación de «{c.nombre if c else 'la cámara'}»"
                                + (f": {motivo}" if motivo else ".")
                                + (f" Lo grabado quedó en {ruta.name}." if ruta else ""), True)

    @Slot(str)
    def abrirCarpeta(self, tipo: str) -> None:
        carpeta = grabar.carpeta_videos() if tipo == "videos" else grabar.carpeta_fotos()
        carpeta.mkdir(parents=True, exist_ok=True)
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(carpeta)))

    # --- mover las PTZ ---

    @Slot(str, str)
    def moverPtz(self, ident: str, direccion: str) -> None:
        c = self._almacen.camara(ident)
        if c is None or not c.ip or direccion not in ptz.DIRECCIONES:
            return

        def listo(metodo: str) -> None:
            if c.ptz != metodo:
                self._almacen.actualizar(ident, {"ptz": metodo})
                self.camarasCambio.emit()

        def fallo(e: BaseException) -> None:
            if c.ptz == "":    # nunca había funcionado: se marca para esconder las flechas
                self._almacen.actualizar(ident, {"ptz": "no"})
                self.camarasCambio.emit()
                self.aviso.emit(f"«{c.nombre}» no se deja mover desde la PC.", True)
            else:
                self.aviso.emit("No se pudo mover la cámara.", True)

        def trabajo() -> str:
            metodo = ptz.mover(c, direccion)
            c.ptz = metodo           # para que el «parar» que viene en la cola sepa cómo
            return metodo

        self._lanzar(trabajo, listo, fallo, cola=self._ptz)

    @Slot(str, str)
    def pararPtz(self, ident: str, direccion: str) -> None:
        def trabajo() -> None:
            c = self._almacen.camara(ident)
            if c is not None and c.ptz in ("onvif", "cgi"):
                ptz.parar(c, direccion)
        self._lanzar(trabajo, None, lambda e: None, cola=self._ptz)

    @Slot(str)
    def probarPtz(self, ident: str) -> None:
        """«Volver a probar» las flechas de una cámara marcada como que no se mueve."""
        if self._almacen.camara(ident) is not None:
            self._almacen.actualizar(ident, {"ptz": ""})
            self.camarasCambio.emit()

    # --- ventanas del mosaico ---

    @Slot(result=int)
    def nuevaVentana(self) -> int:
        i = self._almacen.nueva_ventana()
        self.ventanasCambio.emit()
        return i

    @Slot(int)
    def quitarVentana(self, i: int) -> None:
        self._almacen.quitar_ventana(i)
        self.ventanasCambio.emit()

    @Slot(int, str)
    def renombrarVentana(self, i: int, nombre: str) -> None:
        self._almacen.renombrar_ventana(i, nombre)
        self.ventanasCambio.emit()

    @Slot(int, int)
    def ponerDivision(self, i: int, division: int) -> None:
        self._almacen.poner_division(i, division)
        self.ventanasCambio.emit()

    @Slot(int, int, str)
    def ponerCelda(self, i: int, pos: int, ident: str) -> None:
        self._almacen.poner_celda(i, pos, ident)
        self.ventanasCambio.emit()

    def cerrar(self) -> None:
        self._reloj.stop()
        self._vigia.stop()
        for g in self._grabaciones.values():
            g.detener()
        self._grabaciones.clear()
        self._trabajos.shutdown(wait=False, cancel_futures=True)
        self._ptz.shutdown(wait=False, cancel_futures=True)


def limpio_serie(serie: Any) -> str:
    return limpiar_serie(str(serie or ""))


def leer_fecha(fecha: str, hora: str) -> datetime | None:
    try:
        return datetime.strptime(f"{fecha.strip()} {hora.strip()}", "%Y-%m-%d %H:%M")
    except ValueError:
        return None
