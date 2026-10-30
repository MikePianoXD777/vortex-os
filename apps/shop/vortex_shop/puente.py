"""Puente entre el motor de Vortexine's Shop (catálogo de Flathub + flatpak) y la interfaz QML.

La interfaz nunca habla con el motor: pide cosas al `Puente` (slots) y lee de él (propiedades con
señal de cambio). Todo lo lento (red, flatpak) corre en hilos y vuelve al hilo de la interfaz con
una señal en cola, así que la ventana no se congela nunca.

Instalar, quitar y actualizar pasan por una cola de una sola vía (un QThreadPool de 1 hilo):
las demás tareas esperan su turno y la tarjeta de cada app muestra «En cola» o su progreso.

El motor se inyecta (`Motor`): en producción son los módulos reales (`catalogo`, `flatpak`,
`errores`); las pruebas de la interfaz usan el doble falso de `pruebas/falso_motor.py`.
"""
from __future__ import annotations

import itertools
import sys
import traceback
from dataclasses import dataclass
from typing import Any, Callable

from PySide6.QtCore import (Property, QObject, QRunnable, QThreadPool, QTimer,
                            Signal, Slot)

from . import propias

# Lo que flatpak instala pero no es una app (runtimes, temas, extensiones): la tienda no lo muestra.
_PREFIJOS_NO_APP = ("org.freedesktop.Platform", "org.freedesktop.Sdk", "org.gnome.Platform",
                    "org.gnome.Sdk", "org.kde.Platform", "org.kde.Sdk", "org.gtk.Gtk3theme",
                    "org.kde.KStyle", "org.kde.PlatformTheme", "org.freedesktop.LinuxAudio",
                    "org.freedesktop.Platform.GL", "org.freedesktop.Platform.VAAPI")
_SUFIJOS_NO_APP = (".Locale", ".Debug", ".Sources", ".Codecs", ".Extension")


@dataclass
class Motor:
    """Las tres piezas del motor, tal como las usa la interfaz."""
    catalogo: Any   # instancia de Catalogo
    flatpak: Any    # módulo (u objeto) con instaladas, actualizaciones, instalar, quitar, actualizar, abrir, preparar
    errores: Any    # módulo (u objeto) con SinRed y ErrorFlatpak


# --- Conversión de los datos del motor a lo que entiende QML -------------------------------------

def _tamano(n: int | None) -> str:
    """Bytes → «120 MB» (con coma decimal si es chico: «1,4 GB»)."""
    if not n or n < 0:
        return ""
    v, i = float(n), 0
    unidades = ["B", "KB", "MB", "GB", "TB"]
    while v >= 1024 and i < len(unidades) - 1:
        v /= 1024
        i += 1
    texto = f"{v:.0f}" if (i <= 1 or v >= 10) else f"{v:.1f}".replace(".", ",")
    return f"{texto} {unidades[i]}"


def _descargas(n: int | None) -> str:
    """Descargas al mes → «12 mil descargas al mes» / «1,2 M descargas al mes»."""
    if n is None or n <= 0:
        return ""
    if n >= 1_000_000:
        base = f"{n / 1_000_000:.1f}".replace(".", ",").removesuffix(",0") + " M"
    elif n >= 1000:
        base = f"{round(n / 1000)} mil"
    else:
        base = str(n)
    return f"{base} descargas al mes"


def _app_a_dict(a: Any) -> dict:
    d = getattr(a, "descargas_mes", None)
    return {
        "id": a.id,
        "nombre": a.nombre or a.id,
        "resumen": a.resumen or "",
        "icono": a.icono_url or "",
        "desarrollador": a.desarrollador or "",
        "verificado": bool(a.verificado),
        "descargas": -1 if d is None else int(d),
        "descargasTexto": _descargas(d),
    }


def _detalle_a_dict(a: Any) -> dict:
    d = _app_a_dict(a)
    d.update({
        "descripcion": a.descripcion or "",
        "capturas": list(a.capturas or []),
        "licencia": a.licencia or "",
        "web": a.web or "",
        "version": a.version or "",
        "tamano": a.tamano_descarga or 0,
        "tamanoTexto": _tamano(a.tamano_descarga),
        "categorias": list(a.categorias or []),
    })
    return d


