"""El puente entre el motor (biblioteca, correr, protones) y la interfaz QML.

Un solo objeto, «runner» en QML. La lista de programas va como lista de diccionarios (programas), con
lo que la interfaz necesita ya calculado: el ícono como URL, si está abierto y el texto de abajo
(«Jugaste hace 2 h · 5 h en total»). Cada 3 s se mira qué programas siguen abiertos (sus procesos con
su WINEPREFIX), para el botón Jugar/Detener y para sumar el tiempo de juego.

Lo lento (instalar el motor con pacman) va en un hilo y avisa con una señal.
"""
from __future__ import annotations

import os
import subprocess
import threading
import time
from pathlib import Path

from PySide6.QtCore import Property, QObject, QTimer, QUrl, Signal, Slot

from . import biblioteca, correr, protones, rutas


def _ruta(url_o_ruta: str) -> str:
    if url_o_ruta.startswith("file:"):
        return QUrl(url_o_ruta).toLocalFile()
    return url_o_ruta


def _hace(ts: int) -> str:
    if not ts:
        return ""
    s = max(0, time.time() - ts)
    if s < 120:
        return "hace un momento"
    if s < 3600:
        return f"hace {int(s // 60)} min"
    if s < 86400:
        return f"hace {int(s // 3600)} h"
    d = int(s // 86400)
    return "ayer" if d == 1 else f"hace {d} días"


def _total(minutos: int) -> str:
    if minutos < 60:
        return f"{minutos} min" if minutos else ""
    h = minutos / 60
    return f"{h:.1f} h".replace(".0 h", " h") if h < 10 else f"{int(h)} h"


def subtitulo(e: dict, abierto: bool) -> str:
    if abierto:
        return "Abierto ahora"
    if not e.get("ultima_vez"):
        return "Nunca abierto"
    partes = [f"Abierto {_hace(e['ultima_vez'])}"]
    t = _total(int(e.get("minutos", 0)))
    if t:
        partes.append(f"{t} en total")
    return " · ".join(partes)


class Runner(QObject):
    programasCambio = Signal()
    motorCambio = Signal()
    instalandoCambio = Signal()
    aviso = Signal(str, str)                 # texto, tipo (info | error | exito)
    elegirInstalado = Signal("QVariantList")  # los .exe que dejó un instalador, para elegir
    preguntarArchivo = Signal(str, str, bool)  # ruta, nombre, parece instalador
    mostrarVentana = Signal()
    _motorListo = Signal(int, str)

    def __init__(self, parent=None):
        super().__init__(parent)
        self._programas: list[dict] = []
        self._abiertos: set[str] = set()
        self._inicios: dict[str, float] = {}      # id → cuándo lo vimos abrirse (para sumar el tiempo)
        self._falta: list[str] = correr.falta_motor()
        self._instalando_motor = False
        self._instalando: dict | None = None       # {nombre, prefijo, antes, proceso}
        self._protones = protones.lista()
        self._motorListo.connect(self._fin_motor)
        self._recargar()
        self._reloj = QTimer(self)
        self._reloj.setInterval(3000)
        self._reloj.timeout.connect(self._revisar)
        self._reloj.start()

    # ---- estado -----------------------------------------------------------------------------

    def _fila(self, e: dict) -> dict:
        abierto = e["id"] in self._abiertos
        ic = e.get("icono") or ""
        return {
            "id": e["id"], "nombre": e["nombre"], "exe": e["exe"], "prefijo": e["prefijo"],
            "proton": e.get("proton") or "", "argumentos": e.get("argumentos") or "",
            "icono": QUrl.fromLocalFile(ic).toString() if ic and Path(ic).is_file() else "",
            "abierto": abierto, "subtitulo": subtitulo(e, abierto), "lanzador": bool(e.get("lanzador")),
            "existe": Path(e["exe"]).exists(), "ultima": int(e.get("ultima_vez") or 0),
        }

    def _recargar(self) -> None:
        datos = biblioteca.cargar()
        datos.sort(key=lambda e: (-(e.get("ultima_vez") or 0), -(e.get("agregado") or 0)))
        self._programas = [self._fila(e) for e in datos]
        self.programasCambio.emit()

    def _revisar(self) -> None:
        """¿Qué sigue abierto? Suma el tiempo de lo que se cerró y mira si terminó un instalador."""
        datos = biblioteca.cargar()
        abiertos = correr.prefijos_abiertos()
        ahora = {e["id"] for e in datos if os.path.normpath(e["prefijo"]) in abiertos}
        for pid in ahora - self._abiertos:
            self._inicios.setdefault(pid, time.time())
        for pid in self._abiertos - ahora:
            inicio = self._inicios.pop(pid, None)
            if inicio is not None:
                biblioteca.sumar_tiempo(pid, time.time() - inicio, inicio)
        cambio = ahora != self._abiertos
        self._abiertos = ahora
        if self._instalando and os.path.normpath(self._instalando["prefijo"]) not in abiertos \
                and self._instalando["proceso"].poll() is not None:
            self._fin_instalador()
        if cambio:
            self._recargar()

    programas = Property("QVariantList", lambda s: s._programas, notify=programasCambio)
    faltaMotor = Property("QStringList", lambda s: s._falta, notify=motorCambio)
    instalandoMotor = Property(bool, lambda s: s._instalando_motor, notify=motorCambio)
    instalando = Property(str, lambda s: s._instalando["nombre"] if s._instalando else "", notify=instalandoCambio)

    @Property("QVariantList", notify=motorCambio)
    def protones(self):
        return [{"id": "", "nombre": f"El de serie ({self._protones[0]['nombre']})"}] + \
               [{"id": p["id"], "nombre": p["nombre"] + (f" · {p['version']}" if p["version"] and p["version"] != p["id"] else "")}
                for p in self._protones]

    @Property(str, constant=True)
    def carpetaDatos(self):
        return str(rutas.DATOS)

    # ---- agregar ----------------------------------------------------------------------------

    def _listo_motor(self) -> bool:
        if self._falta:
            self.aviso.emit("Primero instala el motor (umu y Proton CachyOS): botón «Instalar» arriba.", "error")
            return False
        return True

    @Slot(str, result=str)
    def agregar(self, url: str) -> str:
        try:
            e = biblioteca.agregar(_ruta(url))
        except biblioteca.ErrorBiblioteca as err:
            self.aviso.emit(str(err), "error")
            return ""
        self._recargar()
        self.aviso.emit(f"Agregué {e['nombre']}.", "exito")
        return e["id"]

    @Slot(str)
    def instalar(self, url: str) -> None:
        """Corre un instalador (setup.exe, .msi) en un prefijo nuevo; al terminar, pregunta cuál es el programa."""
        ruta = _ruta(url)
        if not self._listo_motor():
            return
        if self._instalando:
            self.aviso.emit(f"Espera a que termine de instalarse {self._instalando['nombre']}.", "info")
            return
        if not Path(ruta).is_file():
            self.aviso.emit("No encuentro ese instalador.", "error")
            return
        nombre = Path(ruta).stem
        prefijo = biblioteca.nuevo_prefijo(nombre)
        antes = biblioteca.exes_en(prefijo)
        temporal = {"id": prefijo.name, "prefijo": str(prefijo), "exe": ruta, "argumentos": ""}
        try:
            proceso = correr.lanzar(temporal)
        except (OSError, FileNotFoundError) as err:
            self.aviso.emit(f"No se pudo abrir el instalador: {err}", "error")
            return
        self._instalando = {"nombre": nombre, "prefijo": str(prefijo), "antes": antes, "proceso": proceso}
        self.instalandoCambio.emit()
        self.aviso.emit("Sigue los pasos del instalador. La primera vez tarda más: se prepara Windows.", "info")

    def _fin_instalador(self) -> None:
        datos = self._instalando
        self._instalando = None
        self.instalandoCambio.emit()
        nuevos = biblioteca.exes_nuevos(datos["prefijo"], datos["antes"])
        if not nuevos:
            self.aviso.emit("El instalador terminó, pero no encontré ningún programa nuevo. "
                            "Si sí se instaló, agrégalo con «Agregar .exe» desde su carpeta.", "info")
            self._prefijo_pendiente = datos["prefijo"]
            return
        self._prefijo_pendiente = datos["prefijo"]
        self.elegirInstalado.emit([{"exe": n["exe"], "nombre": n["nombre"],
                                    "carpeta": str(Path(n["exe"]).parent.name)} for n in nuevos[:8]])

    @Slot(str, str)
    def agregarInstalado(self, exe: str, nombre: str) -> None:
        prefijo = getattr(self, "_prefijo_pendiente", None)
        try:
            e = biblioteca.agregar(exe, nombre=nombre or None, prefijo=prefijo)
        except biblioteca.ErrorBiblioteca as err:
            self.aviso.emit(str(err), "error")
            return
        self._recargar()
        self.aviso.emit(f"{e['nombre']} quedó en tu biblioteca.", "exito")

    @Slot(str)
    def abrirArchivo(self, ruta: str) -> None:
        """Un .exe que abriste con doble clic: si ya está, se abre; si no, se pregunta qué hacer."""
        ruta = _ruta(ruta)
        e = biblioteca.por_exe(ruta)
        if e:
            self.jugar(e["id"])
            return
        if not Path(ruta).is_file():
            self.aviso.emit("No encuentro ese archivo.", "error")
            return
        from . import pe
        self.preguntarArchivo.emit(ruta, pe.nombre(ruta), biblioteca.parece_instalador(ruta))

    # ---- jugar ------------------------------------------------------------------------------

    @Slot(str)
    def jugar(self, pid: str) -> None:
        e = biblioteca.por_id(pid)
        if e is None or not self._listo_motor():
            return
        if not Path(e["exe"]).exists():
            self.aviso.emit(f"Ya no encuentro el .exe de {e['nombre']}: cámbialo en su configuración.", "error")
            return
        if correr.corriendo(e):
            self.aviso.emit(f"{e['nombre']} ya está abierto.", "info")
            return
        try:
            correr.lanzar(e)
        except (OSError, FileNotFoundError) as err:
            self.aviso.emit(f"No se pudo abrir {e['nombre']}: {err}", "error")
            return
        self._inicios[pid] = time.time()
        self._abiertos.add(pid)
        self._recargar()
        primera = not (Path(e["prefijo"]) / "drive_c").exists()
        self.aviso.emit(f"Abriendo {e['nombre']}…" + (" La primera vez tarda más." if primera else ""), "info")

    @Slot(str)
    def detener(self, pid: str) -> None:
        e = biblioteca.por_id(pid)
        if e is None:
            return
        threading.Thread(target=correr.detener, args=(e,), daemon=True).start()
        self.aviso.emit(f"Cerrando {e['nombre']}…", "info")

    # ---- configurar -------------------------------------------------------------------------

    @Slot(str, str, str, str)
    def guardar(self, pid: str, nombre: str, proton: str, argumentos: str) -> None:
        try:
            biblioteca.cambiar(pid, nombre=nombre, proton=proton, argumentos=argumentos)
        except biblioteca.ErrorBiblioteca as err:
            self.aviso.emit(str(err), "error")
            return
        self._recargar()
        self.aviso.emit("Listo, guardado.", "exito")

    @Slot(str, str)
    def cambiarExe(self, pid: str, url: str) -> None:
        ruta = _ruta(url)
        if not Path(ruta).is_file():
            self.aviso.emit("No encuentro ese archivo.", "error")
            return
        biblioteca.cambiar(pid, exe=str(Path(ruta).resolve()))
        self._recargar()

    @Slot(str, bool)
    def lanzador(self, pid: str, poner: bool) -> None:
        e = biblioteca.por_id(pid)
        if e is None:
            return
        if poner:
            biblioteca.crear_lanzador(e)
            self.aviso.emit(f"{e['nombre']} ya sale en el lanzador de apps.", "exito")
        else:
            biblioteca.quitar_lanzador(e)
        self._recargar()

    def _abrir_carpeta(self, ruta: Path) -> None:
        ruta.mkdir(parents=True, exist_ok=True)
        subprocess.Popen(["xdg-open", str(ruta)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                         start_new_session=True)

    @Slot(str)
    def abrirCarpeta(self, pid: str) -> None:
        e = biblioteca.por_id(pid)
        if e:
            self._abrir_carpeta(Path(e["exe"]).parent)

    @Slot(str)
    def abrirDiscoC(self, pid: str) -> None:
        e = biblioteca.por_id(pid)
        if e:
            self._abrir_carpeta(Path(e["prefijo"]) / "drive_c")

    @Slot(str)
    def configuracionWine(self, pid: str) -> None:
        e = biblioteca.por_id(pid)
        if e is None or not self._listo_motor():
            return
        try:
            correr.lanzar(e, exe="winecfg")
        except (OSError, FileNotFoundError) as err:
            self.aviso.emit(f"No se pudo abrir la configuración de Wine: {err}", "error")

    @Slot(str, bool)
    def quitar(self, pid: str, borrarPrefijo: bool) -> None:
        e = biblioteca.por_id(pid)
        if e is None:
            return
        if correr.corriendo(e):
            correr.detener(e)
        biblioteca.quitar(pid, borrar_prefijo=borrarPrefijo)
        self._recargar()
        self.aviso.emit(f"Quité {e['nombre']} de tu biblioteca." + ("" if borrarPrefijo else " Sus partidas se quedaron."), "info")

    # ---- motor ------------------------------------------------------------------------------

    @Slot()
    def instalarMotor(self) -> None:
        if self._instalando_motor:
            return
        self._instalando_motor = True
        self.motorCambio.emit()

        def trabajo():
            try:
                r = correr.instalar_motor()
                self._motorListo.emit(r.returncode, (r.stderr or "").strip().splitlines()[-1] if r.stderr else "")
            except Exception as err:   # pkexec no está, se cortó...
                self._motorListo.emit(1, str(err))

        threading.Thread(target=trabajo, daemon=True).start()

    @Slot(int, str)
    def _fin_motor(self, codigo: int, detalle: str) -> None:
        self._instalando_motor = False
        self._falta = correr.falta_motor()
        self._protones = protones.lista()
        self.motorCambio.emit()
        if not self._falta:
            self.aviso.emit("Listo: ya puedes abrir programas de Windows.", "exito")
        elif codigo in (126, 127):
            self.aviso.emit("No se instaló: cancelaste la contraseña.", "info")
        else:
            self.aviso.emit("No se pudo instalar el motor" + (f": {detalle}" if detalle else "."), "error")

    @Slot()
    def refrescar(self) -> None:
        self._protones = protones.lista()
        self._falta = correr.falta_motor()
        self.motorCambio.emit()
        self._recargar()

    def cerrar(self) -> None:
        self._reloj.stop()
