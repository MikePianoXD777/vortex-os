"""Puente entre el motor (sistema.py, servicios.py) y QML.

Cada segundo (cada 3 si la ventana no está enfocada) mide en un hilo aparte —psutil, hyprctl y
nvidia-smi tardan unas decenas de milisegundos— y la interfaz solo recibe el resultado. Las acciones que
pueden pedir contraseña (pkexec, systemctl del sistema) también van en otro hilo, para no congelarla.
"""
from __future__ import annotations

import os
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QDesktopServices, QGuiApplication, QIcon

from . import servicios, sistema
from .modelo import ModeloFilas

HISTORIA = 60
SECCIONES = (("apps", "Apps"), ("fondo", "Segundo plano"), ("sistema", "Del sistema"))


def _empujar(lista: list, valor: float) -> list:
    lista.append(valor)
    del lista[:-HISTORIA]
    return lista


class Puente(QObject):
    aviso = Signal(str)
    mostrarVentana = Signal()
    procesosCambio = Signal()
    rendimientoCambio = Signal()
    serviciosCambio = Signal()
    vistaCambio = Signal()
    _llegoMuestra = Signal(object)
    _llegoServicios = Signal(object)
    _termino = Signal(str, bool)       # error («» si salió bien), y si hay que recargar servicios

    def __init__(self, parent: QObject | None = None, apps: sistema.Aplicaciones | None = None):
        super().__init__(parent)
        self.modelo = ModeloFilas(self)
        self._procesos = sistema.Procesos()
        self._rend = sistema.Rendimiento()
        self._apps = apps if apps is not None else sistema.Aplicaciones()
        self._medir = ThreadPoolExecutor(max_workers=1, thread_name_prefix="medir")
        self._trabajos = ThreadPoolExecutor(max_workers=2, thread_name_prefix="accion")
        self._ocupado = False
        self._grupos: dict[str, list[dict]] = {"apps": [], "fondo": [], "sistema": []}
        self._por_clave: dict[str, dict] = {}
        self._expandidos: set[str] = set()
        self._colapsadas: set[str] = set()
        self._filtro = ""
        self._orden = "nombre"
        self._ver_procesos = True
        self._enfocada = True
        self._iconos: dict[str, str] = {}
        self._totales = {"cpu": 0.0, "mem": 0.0, "disco": 0.0, "procesos": 0}
        self._resumen: dict = {}
        self._rendimiento: dict = {}
        self._historia: dict = {"cpu": [], "mem": [], "nucleos": [], "discos": {}, "redes": {}, "gpus": {}}
        self._servicios = {"usuario": [], "sistema": []}
        self._cargando_servicios = False
        self._ticks = 0

        self._llegoMuestra.connect(self._alLlegar)
        self._llegoServicios.connect(self._alLlegarServicios)
        self._termino.connect(self._alTerminar)
        self._reloj = QTimer(self)
        self._reloj.setInterval(1000)
        self._reloj.timeout.connect(self._tick)
        self._reloj.start()
        self._tick()

    # --- muestreo ---

    def _tick(self) -> None:
        self._ticks += 1
        if self._ocupado or (not self._enfocada and self._ticks % 3):
            return
        self._ocupado = True
        con_procesos = self._ver_procesos or not self._por_clave or self._ticks % 5 == 0
        self._medir.submit(self._muestrear, con_procesos)

    def _muestrear(self, con_procesos: bool) -> None:
        try:
            filas = self._procesos.muestrear() if con_procesos else None
            grupos = sistema.agrupar(filas, sistema.ventanas_hyprland(), self._apps) if filas is not None else None
            n = len(filas) if filas is not None else self._totales["procesos"]
            hilos = sum(f["hilos"] for f in filas) if filas is not None else self._rendimiento.get("cpu", {}).get("hilos", 0)
            self._llegoMuestra.emit({"grupos": grupos, "rend": self._rend.muestrear(n, hilos)})
        except Exception as e:  # que un error raro de psutil no detenga el reloj
            self._llegoMuestra.emit({"error": str(e)})

    def _alLlegar(self, res: dict) -> None:
        self._ocupado = False
        if "error" in res:
            print("Vortexine Task Manager:", res["error"])
            return
        rend = res["rend"]
        if res["grupos"] is not None:
            self._grupos = res["grupos"]
            self._por_clave = {g["clave"]: g for s in self._grupos.values() for g in s}
            todos = [g for s in self._grupos.values() for g in s]
            self._totales = {"cpu": rend["cpu"]["uso"], "mem": rend["mem"]["uso"],
                             "disco": sum(g["disco"] for g in todos), "procesos": rend["cpu"]["procesos"]}
            apps = [g for g in todos if os.getpid() not in g["pids"]]     # sin contarme a mí
            top_cpu = max(apps, key=lambda g: g["cpu"], default=None)
            top_mem = max(apps, key=lambda g: g["mem"], default=None)
            self._resumen = {
                "cpu": rend["cpu"]["uso"], "memUsada": rend["mem"]["usada"], "memTotal": rend["mem"]["total"],
                "topCpu": {"nombre": top_cpu["nombre"], "valor": top_cpu["cpu"], "icono": self._icono(top_cpu["icono"])} if top_cpu else {},
                "topMem": {"nombre": top_mem["nombre"], "valor": top_mem["mem"], "icono": self._icono(top_mem["icono"])} if top_mem else {},
            }
            self._reconstruir()
        else:
            self._totales["cpu"], self._totales["mem"] = rend["cpu"]["uso"], rend["mem"]["uso"]
        self._guardarHistoria(rend)
        self._rendimiento = rend
        self.rendimientoCambio.emit()

    def _guardarHistoria(self, r: dict) -> None:
        h = self._historia
        _empujar(h["cpu"], r["cpu"]["uso"])
        _empujar(h["mem"], r["mem"]["uso"])
        while len(h["nucleos"]) < len(r["cpu"]["nucleos"]):
            h["nucleos"].append([])
        for i, v in enumerate(r["cpu"]["nucleos"]):
            _empujar(h["nucleos"][i], v)
        for d in r["discos"]:
            hd = h["discos"].setdefault(d["id"], {"activo": [], "lectura": [], "escritura": []})
            for k in hd:
                _empujar(hd[k], d[k])
        for n in r["redes"]:
            hn = h["redes"].setdefault(n["id"], {"envio": [], "recepcion": []})
            for k in hn:
                _empujar(hn[k], n[k])
        for g in r["gpus"]:
            hg = h["gpus"].setdefault(g["id"], {"uso": [], "vram": []})
            _empujar(hg["uso"], max(0.0, g["uso"]))
            _empujar(hg["vram"], 100 * g["vramUsada"] / g["vramTotal"] if g["vramTotal"] else 0.0)

    # --- la lista de procesos ---

    def _icono(self, nombre: str) -> str:
        """El nombre (o la ruta) si hay ícono para él; si no, «» y QML pone el genérico."""
        if not nombre:
            return ""
        if nombre not in self._iconos:
            if nombre.startswith("/"):
                valido = os.path.isfile(nombre)
            else:
                valido = QIcon.hasThemeIcon(nombre) or QIcon.hasThemeIcon(nombre.lower())
            self._iconos[nombre] = nombre if valido else ""
        return self._iconos[nombre]

    def _reconstruir(self) -> None:
        filas: list[dict] = []
        for seccion, titulo in SECCIONES:
            grupos = [g for g in self._grupos.get(seccion, []) if sistema.coincide(g, self._filtro)]
            if not grupos:
                continue
            colapsada = seccion in self._colapsadas and not self._filtro
            filas.append({"tipo": "seccion", "clave": "s:" + seccion, "seccion": seccion, "nombre": titulo,
                          "cantidad": len(grupos), "colapsada": colapsada,
                          "cpu": sum(g["cpu"] for g in grupos), "mem": sum(g["mem"] for g in grupos)})
            if colapsada:
                continue
            for g in sistema.ordenar(grupos, self._orden):
                n = len(g["procesos"])
                expandido = g["clave"] in self._expandidos and n > 1
                filas.append({"tipo": "grupo", "clave": g["clave"], "seccion": seccion, "nombre": g["nombre"],
                              "icono": self._icono(g["icono"]), "subtitulo": g["subtitulo"], "cpu": g["cpu"],
                              "mem": g["mem"], "disco": g["disco"], "n": n, "expandido": expandido,
                              "suspendido": g["suspendido"], "ventana": bool(g["ventanas"]), "usuario": g["usuario"]})
                if expandido:
                    for p in sistema.ordenar([{**p, "clave": str(p["pid"])} for p in g["procesos"]], self._orden):
                        filas.append({"tipo": "proceso", "clave": f"{g['clave']}/{p['pid']}", "seccion": seccion,
                                      "padre": g["clave"], "nombre": p["nombre"], "pid": p["pid"], "cpu": p["cpu"],
                                      "mem": p["mem"], "disco": p["disco"], "usuario": p["usuario"],
                                      "suspendido": p["estado"] == "stopped", "subtitulo": ""})
        self.modelo.actualizar(filas)
        self.procesosCambio.emit()

    def _pids(self, clave: str) -> list[int]:
        if "/" in clave:
            return [int(clave.rsplit("/", 1)[1])]
        g = self._por_clave.get(clave)
        return list(g["pids"]) if g else []

    # --- propiedades ---

    filas = Property(QObject, lambda s: s.modelo, constant=True)
    totales = Property("QVariantMap", lambda s: s._totales, notify=procesosCambio)
    resumen = Property("QVariantMap", lambda s: s._resumen, notify=procesosCambio)
    rendimiento = Property("QVariantMap", lambda s: s._rendimiento, notify=rendimientoCambio)
    historia = Property("QVariantMap", lambda s: s._historia, notify=rendimientoCambio)
    serviciosUsuario = Property("QVariantList", lambda s: s._servicios["usuario"], notify=serviciosCambio)
    serviciosSistema = Property("QVariantList", lambda s: s._servicios["sistema"], notify=serviciosCambio)
    cargandoServicios = Property(bool, lambda s: s._cargando_servicios, notify=serviciosCambio)

    def _poner_filtro(self, v: str) -> None:
        if v != self._filtro:
            self._filtro = v
            self._reconstruir()
            self.vistaCambio.emit()

    def _poner_orden(self, v: str) -> None:
        if v in sistema.ORDENES and v != self._orden:
            self._orden = v
            self._reconstruir()
            self.vistaCambio.emit()

    def _poner_ver(self, v: bool) -> None:
        if v != self._ver_procesos:
            self._ver_procesos = v
            self.vistaCambio.emit()
            if v:
                self._tick()

    def _poner_enfocada(self, v: bool) -> None:
        if v != self._enfocada:
            self._enfocada = v
            self.vistaCambio.emit()

    filtro = Property(str, lambda s: s._filtro, _poner_filtro, notify=vistaCambio)
    orden = Property(str, lambda s: s._orden, _poner_orden, notify=vistaCambio)
    verProcesos = Property(bool, lambda s: s._ver_procesos, _poner_ver, notify=vistaCambio)
    enfocada = Property(bool, lambda s: s._enfocada, _poner_enfocada, notify=vistaCambio)

    # --- acciones de la lista ---

    @Slot(str)
    def alternarGrupo(self, clave: str) -> None:
        self._expandidos.symmetric_difference_update({clave})
        self._reconstruir()

    @Slot(str)
    def alternarSeccion(self, seccion: str) -> None:
        self._colapsadas.symmetric_difference_update({seccion})
        self._reconstruir()

    @Slot(str, result="QVariantMap")
    def detalles(self, clave: str) -> dict:
        pids = self._pids(clave)
        if not pids:
            return {}
        g = self._por_clave.get(clave.split("/")[0], {})
        proc = next((p for p in g.get("procesos", []) if p["pid"] == pids[0]), {})
        return {"pids": pids, "pid": pids[0], "comando": proc.get("comando", ""), "usuario": proc.get("usuario", ""),
                "prioridad": sistema.prioridad_de(pids[0]), "ubicacion": sistema.ubicacion(pids[0]),
                "ventana": bool(g.get("ventanas")) and "/" not in clave, "suspendido": all(
                    p["estado"] == "stopped" for p in g.get("procesos", []) if p["pid"] in pids)}

    def _en_fondo(self, trabajo, *args, servicios_tambien: bool = False) -> None:
        def correr() -> None:
            try:
                error = trabajo(*args)
            except Exception as e:
                error = str(e)
            self._termino.emit(error or "", servicios_tambien)
        self._trabajos.submit(correr)

    def _alTerminar(self, error: str, recargar_servicios: bool) -> None:
        if error:
            self.aviso.emit(error)
        self._tick()
        if recargar_servicios:
            self.cargarServicios()

    @Slot(str, bool)
    def finalizar(self, clave: str, forzar: bool) -> None:
        """Finalizar tarea: la app y todos sus procesos (SIGTERM; con «forzar», SIGKILL)."""
        pids = self._pids(clave)
        if os.getpid() in pids and not forzar:
            pids.remove(os.getpid())      # que «finalizar» el grupo donde estoy no me cierre a medias
        if pids:
            self._en_fondo(sistema.terminar, pids, forzar)

    @Slot(str)
    def cerrarVentana(self, clave: str) -> None:
        g = self._por_clave.get(clave)
        if g and g["ventanas"]:
            self._en_fondo(lambda: sistema.cerrar_ventanas(g["ventanas"]))

    @Slot(str, bool)
    def suspender(self, clave: str, si: bool) -> None:
        pids = [p for p in self._pids(clave) if p != os.getpid()]
        if pids:
            self._en_fondo(sistema.suspender, pids, si)

    @Slot(str, str)
    def ponerPrioridad(self, clave: str, nivel: str) -> None:
        if nivel in sistema.PRIORIDADES:
            self._en_fondo(sistema.prioridad, self._pids(clave), nivel)

    @Slot(str)
    def abrirUbicacion(self, clave: str) -> None:
        pids = self._pids(clave)
        ruta = sistema.ubicacion(pids[0]) if pids else ""
        if ruta:
            QDesktopServices.openUrl(QUrl.fromLocalFile(str(Path(ruta).parent)))
        else:
            self.aviso.emit("No se puede ver de dónde es ese proceso (es del sistema).")

    @Slot(str)
    def copiar(self, texto: str) -> None:
        QGuiApplication.clipboard().setText(texto)

    # --- servicios ---

    @Slot()
    def cargarServicios(self) -> None:
        if self._cargando_servicios:
            return
        self._cargando_servicios = True
        self.serviciosCambio.emit()
        self._trabajos.submit(lambda: self._llegoServicios.emit(
            {"usuario": servicios.listar(True), "sistema": servicios.listar(False)}))

    def _alLlegarServicios(self, datos: dict) -> None:
        self._servicios = datos
        self._cargando_servicios = False
        self.serviciosCambio.emit()

    @Slot(str, str, bool)
    def servicio(self, unidad: str, que: str, usuario: bool) -> None:
        self._en_fondo(servicios.accion, unidad, que, usuario, servicios_tambien=True)

    def cerrar(self) -> None:
        self._reloj.stop()
        self._medir.shutdown(wait=False, cancel_futures=True)
        self._trabajos.shutdown(wait=False, cancel_futures=True)