def _marcador(app_id: str) -> dict:
    """Ficha provisional de una app de la que aún no tenemos datos (solo su id)."""
    nombre = app_id.split(".")[-1].replace("-", " ").replace("_", " ") or app_id
    return {"id": app_id, "nombre": nombre[:1].upper() + nombre[1:], "resumen": "", "icono": "",
            "desarrollador": "", "verificado": False, "descargas": -1, "descargasTexto": ""}


def _es_app(app_id: str) -> bool:
    """Descarta runtimes, temas y extensiones: la tienda solo muestra apps."""
    return not (app_id.startswith(_PREFIJOS_NO_APP) or app_id.endswith(_SUFIJOS_NO_APP))


# --- Hilos ---------------------------------------------------------------------------------------

class _Senales(QObject):
    """Señales que emiten los hilos de trabajo; el Puente (hilo de la interfaz) las recibe en cola."""
    resultado = Signal(int, bool, object)   # (ficha, salió bien, valor o excepción)
    progreso = Signal(str, int, str)        # (app_id, porcentaje, texto)


class _Trabajo(QRunnable):
    def __init__(self, ficha: int, funcion: Callable[[], Any], senales: _Senales):
        super().__init__()
        self.setAutoDelete(False)   # el Puente guarda la referencia hasta que termina
        self._ficha, self._funcion, self._senales = ficha, funcion, senales

    def run(self) -> None:
        try:
            valor = self._funcion()
        except BaseException as e:  # noqa: BLE001 - todo error vuelve a la interfaz, nada muere en silencio
            self._senales.resultado.emit(self._ficha, False, e)
        else:
            self._senales.resultado.emit(self._ficha, True, valor)


_FASES = {"instalar": "instalando", "quitar": "quitando", "actualizar": "actualizando"}


