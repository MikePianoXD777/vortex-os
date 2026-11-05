"""El puente entre la ventana (QML) y el motor. Lo lento corre en hilos y avisa con señales."""
from __future__ import annotations

import getpass
import json
import os
import threading
import time
from pathlib import Path

from PySide6.QtCore import Property, QObject, QUrl, Signal, Slot
from PySide6.QtGui import QDesktopServices, QGuiApplication

from . import motor

MAX_RECIENTES = 5


def _ruta_recientes() -> Path:
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config") / "vortex" / "shared.json"


def leer_recientes() -> list[dict]:
    try:
        datos = json.loads(_ruta_recientes().read_text())["recientes"]
    except (OSError, ValueError, KeyError, TypeError):
        return []
    if not isinstance(datos, list):
        return []
    return [r for r in datos if isinstance(r, dict) and isinstance(r.get("host"), str) and isinstance(r.get("usuario"), str)][:MAX_RECIENTES]


class Puente(QObject):
    mostrarVentana = Signal()
    cambio = Signal()
    archivosCambio = Signal()
    redCambio = Signal()
    _desdeHilo = Signal(str, float, bool, str)   # texto, progreso, ocupado, tono
    _redLista = Signal(object, object, bool)
    _codigoListo = Signal(str)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._archivos: list[dict] = []
        self._estado, self._progreso, self._ocupado, self._tono = "", -1.0, False, ""
        self._codigo = ""
        self._direcciones: list[dict] = []
        self._compus: list[dict] = []
        self._puerto_cerrado = False
        self._recientes = leer_recientes()
        self._envio: motor.Envio | None = None
        self._cancelar = threading.Event()
        self._red_cuando = -1000.0
        self._desdeHilo.connect(self._poner)
        self._redLista.connect(self._poner_red)
        self._codigoListo.connect(self._poner_codigo)
        self.actualizarRed()

    # --- Lo que muestra ---
    usuario = Property(str, lambda s: getpass.getuser(), constant=True)
    estado = Property(str, lambda s: s._estado, notify=cambio)
    progreso = Property(float, lambda s: s._progreso, notify=cambio)
    ocupado = Property(bool, lambda s: s._ocupado, notify=cambio)
    tono = Property(str, lambda s: s._tono, notify=cambio)
    codigo = Property(str, lambda s: s._codigo, notify=cambio)
    archivos = Property("QVariantList", lambda s: s._archivos, notify=archivosCambio)
    total = Property(str, lambda s: motor.humano(sum(a["tam"] for a in s._archivos)), notify=archivosCambio)
    direcciones = Property("QVariantList", lambda s: s._direcciones, notify=redCambio)
    compus = Property("QVariantList", lambda s: s._compus, notify=redCambio)
    puertoCerrado = Property(bool, lambda s: s._puerto_cerrado, notify=redCambio)
    recientes = Property("QVariantList", lambda s: s._recientes, notify=redCambio)

    @Slot(str, float, bool, str)
    def _poner(self, texto: str, progreso: float, ocupado: bool, tono: str) -> None:
        self._estado, self._progreso, self._ocupado, self._tono = texto, progreso, ocupado, tono
        self.cambio.emit()

    @Slot(str)
    def _poner_codigo(self, codigo: str) -> None:
        self._codigo = codigo
        self.cambio.emit()

    @Slot(object, object, bool)
    def _poner_red(self, direcciones, compus, cerrado: bool) -> None:
        self._direcciones, self._compus, self._puerto_cerrado = direcciones, compus, cerrado
        self.redCambio.emit()

    @Slot()
    def actualizarRed(self, forzar: bool = False) -> None:
        # Se llama al volver a la ventana: tailscale no se pregunta más de una vez cada 30 s.
        ahora = time.monotonic()
        if not forzar and ahora - self._red_cuando < 30:
            return
        self._red_cuando = ahora

        def trabajo() -> None:
            estado = motor.tailscale_estado()
            self._redLista.emit(motor.mis_direcciones(estado), motor.compus_tailscale(estado), motor.puerto_cerrado())
        threading.Thread(target=trabajo, daemon=True).start()

    # --- Los archivos ---
    def agregarRutas(self, rutas: list[str]) -> None:
        for ruta in rutas:
            if isinstance(ruta, str) and os.path.exists(ruta) and all(a["ruta"] != ruta for a in self._archivos):
                try:
                    tam = motor.tamano(Path(ruta))
                except OSError:
                    continue
                self._archivos.append({"ruta": ruta, "nombre": os.path.basename(ruta.rstrip("/")) or ruta, "tam": tam,
                                       "texto": motor.humano(tam), "carpeta": os.path.isdir(ruta)})
        self.archivosCambio.emit()

    @Slot("QVariantList")
    def agregar(self, urls: list) -> None:
        self.agregarRutas([QUrl(str(u)).toLocalFile() for u in urls])

    @Slot(int)
    def quitar(self, indice: int) -> None:
        if 0 <= indice < len(self._archivos) and not self._ocupado:
            del self._archivos[indice]
            self.archivosCambio.emit()

    # --- Mandar ---
    def _rutas(self) -> list[str]:
        return [a["ruta"] for a in self._archivos]

    def _empezar(self, texto: str) -> None:
        self._cancelar.clear()
        self._codigo = ""
        self._poner(texto, -1.0, True, "")

    @Slot(str, str)
    def mandar(self, host: str, usuario: str) -> None:
        if self._ocupado:
            return
        if not self._archivos:
            self._poner("Elige qué mandar.", -1.0, False, "error")
            return
        self._empezar("Preparando…")
        rutas, total = self._rutas(), sum(a["tam"] for a in self._archivos)
        host, usuario = host.strip(), usuario.strip()

        def al_estado(fase: str, n: int) -> None:
            if fase == "esperando":
                self._desdeHilo.emit("Esperando a que acepten en la otra compu…", -1.0, True, "")
            else:
                texto = f"Mandando: {motor.humano(n)} de {motor.humano(total)}"
                self._desdeHilo.emit(texto, min(1.0, n / total) if total else -1.0, True, "")

        def trabajo() -> None:
            envio = None
            try:
                envio = motor.Envio(rutas)
                self._envio = envio
                envio.preparar()
                motor.mandar(host, usuario, envio, al_estado, self._cancelar.is_set)
                self._recordar(host, usuario)
                self._desdeHilo.emit("Listo: ya llegó.", 1.0, False, "exito")
            except motor.ErrorShared as e:
                self._desdeHilo.emit("" if self._cancelar.is_set() else str(e), -1.0, False, "error")
            finally:
                self._envio = None
                if envio:
                    envio.terminar()

        threading.Thread(target=trabajo, daemon=True).start()

    def _recordar(self, host: str, usuario: str) -> None:
        recientes = [{"host": host, "usuario": usuario}] + [r for r in self._recientes if r["host"] != host]
        self._recientes = recientes[:MAX_RECIENTES]
        try:
            ruta = _ruta_recientes()
            ruta.parent.mkdir(parents=True, exist_ok=True)
            ruta.write_text(json.dumps({"recientes": self._recientes}))
        except OSError:
            pass
        self.redCambio.emit()

    @Slot()
    def crearCodigo(self) -> None:
        if self._ocupado:
            return
        if not self._archivos:
            self._poner("Elige qué mandar.", -1.0, False, "error")
            return
        self._empezar("Preparando el código…")
        rutas = self._rutas()

        def trabajo() -> None:
            try:
                envio = motor.Envio(rutas)
                self._envio = envio
                codigo = envio.preparar()
                if self._cancelar.is_set():
                    envio.terminar()
                    self._envio = None
                    return
                self._codigoListo.emit(codigo)
                self._desdeHilo.emit("Pégalo en la otra compu. Deja esta ventana abierta hasta que lo reciban.", -1.0, True, "")
            except motor.ErrorShared as e:
                self._envio = None
                self._desdeHilo.emit(str(e), -1.0, False, "error")

        threading.Thread(target=trabajo, daemon=True).start()

    @Slot()
    def cancelar(self) -> None:
        """Corta lo que esté pasando: un envío por aviso se corta solo en su hilo; un código, aquí."""
        self._cancelar.set()
        if self._codigo and self._envio:
            envio, self._envio = self._envio, None
            threading.Thread(target=envio.terminar, daemon=True).start()
        self._codigo = ""
        self._poner("", -1.0, False, "")

    # --- Recibir con código ---
    @Slot(str)
    def recibirCodigo(self, codigo: str) -> None:
        if self._ocupado:
            return
        self._empezar("Recibiendo…")

        def avance(n: int) -> None:
            self._desdeHilo.emit(f"Recibiendo: {motor.humano(n)}", -1.0, True, "")

        def trabajo() -> None:
            try:
                quedaron = motor.recibir(codigo, avance, self._cancelar.is_set)
                texto = quedaron[0].name if len(quedaron) == 1 else f"{quedaron[0].name} y {len(quedaron) - 1} más"
                self._desdeHilo.emit(f"Ya llegó: {texto}", 1.0, False, "exito")
            except motor.ErrorShared as e:
                self._desdeHilo.emit("" if self._cancelar.is_set() else str(e), -1.0, False, "error")

        threading.Thread(target=trabajo, daemon=True).start()

    @Slot()
    def abrirPuerto(self) -> None:
        def trabajo() -> None:
            motor.abrir_puerto()
            self.actualizarRed(True)
        threading.Thread(target=trabajo, daemon=True).start()

    @Slot(str)
    def copiar(self, texto: str) -> None:
        QGuiApplication.clipboard().setText(texto)

    @Slot()
    def abrirCarpeta(self) -> None:
        carpeta = motor.carpeta_recibidos()
        try:
            carpeta.mkdir(parents=True, exist_ok=True)
        except OSError:
            return
        QDesktopServices.openUrl(QUrl.fromLocalFile(str(carpeta)))

    def cerrar(self) -> None:
        self._cancelar.set()
        if self._envio:
            self._envio.terminar()