class Puente(QObject):
    # Una señal de cambio por grupo de propiedades.
    estadosCambio = Signal()
    instaladasCambio = Signal()
    actualizacionesCambio = Signal()
    ocupadoCambio = Signal()
    categoriasCambio = Signal()
    inicioCambio = Signal()
    categoriaCambio = Signal()
    busquedaCambio = Signal()
    detalleCambio = Signal()
    propiasCambio = Signal()     # las apps de Vortexine's (Vortexine's Camera…): si están y qué se hace
    aviso = Signal(str)          # mensaje en español para el aviso que se cierra
    mostrarVentana = Signal()    # otra instancia pidió traer esta ventana al frente

    def __init__(self, motor: Motor, parent: QObject | None = None):
        super().__init__(parent)
        self._motor = motor
        self._v: dict[str, Any] = {
            "estados": {}, "instaladasIds": {}, "sistemaIds": {}, "instaladas": [], "actualizablesIds": {},
            "actualizables": [], "numActualizaciones": 0, "revisandoActualizaciones": False,
            "ocupado": False, "categorias": [],
            "inicioEstado": "cargando", "recomendadas": [], "populares": [],
            "categoriaActual": {}, "categoriaApps": [], "categoriaEstado": "cargando",
            "categoriaHayMas": False, "categoriaCargandoMas": False,
            "busquedaTexto": "", "busquedaApps": [], "busquedaEstado": "listo",
            "detalle": {}, "detalleEstado": "cargando",
            "propias": propias.lista(), "propiasEstados": {},
        }
        self._info: dict[str, dict] = {}          # fichas conocidas (id → dict), para listas locales
        self._ids_instaladas: list[str] = []
        self._ids_sistema: set[str] = set()       # instaladas por el sistema: no se pueden quitar
        self._ids_actualizables: list[str] = []
        self._pidiendo: set[str] = set()           # ids cuya ficha se está pidiendo
        self._sin_ficha: set[str] = set()          # ids cuya ficha no se pudo traer (no insistir)
        self._pagina = 1
        self._gen = {"busqueda": 0, "categoria": 0, "detalle": 0}
        self._ficha_sig = itertools.count(1)
        self._pendientes: dict[int, tuple[Callable | None, Callable | None, _Trabajo]] = {}
        self._senales = _Senales(self)
        self._senales.resultado.connect(self._al_terminar)
        self._senales.progreso.connect(self._al_progresar)
        self._pool = QThreadPool(self)             # consultas (red): varias a la vez
        self._pool.setMaxThreadCount(6)
        self._pool_ops = QThreadPool(self)         # instalar/quitar/actualizar: de una en una
        self._pool_ops.setMaxThreadCount(1)
        self._preparado = False
        self._refresco = QTimer(self)              # revisa actualizaciones de vez en cuando
        self._refresco.setInterval(30 * 60 * 1000)
        self._refresco.timeout.connect(self._refrescar_apps_locales)

    # --- Propiedades hacia QML ---

    estados = Property("QVariant", lambda s: s._v["estados"], notify=estadosCambio)
    instaladasIds = Property("QVariant", lambda s: s._v["instaladasIds"], notify=instaladasCambio)
    sistemaIds = Property("QVariant", lambda s: s._v["sistemaIds"], notify=instaladasCambio)
    instaladas = Property("QVariant", lambda s: s._v["instaladas"], notify=instaladasCambio)
    actualizablesIds = Property("QVariant", lambda s: s._v["actualizablesIds"], notify=actualizacionesCambio)
    actualizables = Property("QVariant", lambda s: s._v["actualizables"], notify=actualizacionesCambio)
    numActualizaciones = Property(int, lambda s: s._v["numActualizaciones"], notify=actualizacionesCambio)
    revisandoActualizaciones = Property(bool, lambda s: s._v["revisandoActualizaciones"], notify=actualizacionesCambio)
    ocupado = Property(bool, lambda s: s._v["ocupado"], notify=ocupadoCambio)
    categorias = Property("QVariant", lambda s: s._v["categorias"], notify=categoriasCambio)
    inicioEstado = Property(str, lambda s: s._v["inicioEstado"], notify=inicioCambio)
    recomendadas = Property("QVariant", lambda s: s._v["recomendadas"], notify=inicioCambio)
    populares = Property("QVariant", lambda s: s._v["populares"], notify=inicioCambio)
    categoriaActual = Property("QVariant", lambda s: s._v["categoriaActual"], notify=categoriaCambio)
    categoriaApps = Property("QVariant", lambda s: s._v["categoriaApps"], notify=categoriaCambio)
    categoriaEstado = Property(str, lambda s: s._v["categoriaEstado"], notify=categoriaCambio)
    categoriaHayMas = Property(bool, lambda s: s._v["categoriaHayMas"], notify=categoriaCambio)
    categoriaCargandoMas = Property(bool, lambda s: s._v["categoriaCargandoMas"], notify=categoriaCambio)
    busquedaTexto = Property(str, lambda s: s._v["busquedaTexto"], notify=busquedaCambio)
    busquedaApps = Property("QVariant", lambda s: s._v["busquedaApps"], notify=busquedaCambio)
    busquedaEstado = Property(str, lambda s: s._v["busquedaEstado"], notify=busquedaCambio)
    detalle = Property("QVariant", lambda s: s._v["detalle"], notify=detalleCambio)
    detalleEstado = Property(str, lambda s: s._v["detalleEstado"], notify=detalleCambio)
    propias = Property("QVariant", lambda s: s._v["propias"], notify=propiasCambio)
    propiasEstados = Property("QVariant", lambda s: s._v["propiasEstados"], notify=propiasCambio)

    def _poner(self, senal: Signal, **valores: Any) -> None:
        self._v.update(valores)
        senal.emit()

    # --- Hilos: lanzar una tarea y recibir su resultado en el hilo de la interfaz ---

    def _lanzar(self, funcion: Callable[[], Any], ok: Callable[[Any], None] | None = None,
                err: Callable[[BaseException], None] | None = None, cola: bool = False) -> None:
        ficha = next(self._ficha_sig)
        trabajo = _Trabajo(ficha, funcion, self._senales)
        self._pendientes[ficha] = (ok, err, trabajo)
        (self._pool_ops if cola else self._pool).start(trabajo)

    @Slot(int, bool, object)
    def _al_terminar(self, ficha: int, bien: bool, valor: Any) -> None:
        ok, err, _trabajo = self._pendientes.pop(ficha, (None, None, None))
        try:
            if bien and ok:
                ok(valor)
            elif not bien:
                if err:
                    err(valor)
                else:
                    self._avisar_error(valor)
        except Exception:  # un fallo al pintar no debe tumbar la tienda
            traceback.print_exc()

    def _clasificar(self, e: BaseException) -> tuple[str, str]:
        """Excepción del motor → (estado de la pantalla, texto en español)."""
        if isinstance(e, self._motor.errores.SinRed):
            return "sinred", "Sin conexión a internet. Revisa tu red y vuelve a intentarlo."
        if isinstance(e, self._motor.errores.ErrorFlatpak):
            return "error", str(e) or "No se pudo completar la operación."
        if isinstance(e, propias.ErrorPropia):
            return "error", str(e)
        traceback.print_exception(type(e), e, e.__traceback__, file=sys.stderr)
        return "error", "Algo salió mal. Vuelve a intentarlo en un momento."

    def _avisar_error(self, e: BaseException) -> None:
        self.aviso.emit(self._clasificar(e)[1])

    # --- Arranque ---

    @Slot()
    def iniciar(self) -> None:
        """Carga lo primero que se ve y deja listo flatpak. Se llama una vez, al abrir la ventana."""
        self.cargarCategorias()
        self.cargarInicio()
        self._refrescar_apps_locales()
        self._lanzar(self._preparar_flatpak, lambda _: None, lambda e: print(f"vortex-shop: preparar: {e}", file=sys.stderr))
        self._refresco.start()

    def _preparar_flatpak(self) -> None:
        if not self._preparado:
            self._motor.flatpak.preparar()
            self._preparado = True

    @Slot()
    def reintentarTodo(self) -> None:
        self._sin_ficha.clear()
        self.cargarCategorias()
        self.cargarInicio()
        self._refrescar_apps_locales()

    # --- Inicio y categorías ---

    @Slot()
    def cargarCategorias(self) -> None:
        def ok(cats: list) -> None:
            self._poner(self.categoriasCambio, categorias=[
                {"id": c.id, "nombre": c.nombre, "icono": c.icono} for c in cats])
        self._lanzar(self._motor.catalogo.categorias, ok, lambda e: None)

    @Slot()
    def cargarInicio(self) -> None:
        self._poner(self.inicioCambio, inicioEstado="cargando")
        partes: dict[str, list] = {}

        def llega(clave: str):
            def ok(apps: list) -> None:
                partes[clave] = [_app_a_dict(a) for a in apps]
                for d in partes[clave]:
                    self._recordar(d)
                if len(partes) == 2:
                    self._poner(self.inicioCambio, inicioEstado="listo",
                                recomendadas=partes["recomendadas"], populares=partes["populares"])
            return ok

        def falla(e: BaseException) -> None:
            if self._v["inicioEstado"] == "cargando":  # solo el primer fallo manda
                self._poner(self.inicioCambio, inicioEstado=self._clasificar(e)[0])

        cat = self._motor.catalogo
        self._lanzar(cat.recomendadas, llega("recomendadas"), falla)
        self._lanzar(cat.destacados, llega("populares"), falla)

    @Slot(str)
    def abrirCategoria(self, cat_id: str) -> None:
        nombre = next((c["nombre"] for c in self._v["categorias"] if c["id"] == cat_id), cat_id)
        icono = next((c["icono"] for c in self._v["categorias"] if c["id"] == cat_id), "")
        self._gen["categoria"] += 1
        self._pagina = 1
        self._poner(self.categoriaCambio, categoriaActual={"id": cat_id, "nombre": nombre, "icono": icono},
                    categoriaApps=[], categoriaEstado="cargando", categoriaHayMas=False,
                    categoriaCargandoMas=False)
        self._pedir_pagina(cat_id, 1)

    def _pedir_pagina(self, cat_id: str, pagina: int) -> None:
        gen = self._gen["categoria"]

        def ok(apps: list) -> None:
            if gen != self._gen["categoria"]:
                return
            nuevas = [_app_a_dict(a) for a in apps]
            for d in nuevas:
                self._recordar(d)
            vistas = {a["id"] for a in self._v["categoriaApps"]}
            todas = self._v["categoriaApps"] + [d for d in nuevas if d["id"] not in vistas]
            # Hay más si la página vino llena (el motor da POR_PAGINA) y trajo algo nuevo.
            por_pagina = getattr(self._motor.catalogo, "POR_PAGINA", None)
            lleno = len(nuevas) >= por_pagina if por_pagina else bool(nuevas)
            self._poner(self.categoriaCambio, categoriaApps=todas, categoriaEstado="listo",
                        categoriaHayMas=lleno and len(todas) > len(vistas), categoriaCargandoMas=False)

        def falla(e: BaseException) -> None:
            if gen != self._gen["categoria"]:
                return
            if pagina > 1:  # ya había apps a la vista: solo un aviso
                self._poner(self.categoriaCambio, categoriaCargandoMas=False)
                self._avisar_error(e)
            else:
                self._poner(self.categoriaCambio, categoriaEstado=self._clasificar(e)[0])

        self._lanzar(lambda: self._motor.catalogo.por_categoria(cat_id, pagina), ok, falla)

    @Slot()
    def masDeCategoria(self) -> None:
        cat = self._v["categoriaActual"]
        if not cat or self._v["categoriaCargandoMas"] or not self._v["categoriaHayMas"]:
            return
        self._pagina += 1
        self._poner(self.categoriaCambio, categoriaCargandoMas=True)
        self._pedir_pagina(cat["id"], self._pagina)

    @Slot()
    def reintentarCategoria(self) -> None:
        cat = self._v["categoriaActual"]
        if cat:
            self.abrirCategoria(cat["id"])

    # --- Búsqueda ---

    @Slot(str)
    def buscar(self, texto: str) -> None:
        texto = texto.strip()
        self._gen["busqueda"] += 1
        gen = self._gen["busqueda"]
        if not texto:
            self._poner(self.busquedaCambio, busquedaTexto="", busquedaApps=[], busquedaEstado="listo")
            return
        self._poner(self.busquedaCambio, busquedaTexto=texto, busquedaEstado="cargando")

        def ok(apps: list) -> None:
            if gen != self._gen["busqueda"]:
                return  # ya se escribió otra cosa
            lista = [_app_a_dict(a) for a in apps]
            for d in lista:
                self._recordar(d)
            self._poner(self.busquedaCambio, busquedaApps=lista, busquedaEstado="listo")

        def falla(e: BaseException) -> None:
            if gen == self._gen["busqueda"]:
                self._poner(self.busquedaCambio, busquedaApps=[], busquedaEstado=self._clasificar(e)[0])

        self._lanzar(lambda: self._motor.catalogo.buscar(texto), ok, falla)

    @Slot()
    def reintentarBusqueda(self) -> None:
        self.buscar(self._v["busquedaTexto"])

    # --- Detalle de una app ---

    @Slot(str)
    def abrirDetalle(self, app_id: str) -> None:
        self._gen["detalle"] += 1
        gen = self._gen["detalle"]
        base = dict(self._info.get(app_id) or _marcador(app_id))
        base.update({"descripcion": "", "capturas": [], "licencia": "", "web": "", "version": "",
                     "tamano": 0, "tamanoTexto": "", "categorias": []})
        self._poner(self.detalleCambio, detalle=base, detalleEstado="cargando")

        def ok(d: Any) -> None:
            if gen != self._gen["detalle"]:
                return
            ficha = _detalle_a_dict(d)
            self._recordar(ficha)
            self._poner(self.detalleCambio, detalle=ficha, detalleEstado="listo")
            self._reconstruir_listas()

        def falla(e: BaseException) -> None:
            if gen == self._gen["detalle"]:
                self._poner(self.detalleCambio, detalleEstado=self._clasificar(e)[0])

        self._lanzar(lambda: self._motor.catalogo.detalle(app_id), ok, falla)

    @Slot()
    def reintentarDetalle(self) -> None:
        d = self._v["detalle"]
        if d:
            self.abrirDetalle(d["id"])

    # --- Fichas conocidas, instaladas y actualizaciones ---

    _CLAVES_BASE = ("id", "nombre", "resumen", "icono", "desarrollador", "verificado",
                    "descargas", "descargasTexto")

    def _recordar(self, ficha: dict) -> None:
        self._info[ficha["id"]] = {k: ficha[k] for k in self._CLAVES_BASE}

    def _refrescar_apps_locales(self) -> None:
        self._refrescar_instaladas()
        self._refrescar_actualizaciones()
        self._poner(self.propiasCambio, propias=propias.lista())

    # --- Apps de Vortexine's (no son de Flathub: las instala «vortex <app> instalar») ---

    def _estado_propia(self, app_id: str, fase: str | None) -> None:
        estados = dict(self._v["propiasEstados"])
        if fase:
            estados[app_id] = fase
        else:
            estados.pop(app_id, None)
        self._poner(self.propiasCambio, propiasEstados=estados)

    def _accion_propia(self, app_id: str, fase: str, funcion: Callable[[str], None]) -> None:
        if app_id in self._v["propiasEstados"]:
            return
        self._estado_propia(app_id, fase)

        def listo(_: Any) -> None:
            self._estado_propia(app_id, None)
            self._poner(self.propiasCambio, propias=propias.lista())
            nombre = propias.app(app_id)["nombre"]
            self.aviso.emit(f"{nombre} quedó instalada: ábrela desde aquí o desde el lanzador." if fase == "instalando"
                            else f"{nombre} se quitó.")

        def fallo(e: BaseException) -> None:
            self._estado_propia(app_id, None)
            self._avisar_error(e)

        self._lanzar(lambda: funcion(app_id), listo, fallo, cola=True)

    @Slot(str)
    def instalarPropia(self, app_id: str) -> None:
        self._accion_propia(app_id, "instalando", propias.instalar)

    @Slot(str)
    def quitarPropia(self, app_id: str) -> None:
        self._accion_propia(app_id, "quitando", propias.quitar)

    @Slot(str)
    def abrirPropia(self, app_id: str) -> None:
        if app_id in self._v["propiasEstados"]:
            return
        self._estado_propia(app_id, "abriendo")
        QTimer.singleShot(4000, lambda: self._estado_propia(app_id, None) if self._v["propiasEstados"].get(app_id) == "abriendo" else None)
        self._lanzar(lambda: propias.abrir(app_id), None, lambda e: (self._estado_propia(app_id, None), self._avisar_error(e)))

    def _refrescar_instaladas(self) -> None:
        fl = self._motor.flatpak

        def leer() -> tuple[list[str], set[str]]:   # en el hilo: flatpak bloquea
            ids = [i for i in fl.instaladas() if _es_app(i)]
            sistema = set()
            for i in ids:
                try:
                    if fl.es_del_sistema(i):
                        sistema.add(i)
                except Exception:  # noqa: BLE001 - sin saberlo, se deja quitar (y flatpak dirá si no)
                    pass
            return ids, sistema

        def ok(res: tuple[list[str], set[str]]) -> None:
            self._ids_instaladas, self._ids_sistema = res
            self._reconstruir_listas()
        self._lanzar(leer, ok, lambda e: self._avisar_error(e))

    def _refrescar_actualizaciones(self) -> None:
        self._poner(self.actualizacionesCambio, revisandoActualizaciones=True)

        def ok(ids: list) -> None:
            self._ids_actualizables = [i for i in ids if _es_app(i)]
            self._v["revisandoActualizaciones"] = False
            self._reconstruir_listas()

        def falla(_e: BaseException) -> None:  # sin red no se sabe: se deja lo anterior, sin molestar
            self._poner(self.actualizacionesCambio, revisandoActualizaciones=False)

        self._lanzar(self._motor.flatpak.actualizaciones, ok, falla)

    @Slot()
    def buscarActualizaciones(self) -> None:
        self._refrescar_actualizaciones()

    def _reconstruir_listas(self) -> None:
        """Rearma «Instaladas» y «Actualizaciones» con lo que se sabe y pide lo que falta."""
        def ficha(i: str) -> dict:
            return self._info.get(i) or _marcador(i)

        def ordenada(ids: list[str]) -> list[dict]:
            return sorted((ficha(i) for i in ids), key=lambda d: d["nombre"].casefold())

        faltan = [i for i in dict.fromkeys(self._ids_instaladas + self._ids_actualizables)
                  if i not in self._info and i not in self._pidiendo and i not in self._sin_ficha]
        for i in faltan:
            self._pedir_ficha(i)
        self._v["instaladas"] = ordenada(self._ids_instaladas)
        self._v["instaladasIds"] = {i: True for i in self._ids_instaladas}
        self._v["sistemaIds"] = {i: True for i in self._ids_sistema}
        self._v["actualizables"] = ordenada(self._ids_actualizables)
        self._v["actualizablesIds"] = {i: True for i in self._ids_actualizables}
        self._v["numActualizaciones"] = len(self._ids_actualizables)
        self.instaladasCambio.emit()
        self.actualizacionesCambio.emit()

    def _pedir_ficha(self, app_id: str) -> None:
        self._pidiendo.add(app_id)

        def ok(d: Any) -> None:
            self._pidiendo.discard(app_id)
            self._recordar(_app_a_dict(d))
            self._reconstruir_listas()

        def falla(_e: BaseException) -> None:  # sin red: se queda con el nombre sacado del id
            self._pidiendo.discard(app_id)
            self._sin_ficha.add(app_id)

        self._lanzar(lambda: self._motor.catalogo.detalle(app_id), ok, falla)

    # --- Operaciones sobre apps (cola de una en una) ---

    @Slot(str)
    def instalar(self, app_id: str) -> None:
        self._encolar("instalar", app_id)

    @Slot(str)
    def quitar(self, app_id: str) -> None:
        self._encolar("quitar", app_id)

    @Slot(str)
    def actualizar(self, app_id: str) -> None:
        self._encolar("actualizar", app_id)

    @Slot()
    def actualizarTodo(self) -> None:
        for i in list(self._ids_actualizables):
            self._encolar("actualizar", i)

    def _encolar(self, tipo: str, app_id: str) -> None:
        if app_id in self._v["estados"] and self._v["estados"][app_id]["fase"] != "abriendo":
            return  # ya está en la cola o en curso
        self._poner_estado(app_id, {"fase": "cola", "tipo": tipo, "pct": 0, "texto": "En cola"})
        fl = self._motor.flatpak

        def tarea() -> None:
            self._senales.progreso.emit(app_id, 0, "Preparando…")
            self._preparar_flatpak()

            def progreso(pct: int, texto: str) -> None:
                self._senales.progreso.emit(app_id, int(pct), str(texto))

            if tipo == "instalar":
                fl.instalar(app_id, progreso)
            elif tipo == "quitar":
                fl.quitar(app_id, progreso)
            else:
                fl.actualizar([app_id], progreso)

        def ok(_: Any) -> None:
            self._quitar_estado(app_id)
            if tipo == "instalar" and app_id not in self._ids_instaladas:
                self._ids_instaladas.append(app_id)
            elif tipo == "quitar":
                self._ids_instaladas = [i for i in self._ids_instaladas if i != app_id]
            if tipo in ("actualizar", "quitar"):
                self._ids_actualizables = [i for i in self._ids_actualizables if i != app_id]
            self._reconstruir_listas()
            self._refrescar_apps_locales()  # y se confirma con lo que diga flatpak de verdad

        def err(e: BaseException) -> None:
            self._quitar_estado(app_id)
            self._avisar_error(e)

        self._lanzar(tarea, ok, err, cola=True)

    @Slot(str, int, str)
    def _al_progresar(self, app_id: str, pct: int, texto: str) -> None:
        est = self._v["estados"].get(app_id)
        if not est or est["fase"] == "abriendo":
            return
        nuevo = {"fase": _FASES[est["tipo"]], "tipo": est["tipo"], "pct": max(0, min(100, pct)), "texto": texto}
        if nuevo != est:
            self._poner_estado(app_id, nuevo)

    def _poner_estado(self, app_id: str, est: dict) -> None:
        self._v["estados"] = {**self._v["estados"], app_id: est}   # objeto nuevo: QML se entera
        self._estados_cambiaron()

    def _quitar_estado(self, app_id: str) -> None:
        self._v["estados"] = {k: v for k, v in self._v["estados"].items() if k != app_id}
        self._estados_cambiaron()

    def _estados_cambiaron(self) -> None:
        self.estadosCambio.emit()
        ocupado = any(e["fase"] != "abriendo" for e in self._v["estados"].values())
        if ocupado != self._v["ocupado"]:
            self._poner(self.ocupadoCambio, ocupado=ocupado)

    def _dejar_de_abrir(self, app_id: str) -> None:
        if self._v["estados"].get(app_id, {}).get("fase") == "abriendo":
            self._quitar_estado(app_id)

    @Slot(str)
    def abrir(self, app_id: str) -> None:
        if app_id in self._v["estados"]:
            return
        self._poner_estado(app_id, {"fase": "abriendo", "tipo": "abrir", "pct": 0, "texto": "Abriendo…"})
        # La app tarda unos segundos en aparecer: el botón queda en «Abriendo…» para no abrirla dos veces.
        QTimer.singleShot(4000, lambda: self._dejar_de_abrir(app_id))

        def err(e: BaseException) -> None:
            self._quitar_estado(app_id)
            self._avisar_error(e)

        self._lanzar(lambda: self._motor.flatpak.abrir(app_id), None, err)
