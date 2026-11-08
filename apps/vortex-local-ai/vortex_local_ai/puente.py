"""Puente entre el motor de Vortexine's Local AI (ollama.py, catalogo.py, vault.py, memoria.py,
conversaciones.py) y la interfaz (QML).

Son cinco objetos que QML ve con estos nombres:
- `ia` (General): el estado de Ollama, la gráfica, la carpeta del vault y los avisos.
- `chat` (Chat): las conversaciones y los mensajes, que llegan mientras el modelo escribe.
- `modelos` (Modelos): el catálogo de Vortexine con «te cabe / va justo / no te cabe», bajar y borrar.
- `boveda` (Boveda): el vault de notas (árbol, editor, enlaces, grafo y la memoria para la IA).
- `colores` (colores.py): la paleta del fondo de pantalla.

Todo lo que habla con Ollama o lee el vault entero corre en un hilo aparte (threading); los resultados
vuelven al hilo de Qt con una señal (`_Hilos`). Los trozos del chat se juntan y se pintan cada 60 ms,
para que la ventana no se trabe aunque el modelo escriba muy rápido.
"""
from __future__ import annotations

import json
import re
import threading
import time
import unicodedata
import urllib.parse
from pathlib import Path
from typing import Any, Callable

from PySide6.QtCore import Property, QAbstractListModel, QByteArray, QModelIndex, QObject, Qt, QTimer, QUrl, Signal, Slot
from PySide6.QtGui import QGuiApplication

from . import catalogo, conversaciones, rutas
from .ollama import Cancelado, ErrorOllama, Ollama

_BASE_ROL = int(Qt.ItemDataRole.UserRole) + 1
_ENLACE = re.compile(r"(!?)\[\[([^\]\n]+?)\]\]")


# --- utilidades ---

def sin_acentos(texto: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFD", texto or "") if unicodedata.category(c) != "Mn").lower()


def tamano(n: float | int | None) -> str:
    """Bytes en texto corto: «4.7 GB», «820 MB»."""
    n = float(n or 0)
    for unidad in ("B", "KB", "MB", "GB", "TB"):
        if n < 1024 or unidad == "TB":
            return f"{n:.0f} {unidad}" if unidad in ("B", "KB", "MB") else f"{n:.1f} {unidad}"
        n /= 1024
    return f"{n:.1f} TB"


_MESES = ("ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic")


def fecha_corta(epoch: float | None) -> str:
    if not epoch:
        return ""
    t, hoy = time.localtime(epoch), time.localtime()
    dias = int(time.mktime(time.struct_time((hoy.tm_year, hoy.tm_mon, hoy.tm_mday, 0, 0, 0, 0, 0, -1))) // 86400
               - time.mktime(time.struct_time((t.tm_year, t.tm_mon, t.tm_mday, 0, 0, 0, 0, 0, -1))) // 86400)
    if dias <= 0:
        return time.strftime("%H:%M", t)
    if dias == 1:
        return "Ayer"
    if dias < 7:
        return ("lun", "mar", "mié", "jue", "vie", "sáb", "dom")[t.tm_wday]
    texto = f"{t.tm_mday} {_MESES[t.tm_mon - 1]}"
    return texto if t.tm_year == hoy.tm_year else f"{texto} {t.tm_year}"


def markdown_con_enlaces(texto: str) -> str:
    """Markdown listo para Text.MarkdownText: sin frontmatter y con [[Nota|alias]] → [alias](nota:Nota)."""
    texto = texto or ""
    if texto.startswith("---\n"):
        fin = texto.find("\n---", 4)
        if fin != -1:
            texto = texto[fin + 4:].lstrip("\n")

    def cambiar(m: re.Match) -> str:
        incrustado, cuerpo = m.group(1), m.group(2)
        destino, _, alias = cuerpo.partition("|")
        visible = alias.strip() or destino.strip()
        return f"{'📎 ' if incrustado else ''}[{visible}](nota:{urllib.parse.quote(destino.strip())})"
    # Fuera de los bloques de código: ahí [[x]] es texto
    partes = re.split(r"(```.*?```|`[^`\n]*`)", texto, flags=re.S)
    return "".join(p if i % 2 else _ENLACE.sub(cambiar, p) for i, p in enumerate(partes))


def _local(url: str) -> str:
    q = QUrl(url)
    return q.toLocalFile() if q.isLocalFile() else url


# --- piezas comunes ---

class _Hilos(QObject):
    """Corre funciones en hilos y entrega el resultado (o el error) en el hilo de Qt."""
    _listo = Signal(object, object)

    def __init__(self, parent: QObject | None = None):
        super().__init__(parent)
        self._listo.connect(self._alListo)

    def correr(self, trabajo: Callable[[], Any], al_terminar: Callable[[Any], None] | None = None,
               al_fallar: Callable[[Exception], None] | None = None) -> threading.Thread:
        def hilo() -> None:
            try:
                resultado = trabajo()
            except Exception as e:   # noqa: BLE001 — se entrega a quien lo pidió
                self._listo.emit(al_fallar, e)
            else:
                self._listo.emit(al_terminar, resultado)
        t = threading.Thread(target=hilo, daemon=True)
        t.start()
        return t

    def en_qt(self, f: Callable[[], None]) -> None:
        """Llama a `f` en el hilo de Qt (desde cualquier hilo)."""
        self._listo.emit(lambda _: f(), None)

    @Slot(object, object)
    def _alListo(self, f: Callable | None, valor: Any) -> None:
        if f is not None:
            f(valor)


class ModeloLista(QAbstractListModel):
    """Lista de dicts para QML (cada clave es un rol). `cambiar` avisa solo de los roles que cambian,
    así una barra de progreso o el mensaje que se escribe no redibujan toda la lista."""
    cuentaCambio = Signal()

    def __init__(self, roles: dict[str, Any], parent: QObject | None = None):
        super().__init__(parent)
        self._roles = list(roles)
        self._por_defecto = dict(roles)
        self._filas: list[dict] = []

    def _completa(self, fila: dict) -> dict:
        return {k: (fila.get(k) if fila.get(k) is not None else self._por_defecto[k]) for k in self._roles}

    def roleNames(self) -> dict[int, QByteArray]:
        return {_BASE_ROL + i: QByteArray(r.encode()) for i, r in enumerate(self._roles)}

    def rowCount(self, parent: QModelIndex = QModelIndex()) -> int:   # noqa: B008
        return 0 if parent.isValid() else len(self._filas)

    def data(self, indice: QModelIndex, rol: int = Qt.ItemDataRole.DisplayRole) -> Any:
        if not indice.isValid() or not 0 <= indice.row() < len(self._filas):
            return None
        i = int(rol) - _BASE_ROL
        return self._filas[indice.row()][self._roles[i]] if 0 <= i < len(self._roles) else None

    def poner(self, filas: list[dict]) -> None:
        self.beginResetModel()
        self._filas = [self._completa(f) for f in filas]
        self.endResetModel()
        self.cuentaCambio.emit()

    def agregar(self, fila: dict) -> None:
        n = len(self._filas)
        self.beginInsertRows(QModelIndex(), n, n)
        self._filas.append(self._completa(fila))
        self.endInsertRows()
        self.cuentaCambio.emit()

    def quitar_desde(self, i: int) -> None:
        if 0 <= i < len(self._filas):
            self.beginRemoveRows(QModelIndex(), i, len(self._filas) - 1)
            del self._filas[i:]
            self.endRemoveRows()
            self.cuentaCambio.emit()

    def cambiar(self, i: int, **campos: Any) -> None:
        if not 0 <= i < len(self._filas):
            return
        fila = self._filas[i]
        roles = [_BASE_ROL + self._roles.index(k) for k, v in campos.items() if k in fila and fila[k] != v]
        if not roles:
            return
        fila.update({k: v for k, v in campos.items() if k in fila})
        indice = self.index(i, 0)
        self.dataChanged.emit(indice, indice, roles)

    def buscar(self, clave: str, valor: Any) -> int:
        return next((i for i, f in enumerate(self._filas) if f.get(clave) == valor), -1)

    def fila(self, i: int) -> dict:
        return dict(self._filas[i]) if 0 <= i < len(self._filas) else {}

    def filas(self) -> list[dict]:
        return [dict(f) for f in self._filas]

    @Slot(int, result="QVariantMap")
    def obtener(self, i: int) -> dict:
        return self.fila(i)

    count = Property(int, lambda s: len(s._filas), notify=cuentaCambio)


class Config:
    """La configuración de la interfaz (rutas.config(): vault, modelo, modelo de embeddings…)."""

    def __init__(self) -> None:
        self.ruta = rutas.config()
        try:
            datos = json.loads(self.ruta.read_text(encoding="utf-8"))
            self.datos: dict = datos if isinstance(datos, dict) else {}
        except (OSError, ValueError):
            self.datos = {}

    def get(self, clave: str, defecto: Any = None) -> Any:
        return self.datos.get(clave, defecto)

    def poner(self, clave: str, valor: Any) -> None:
        if self.datos.get(clave) == valor:
            return
        self.datos[clave] = valor
        try:
            self.ruta.parent.mkdir(parents=True, exist_ok=True)
            temporal = self.ruta.with_suffix(".tmp")
            temporal.write_text(json.dumps(self.datos, ensure_ascii=False, indent=2), encoding="utf-8")
            temporal.replace(self.ruta)
        except OSError:
            pass


def _mensaje_error(e: Exception) -> str:
    if isinstance(e, ErrorOllama):
        return str(e)
    if isinstance(e, FileExistsError):
        return "Ya hay una nota con ese nombre."
    if isinstance(e, FileNotFoundError):
        return "Esa nota ya no existe (¿la borraste o la moviste fuera de la app?)."
    if isinstance(e, PermissionError):
        return "No tengo permiso para escribir ahí."
    if isinstance(e, ValueError) and str(e):
        return str(e)
    return f"Algo salió mal: {e}"


# --- General (ia) ---

class General(QObject):
    aviso = Signal(str, str)        # texto, tipo (info | error | exito)
    mostrarVentana = Signal()
    irA = Signal(str)
    estadoCambio = Signal()
    gpuCambio = Signal()
    configCambio = Signal()

    def __init__(self, puente: "Puente"):
        super().__init__(puente)
        self.p = puente
        self._estado = "detenido"       # corriendo | arrancando | detenido | falta
        self._mensaje = ""
        self._gpu = ""
        self._vigilando = False
        self._reloj = QTimer(self)
        self._reloj.setInterval(15000)
        self._reloj.timeout.connect(lambda: self.revisarOllama(False))

    def arrancar(self) -> None:
        self.revisarOllama(True)
        self._reloj.start()

    def _poner_estado(self, estado: str, mensaje: str = "") -> None:
        if (estado, mensaje) != (self._estado, self._mensaje):
            antes = self._estado
            self._estado, self._mensaje = estado, mensaje
            self.estadoCambio.emit()
            if estado == "corriendo" and antes != "corriendo":
                self.p.modelos.recargar()

    @Slot(bool)
    def revisarOllama(self, arrancar: bool = False) -> None:
        """Pregunta si Ollama contesta; con `arrancar`, si no contesta lo arranca (asegurar_servidor)."""
        if self._vigilando:
            return
        self._vigilando = True
        o = self.p.ollama

        def trabajo() -> tuple[str, str, bool]:
            if o.disponible():
                return "corriendo", "", False
            instalado = getattr(o, "instalado", lambda: True)()
            if not arrancar:
                return ("detenido" if instalado else "falta"), "", False
            if not instalado:
                return "falta", "", False
            self.p.hilos.en_qt(lambda: self._poner_estado("arrancando"))
            if o.asegurar_servidor():
                return "corriendo", "", True
            return "detenido", getattr(o, "ultimo_error", "") or "", False

        def listo(r: tuple[str, str, bool]) -> None:
            self._vigilando = False
            estado, error, arranco = r
            self._poner_estado(estado, error)
            if arranco:
                self.aviso.emit("Arranqué Ollama, el motor que corre tus modelos. Ya puedes usarlos.", "exito")
            elif arrancar and estado == "falta":
                self.aviso.emit("Falta Ollama, el motor que corre los modelos. Instálalo con «vortex actualizar».", "error")
            elif arrancar and estado == "detenido":
                self.aviso.emit("No pude arrancar Ollama" + (f": {error}" if error else ".")
                                + " Prueba con «Arrancar» en un momento.", "error")

        def fallo(e: Exception) -> None:
            self._vigilando = False
            self._poner_estado("detenido", _mensaje_error(e))

        self.p.hilos.correr(trabajo, listo, fallo)

    @Slot()
    def arrancarOllama(self) -> None:
        self.revisarOllama(True)

    def poner_gpu(self, texto: str) -> None:
        if texto != self._gpu:
            self._gpu = texto
            self.gpuCambio.emit()

    # --- ajustes ---
    @Slot(str)
    def abrirCarpetaVault(self, url: str) -> None:
        ruta = _local(url)
        if ruta:
            self.p.boveda.usar(Path(ruta))
            self.configCambio.emit()

    @Slot()
    def vaultPorDefecto(self) -> None:
        self.p.boveda.usar(Path(rutas.VAULT_POR_DEFECTO))
        self.configCambio.emit()

    def _modelo_memoria(self) -> str:
        return self.p.config.get("modelo_embeddings", "") or ""

    def _poner_modelo_memoria(self, nombre: str) -> None:
        if nombre != self._modelo_memoria():
            self.p.config.poner("modelo_embeddings", nombre)
            self.configCambio.emit()

    @Slot(str)
    def copiar(self, texto: str) -> None:
        QGuiApplication.clipboard().setText(texto)
        self.aviso.emit("Copiado.", "exito")

    estadoOllama = Property(str, lambda s: s._estado, notify=estadoCambio)
    mensajeOllama = Property(str, lambda s: s._mensaje, notify=estadoCambio)
    urlOllama = Property(str, lambda s: str(getattr(s.p.ollama, "url", "")), constant=True)
    gpu = Property(str, lambda s: s._gpu, notify=gpuCambio)
    carpetaVault = Property(str, lambda s: str(s.p.boveda.raiz), notify=configCambio)
    modeloMemoria = Property(str, _modelo_memoria, _poner_modelo_memoria, notify=configCambio)
    embeddingsRecomendado = Property(str, lambda s: getattr(catalogo, "EMBEDDINGS", ""), constant=True)
    version = Property(str, lambda s: s.p.version, constant=True)


# --- Modelos ---

def _para_lista(para: Any) -> list[str]:
    if isinstance(para, str):
        return [para]
    return [str(x) for x in para or []]


class Modelos(QObject):
    instaladosCambio = Signal()
    filtroCambio = Signal()
    gpuCambio = Signal()
    _progreso = Signal(str, float, str)

    ROLES_CATALOGO = {"ident": "", "nombre": "", "descripcion": "", "para": "", "tipo": "", "gb": 0.0, "vram_gb": 0.0,
                      "tamano": "", "cabe": "si", "instalado": False, "bajando": False, "progreso": -1.0,
                      "estado": "", "recomendado": False, "contexto": "", "piensa": False}
    ROLES_INSTALADOS = {"nombre": "", "bonito": "", "bytes": 0.0, "tamano": "", "detalles": "", "embeddings": False}

    def __init__(self, puente: "Puente"):
        super().__init__(puente)
        self.p = puente
        self.catalogo = ModeloLista(self.ROLES_CATALOGO, self)
        self.instalados = ModeloLista(self.ROLES_INSTALADOS, self)
        self._filtro = "todos"
        self._vram: dict = {}
        self._gpu = ""
        self._recomendado = ""
        self._nombres: list[str] = []
        self._bajando: dict[str, dict] = {}       # id -> {"cancelar": Event, "progreso": float, "estado": str}
        self._cargando = False
        self._progreso.connect(self._alProgreso)

    def arrancar(self, vram: dict | None = None) -> None:
        if vram is not None:
            self._poner_vram(vram)
        else:
            self.p.hilos.correr(catalogo.vram_disponible, self._poner_vram, lambda e: self._poner_vram({}))
        self.recargar()

    def _poner_vram(self, vram: dict | None) -> None:
        self._vram = vram or {}
        try:
            self._recomendado = (catalogo.recomendado(self._vram) or {}).get("id", "") if self._vram else ""
        except Exception:   # noqa: BLE001
            self._recomendado = ""
        nombre = re.sub(r"^(NVIDIA |AMD |Intel\(R\) )?(GeForce |Radeon )?", "", str(self._vram.get("nombre") or "")).strip()
        gb = (self._vram.get("total_mb") or 0) / 1024
        if not self._vram:
            texto = ""
        elif self._vram.get("de_entrada"):
            texto = f"Tu gráfica se queda corta · usarás el procesador y tu RAM ({gb:.0f} GB)"
        elif self._vram.get("tipo") == "cpu":
            texto = f"Sin gráfica dedicada · usarás tu RAM ({gb:.0f} GB)"
        else:
            texto = f"Tu gráfica: {nombre or 'sin nombre'} · {gb:.0f} GB"
        self._gpu = texto
        self.gpuCambio.emit()
        self.p.general.poner_gpu(texto.replace("Tu gráfica: ", ""))
        self._llenar_catalogo()

    def _instalado(self, id_: str) -> bool:
        return id_ in self._nombres or (":" not in id_ and f"{id_}:latest" in self._nombres)

    def _llenar_catalogo(self) -> None:
        tipos = getattr(catalogo, "TIPOS", {})
        filas = []
        for m in catalogo.CATALOGO:
            para = _para_lista(m.get("para"))
            if self._filtro != "todos" and self._filtro not in para:
                continue
            try:
                cabe = catalogo.cabe(m, self._vram) if self._vram else "si"
            except Exception:   # noqa: BLE001
                cabe = "si"
            b = self._bajando.get(m["id"])
            filas.append({**{k: m.get(k) for k in ("nombre", "descripcion", "gb", "vram_gb", "piensa")}, "ident": m["id"],
                          "contexto": str(m.get("contexto") or ""),
                          "para": ", ".join(para), "tipo": " · ".join(tipos.get(x, x.capitalize()) for x in para),
                          "tamano": f"{float(m.get('gb') or 0):.1f} GB", "cabe": cabe,
                          "instalado": self._instalado(m["id"]), "recomendado": m["id"] == self._recomendado,
                          "bajando": b is not None, "progreso": b["progreso"] if b else -1.0,
                          "estado": b["estado"] if b else ""})
        orden = {"si": 0, "justo": 1, "no": 2}
        filas.sort(key=lambda f: (not f["recomendado"], orden.get(f["cabe"], 3)))
        self.catalogo.poner(filas)

    @Slot()
    def recargar(self) -> None:
        if self._cargando:
            return
        self._cargando = True

        def listo(modelos: list[dict]) -> None:
            self._cargando = False
            filas = []
            for m in sorted(modelos, key=lambda x: str(x.get("nombre", ""))):
                del_catalogo = next((c for c in catalogo.CATALOGO if c["id"] == m.get("nombre")
                                     or f"{c['id']}:latest" == m.get("nombre")), None)
                detalles = " · ".join(str(x) for x in (m.get("familia"), m.get("parametros"), m.get("cuantizacion")) if x)
                filas.append({"nombre": m.get("nombre", ""), "bonito": (del_catalogo or {}).get("nombre", ""),
                              "bytes": float(m.get("bytes") or 0), "tamano": tamano(m.get("bytes")),
                              "detalles": detalles, "embeddings": bool(m.get("embeddings"))})
            self.instalados.poner(filas)
            self._nombres = [f["nombre"] for f in filas]
            self._llenar_catalogo()
            self.instaladosCambio.emit()

        def fallo(e: Exception) -> None:
            self._cargando = False
            if self._nombres:
                self.instalados.poner([])
                self._nombres = []
                self._llenar_catalogo()
                self.instaladosCambio.emit()

        self.p.hilos.correr(self.p.ollama.modelos, listo, fallo)

    @Slot(str)
    def bajar(self, id_: str) -> None:
        if id_ in self._bajando:
            return
        cancelar = threading.Event()
        self._bajando[id_] = {"cancelar": cancelar, "progreso": -1.0, "estado": "Empezando…"}
        self._marcar(id_)
        ultimo = [0.0, -1.0]   # cuándo y con qué fracción se avisó la última vez

        def al_progreso(completado: int, total: int, estado: str) -> None:
            fraccion = completado / total if total else -1.0
            ahora = time.monotonic()
            if ahora - ultimo[0] >= 0.15 or abs(fraccion - ultimo[1]) >= 0.01 or estado == "Listo":
                ultimo[0], ultimo[1] = ahora, fraccion
                self._progreso.emit(id_, float(fraccion), estado or "")

        def trabajo() -> None:
            if not self.p.ollama.disponible():
                if not self.p.ollama.asegurar_servidor():
                    raise ErrorOllama("Ollama no está corriendo y no lo pude arrancar. Ve a Ajustes y toca «Arrancar».")
            self.p.ollama.bajar(id_, al_progreso, cancelar)

        def listo(_: Any) -> None:
            self._bajando.pop(id_, None)
            nombre = (catalogo.por_id(id_) or {}).get("nombre", id_) if hasattr(catalogo, "por_id") else id_
            self.p.general.aviso.emit(f"Listo: {nombre} ya está instalado.", "exito")
            self._marcar(id_)
            self.recargar()

        def fallo(e: Exception) -> None:
            self._bajando.pop(id_, None)
            self._marcar(id_)
            if isinstance(e, Cancelado) or getattr(e, "clave", "") == "cancelado":
                self.p.general.aviso.emit("Cancelé la descarga. Lo que ya bajó se retoma la próxima vez.", "info")
            else:
                self.p.general.aviso.emit(_mensaje_error(e), "error")

        self.p.hilos.correr(trabajo, listo, fallo)

    @Slot(str)
    def cancelar(self, id_: str) -> None:
        b = self._bajando.get(id_)
        if b:
            b["cancelar"].set()
            b["estado"] = "Cancelando…"
            self._marcar(id_)

    @Slot(str, float, str)
    def _alProgreso(self, id_: str, fraccion: float, estado: str) -> None:
        b = self._bajando.get(id_)
        if b and not b["cancelar"].is_set():
            b["progreso"], b["estado"] = fraccion, estado
            self._marcar(id_)

    def _marcar(self, id_: str) -> None:
        i = self.catalogo.buscar("ident", id_)
        b = self._bajando.get(id_)
        self.catalogo.cambiar(i, bajando=b is not None, progreso=b["progreso"] if b else -1.0,
                              estado=b["estado"] if b else "", instalado=self._instalado(id_))

    @Slot(str)
    def borrar(self, nombre: str) -> None:
        def listo(_: Any) -> None:
            self.p.general.aviso.emit(f"Borré {nombre}. Lo puedes volver a bajar cuando quieras.", "exito")
            self.recargar()
        self.p.hilos.correr(lambda: self.p.ollama.borrar(nombre), listo,
                            lambda e: self.p.general.aviso.emit(_mensaje_error(e), "error"))

    def _poner_filtro(self, f: str) -> None:
        if f != self._filtro:
            self._filtro = f
            self.filtroCambio.emit()
            self._llenar_catalogo()

    def nombres_chat(self) -> list[str]:
        return [f["nombre"] for f in self.instalados.filas() if not f["embeddings"]]

    def nombres_embeddings(self) -> list[str]:
        return [f["nombre"] for f in self.instalados.filas() if f["embeddings"]]

    def _tipos(self) -> list[dict]:
        tipos = getattr(catalogo, "TIPOS", {}) or {}
        return [{"id": "todos", "nombre": "Todos"}] + [{"id": k, "nombre": v} for k, v in tipos.items()]

    filtro = Property(str, lambda s: s._filtro, _poner_filtro, notify=filtroCambio)
    gpu = Property(str, lambda s: s._gpu, notify=gpuCambio)
    tipos = Property("QVariantList", _tipos, constant=True)
    nombresChat = Property("QStringList", nombres_chat, notify=instaladosCambio)
    nombresEmbeddings = Property("QStringList", nombres_embeddings, notify=instaladosCambio)
    hayInstalados = Property(bool, lambda s: bool(s.nombres_chat()), notify=instaladosCambio)
    lista = Property(QObject, lambda s: s.catalogo, constant=True)
    listaInstalados = Property(QObject, lambda s: s.instalados, constant=True)


# --- Chat ---

class Chat(QObject):
    actualCambio = Signal()
    modeloCambio = Signal()
    generandoCambio = Signal()
    usarVaultCambio = Signal()

    ROLES_CONV = {"clave": "", "titulo": "", "fecha": "", "vault": False}
    ROLES_MSG = {"rol": "", "contenido": "", "pensamiento": "", "citas": [], "error": "", "escribiendo": False,
                 "pensando": False, "modelo": "", "datos": ""}

    def __init__(self, puente: "Puente"):
        super().__init__(puente)
        self.p = puente
        self.conversaciones = ModeloLista(self.ROLES_CONV, self)
        self.mensajes = ModeloLista(self.ROLES_MSG, self)
        self._conv: dict = conversaciones.nueva()
        self._modelo = self.p.config.get("modelo", "") or ""
        self._usar_vault = bool(self.p.config.get("usar_vault", False))
        self._generando = False
        self._cancelar = threading.Event()
        self._candado = threading.Lock()
        self._trozos = ["", ""]           # respuesta, pensamiento: lo que llegó y aún no se pinta
        self._pintar = QTimer(self)
        self._pintar.setInterval(60)
        self._pintar.timeout.connect(self._volcar)
        self.p.modelos.instaladosCambio.connect(self._revisar_modelo)

    def arrancar(self) -> None:
        self.recargarLista()

    @Slot()
    def recargarLista(self) -> None:
        try:
            convs = conversaciones.lista()
        except Exception:   # noqa: BLE001
            convs = []
        self.conversaciones.poner([{"clave": c.get("id", ""), "titulo": c.get("titulo") or "Chat nuevo",
                                    "fecha": fecha_corta(c.get("modificado")), "vault": bool(c.get("vault"))}
                                   for c in convs])

    def _revisar_modelo(self) -> None:
        nombres = self.p.modelos.nombres_chat()
        if nombres and self._modelo not in nombres:
            recomendado = self.p.modelos._recomendado
            self._poner_modelo(next((n for n in nombres if n == recomendado or n.startswith(recomendado + ":")), nombres[0]))
        elif not nombres and self._modelo:
            self._modelo = ""            # sin modelos: el chat pide bajar uno (la preferencia guardada se queda)
            self.modeloCambio.emit()

    def _poner_modelo(self, nombre: str) -> None:
        if nombre != self._modelo:
            self._modelo = nombre
            self.p.config.poner("modelo", nombre)
            self.modeloCambio.emit()

    def _poner_usar_vault(self, v: bool) -> None:
        if v != self._usar_vault:
            self._usar_vault = v
            self.p.config.poner("usar_vault", v)
            self.usarVaultCambio.emit()

    def _poner_generando(self, v: bool) -> None:
        if v != self._generando:
            self._generando = v
            self.generandoCambio.emit()

    @staticmethod
    def _citas_de(rutas_citadas: list) -> list[dict]:
        vistas, salida = set(), []
        for r in rutas_citadas or []:
            ruta = r.get("ruta") if isinstance(r, dict) else str(r)
            if ruta and ruta not in vistas:
                vistas.add(ruta)
                titulo = r.get("titulo") if isinstance(r, dict) and r.get("titulo") else Path(ruta).stem
                salida.append({"ruta": ruta, "titulo": titulo})
        return salida

    # --- conversaciones ---
    @Slot()
    def nuevo(self) -> None:
        if self._generando:
            self.detener()
        self._conv = conversaciones.nueva(modelo=self._modelo)
        self.mensajes.poner([])
        self.actualCambio.emit()

    @Slot(str)
    def abrir(self, id_: str) -> None:
        if self._generando:
            self.detener()
        conv = conversaciones.abrir(id_)
        if not conv:
            self.p.general.aviso.emit("No encontré ese chat.", "error")
            self.recargarLista()
            return
        self._conv = conv
        filas = []
        for m in conv.get("mensajes", []):
            if m.get("role") not in ("user", "assistant"):
                continue
            filas.append({"rol": m["role"], "contenido": m.get("content", ""), "pensamiento": m.get("pensamiento", ""),
                          "citas": self._citas_de(m.get("citas")), "modelo": m.get("modelo", "")})
        self.mensajes.poner(filas)
        if conv.get("modelo") in self.p.modelos.nombres_chat():
            self._poner_modelo(conv["modelo"])
        self._poner_usar_vault(bool(conv.get("vault", self._usar_vault)))
        self.actualCambio.emit()

    @Slot(str)
    def borrar(self, id_: str) -> None:
        try:
            conversaciones.borrar(id_)
        except OSError as e:
            self.p.general.aviso.emit(_mensaje_error(e), "error")
        if id_ == self._conv.get("id"):
            self.nuevo()
        self.recargarLista()

    def _guardar(self) -> None:
        if not self._conv.get("mensajes"):
            return
        if not self._conv.get("titulo"):
            primero = next((m["content"] for m in self._conv["mensajes"] if m.get("role") == "user"), "")
            self._conv["titulo"] = conversaciones.titulo_automatico(primero)
        try:
            self._conv["id"] = conversaciones.guardar(self._conv)
        except OSError as e:
            self.p.general.aviso.emit("No pude guardar el chat: " + _mensaje_error(e), "error")
            return
        self.recargarLista()
        self.actualCambio.emit()

    # --- mandar y recibir ---
    @Slot(str)
    def enviar(self, texto: str) -> None:
        texto = (texto or "").strip()
        if not texto or self._generando:
            return
        if not self._modelo:
            self.p.general.aviso.emit("Primero baja un modelo: te llevo a Modelos.", "info")
            self.p.general.irA.emit("modelos")
            return
        historial = [{"role": m["role"], "content": m.get("content", "")} for m in self._conv.get("mensajes", [])
                     if m.get("role") in ("user", "assistant") and m.get("content")]
        self._conv.setdefault("mensajes", []).append({"role": "user", "content": texto, "fecha": time.time()})
        self._conv["modelo"] = self._modelo
        if self._usar_vault:
            self._conv["vault"] = True
        self.mensajes.agregar({"rol": "user", "contenido": texto})
        self._generar(historial, texto)

    @Slot()
    def regenerar(self) -> None:
        """Vuelve a pedir la última respuesta (con el modelo elegido ahora)."""
        if self._generando:
            return
        msgs = self._conv.get("mensajes", [])
        i_user = max((i for i, m in enumerate(msgs) if m.get("role") == "user"), default=-1)
        if i_user < 0:
            return
        pregunta = msgs[i_user]["content"]
        del msgs[i_user + 1:]
        historial = [{"role": m["role"], "content": m.get("content", "")} for m in msgs[:i_user]
                     if m.get("role") in ("user", "assistant") and m.get("content")]
        filas = self.mensajes.filas()
        ultima_user = max((i for i, f in enumerate(filas) if f["rol"] == "user"), default=-1)
        self.mensajes.quitar_desde(ultima_user + 1)
        self._generar(historial, pregunta)

    @Slot()
    def detener(self) -> None:
        self._cancelar.set()

    def _generar(self, historial: list[dict], pregunta: str) -> None:
        modelo, usar_vault = self._modelo, self._usar_vault
        self._cancelar = cancelar = threading.Event()
        with self._candado:
            self._trozos = ["", ""]
        self.mensajes.agregar({"rol": "assistant", "escribiendo": True, "modelo": modelo})
        fila = self.mensajes.count - 1
        self._poner_generando(True)
        self._pintar.start()
        p = self.p

        def al_trozo(t: str) -> None:
            with self._candado:
                self._trozos[0] += t

        def al_pensar(t: str) -> None:
            with self._candado:
                self._trozos[1] += t

        def trabajo() -> dict:
            citas: list = []
            aviso = ""
            if usar_vault:
                mem = p.memoria()
                mensajes = mem.mensajes_con_contexto(pregunta, historial)
                citas = self._citas_de(getattr(mem, "ultimo_contexto", []) or [])
                aviso = getattr(mem, "aviso", "") or ""
                p.hilos.en_qt(lambda: self.mensajes.cambiar(fila, citas=citas))
            else:
                mensajes = [*historial, {"role": "user", "content": pregunta}]
            try:
                respuesta = p.ollama.chat(modelo, mensajes, al_trozo, cancelar, None, al_pensar)
            except ErrorOllama as e:
                if getattr(e, "clave", "") != "conexion" or not p.ollama.asegurar_servidor():
                    raise
                p.hilos.en_qt(lambda: p.general.revisarOllama(False))
                respuesta = p.ollama.chat(modelo, mensajes, al_trozo, cancelar, None, al_pensar)
            return {"respuesta": respuesta, "citas": citas, "aviso": aviso,
                    "datos": getattr(p.ollama, "ultimas_estadisticas", None) or {}}

        def terminar(error: str = "", r: dict | None = None) -> None:
            self._pintar.stop()
            self._volcar()
            actual = self.mensajes.fila(fila)
            r = r or {}
            contenido = r.get("respuesta") or actual.get("contenido", "")
            datos = r.get("datos") or {}
            texto_datos = ""
            if datos.get("tokens_por_segundo"):
                texto_datos = f"{datos['tokens_por_segundo']:.0f} palabras/s"   # tokens, pero así se entiende
            if cancelar.is_set() and not error:
                texto_datos = "Detenido" + (f" · {texto_datos}" if texto_datos else "")
            self.mensajes.cambiar(fila, contenido=contenido, escribiendo=False, pensando=False, error=error,
                                  datos=texto_datos, citas=self._citas_de(r.get("citas")) or actual.get("citas", []))
            if contenido or actual.get("pensamiento"):
                self._conv.setdefault("mensajes", []).append({
                    "role": "assistant", "content": contenido, "pensamiento": self.mensajes.fila(fila).get("pensamiento", ""),
                    "citas": [c["ruta"] for c in self.mensajes.fila(fila).get("citas", [])],
                    "modelo": modelo, "fecha": time.time()})
            self._poner_generando(False)
            if r.get("aviso"):
                p.general.aviso.emit(r["aviso"], "info")
            self._guardar()

        self.p.hilos.correr(trabajo, lambda r: terminar("", r), lambda e: terminar(_mensaje_error(e)))

    def _volcar(self) -> None:
        with self._candado:
            respuesta, pensamiento = self._trozos
            self._trozos = ["", ""]
        if not respuesta and not pensamiento:
            return
        fila = self.mensajes.count - 1
        actual = self.mensajes.fila(fila)
        self.mensajes.cambiar(fila, contenido=actual.get("contenido", "") + respuesta,
                              pensamiento=actual.get("pensamiento", "") + pensamiento,
                              pensando=bool(pensamiento) and not (actual.get("contenido") or respuesta))

    @Slot()
    def guardarEnVault(self) -> None:
        if not self._conv.get("mensajes"):
            self.p.general.aviso.emit("Este chat está vacío: escribe algo primero.", "info")
            return
        if self._generando:
            self.p.general.aviso.emit("Espera a que termine de escribir.", "info")
            return
        self._guardar()
        try:
            ruta = self.p.boveda.guardar_chat(self._conv.get("titulo") or "Chat", conversaciones.a_nota(self._conv))
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit("No pude guardarlo en el vault: " + _mensaje_error(e), "error")
            return
        self.p.general.aviso.emit(f"Guardé el chat en tu vault: {ruta}", "exito")

    actual = Property(str, lambda s: s._conv.get("id", "") or "", notify=actualCambio)
    titulo = Property(str, lambda s: s._conv.get("titulo", "") or "", notify=actualCambio)
    modelo = Property(str, lambda s: s._modelo, _poner_modelo, notify=modeloCambio)
    generando = Property(bool, lambda s: s._generando, notify=generandoCambio)
    usarVault = Property(bool, lambda s: s._usar_vault, _poner_usar_vault, notify=usarVaultCambio)
    lista = Property(QObject, lambda s: s.conversaciones, constant=True)
    listaMensajes = Property(QObject, lambda s: s.mensajes, constant=True)


# --- Vault (boveda) ---

class Boveda(QObject):
    raizCambio = Signal()
    notaCambio = Signal()
    panelesCambio = Signal()
    resultadosCambio = Signal()
    grafoCambio = Signal()
    memoriaCambio = Signal()
    notaGuardada = Signal()
    _progresoMemoria = Signal(int, int)

    ROLES_ARBOL = {"tipo": "nota", "nombre": "", "ruta": "", "nivel": 0, "abierta": False, "vacia": False}

    def __init__(self, puente: "Puente"):
        super().__init__(puente)
        self.p = puente
        self.arbol = ModeloLista(self.ROLES_ARBOL, self)
        self.vault = None
        self._abiertas: set[str] = set()
        self._nota = ""
        self._texto = ""
        self._retro: list = []
        self._enlaces: list = []
        self._etiquetas_nota: list = []
        self._etiquetas: list = []
        self._resultados: list = []
        self._busqueda = ""
        self._grafo: dict = {"nodos": [], "aristas": []}
        self._titulos: list[dict] = []
        self._memoria = -1.0
        self._memoria_texto = ""
        self._cancelar_memoria = threading.Event()
        self._progresoMemoria.connect(self._alProgresoMemoria)
        self._vigilar = QTimer(self)          # por si cambias notas con Obsidian u otro editor
        self._vigilar.setInterval(5000)
        self._vigilar.timeout.connect(self._revisar_cambios)
        self._huella = None

    @property
    def raiz(self) -> Path:
        return Path(self.vault.raiz) if self.vault is not None else Path(rutas.VAULT_POR_DEFECTO)

    def arrancar(self) -> None:
        guardada = self.p.config.get("vault", "")
        self.usar(Path(guardada) if guardada else Path(rutas.VAULT_POR_DEFECTO), avisar=False)
        self._vigilar.start()

    def usar(self, ruta: Path, avisar: bool = True) -> None:
        from .vault import Vault
        try:
            self.vault = Vault(ruta)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(f"No pude abrir esa carpeta: {_mensaje_error(e)}", "error")
            if self.vault is None:
                self.vault = Vault(rutas.VAULT_POR_DEFECTO)
            return
        self.p.config.poner("vault", str(self.raiz))
        self._abiertas.clear()
        self._nota, self._texto = "", ""
        self._retro, self._enlaces, self._etiquetas_nota = [], [], []
        self._resultados, self._busqueda = [], ""
        self.notaCambio.emit()
        self.panelesCambio.emit()
        self.resultadosCambio.emit()
        self.raizCambio.emit()
        self.recargar()
        self.pedirGrafo()
        if avisar:
            self.p.general.aviso.emit(f"Ahora usas el vault «{self.raiz.name}».", "exito")

    # --- árbol ---
    @Slot()
    def recargar(self) -> None:
        if self.vault is None:
            return
        try:
            arbol = self.vault.arbol()
            notas = self.vault.notas()
            etiquetas = self.vault.etiquetas()
        except OSError as e:
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return
        self._titulos = [{"titulo": n.get("titulo", ""), "ruta": n.get("ruta", ""), "clave": sin_acentos(n.get("titulo", ""))}
                         for n in notas]
        self._etiquetas = [{"nombre": k, "n": v} for k, v in sorted(etiquetas.items(), key=lambda kv: (-kv[1], kv[0]))]
        self._arbol_crudo = arbol
        self._huella = self._calcular_huella(notas)
        self._aplanar()
        self.panelesCambio.emit()

    @staticmethod
    def _calcular_huella(notas: list[dict]) -> tuple:
        return tuple(sorted((n.get("ruta", ""), n.get("modificado", 0)) for n in notas))

    def _revisar_cambios(self) -> None:
        if self.vault is None:
            return
        try:
            huella = self._calcular_huella(self.vault.notas())
        except OSError:
            return
        if huella != self._huella:
            self.recargar()
            self.pedirGrafo()

    def _aplanar(self) -> None:
        filas: list[dict] = []

        def recorrer(nodo: dict, nivel: int) -> None:
            for h in nodo.get("hijos", []):
                if h.get("tipo") == "carpeta":
                    abierta = h["ruta"] in self._abiertas
                    filas.append({"tipo": "carpeta", "nombre": h.get("nombre", ""), "ruta": h["ruta"], "nivel": nivel,
                                  "abierta": abierta, "vacia": not h.get("hijos")})
                    if abierta:
                        recorrer(h, nivel + 1)
                else:
                    filas.append({"tipo": "nota", "nombre": h.get("titulo") or h.get("nombre", ""), "ruta": h["ruta"],
                                  "nivel": nivel})
        recorrer(getattr(self, "_arbol_crudo", {}) or {}, 0)
        self.arbol.poner(filas)

    @Slot(str)
    def alternar(self, ruta: str) -> None:
        self._abiertas.symmetric_difference_update({ruta})
        self._aplanar()

    def _abrir_padres(self, ruta: str) -> None:
        partes = ruta.split("/")[:-1]
        for i in range(1, len(partes) + 1):
            self._abiertas.add("/".join(partes[:i]))

    # --- notas ---
    @Slot(str)
    def abrir(self, ruta: str) -> None:
        try:
            texto = self.vault.leer(ruta)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return
        self._nota, self._texto = ruta, texto
        self._abrir_padres(ruta)
        self._aplanar()
        self.notaCambio.emit()
        self._paneles()

    @Slot(str)
    def abrirEnlace(self, nombre: str) -> None:
        """Toca un [[enlace]]: abre la nota; si no existe, la crea (como Obsidian)."""
        nombre = nombre.strip()
        if not nombre:
            return
        try:
            ruta = self.vault.resolver(nombre, desde=self._nota or None)
        except TypeError:
            ruta = self.vault.resolver(nombre)
        if ruta is None:
            if nombre.endswith(".md") and self.vault.existe(nombre):
                ruta = nombre
            else:
                titulo = nombre.split("#")[0].split("|")[0].strip() or nombre
                try:
                    ruta = self.vault.crear(titulo, "")
                except Exception as e:   # noqa: BLE001
                    self.p.general.aviso.emit(_mensaje_error(e), "error")
                    return
                self.p.general.aviso.emit(f"Creé la nota «{Path(ruta).stem}».", "info")
                self.recargar()
                self.pedirGrafo()
        self.abrir(ruta)

    @Slot(str)
    def guardar(self, texto: str) -> None:
        if not self._nota or texto == self._texto:
            return
        try:
            self.vault.guardar(self._nota, texto)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit("No pude guardar la nota: " + _mensaje_error(e), "error")
            return
        enlaces_antes = re.findall(_ENLACE, self._texto)
        self._texto = texto
        self.notaGuardada.emit()
        self.recargar()
        self._paneles()
        if re.findall(_ENLACE, texto) != enlaces_antes:
            self.pedirGrafo()

    @Slot(str, str)
    def guardarEn(self, ruta: str, texto: str) -> None:
        """Guarda el texto en esa nota aunque ya no sea la abierta (el editor guarda al cambiar de nota)."""
        if not ruta:
            return
        if ruta == self._nota:
            self.guardar(texto)
            return
        try:
            self.vault.guardar(ruta, texto)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit("No pude guardar la nota: " + _mensaje_error(e), "error")
            return
        self.recargar()

    def _paneles(self) -> None:
        if not self._nota:
            return
        try:
            self._retro = [{"ruta": r["ruta"], "titulo": r.get("titulo") or Path(r["ruta"]).stem,
                            "fragmento": r.get("fragmento", "")} for r in self.vault.retroenlaces(self._nota)]
            self._enlaces = [{"nombre": e.get("nombre", ""), "ruta": e.get("ruta") or "", "existe": bool(e.get("existe"))}
                             for e in self.vault.enlaces(self._nota)]
        except Exception:   # noqa: BLE001
            self._retro, self._enlaces = [], []
        try:
            from .vault import etiquetas_de
            self._etiquetas_nota = list(etiquetas_de(self._texto))
        except Exception:   # noqa: BLE001
            self._etiquetas_nota = sorted(set(re.findall(r"(?<![\w&])#([\w/-]+)", self._texto)))
        self.panelesCambio.emit()

    @Slot(str, str, result=str)
    def nuevaNota(self, titulo: str, carpeta: str = "") -> str:
        try:
            ruta = self.vault.crear(titulo, carpeta or "")
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return ""
        self.recargar()
        self.abrir(ruta)
        self.pedirGrafo()
        return ruta

    @Slot(str, str)
    def nuevaCarpeta(self, nombre: str, padre: str = "") -> None:
        ruta = f"{padre}/{nombre}" if padre else nombre
        try:
            ruta = self.vault.crear_carpeta(ruta)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return
        if padre:
            self._abiertas.add(padre)
        self._abiertas.add(ruta)
        self.recargar()

    @Slot(str, str)
    def renombrar(self, ruta: str, nuevo: str) -> None:
        nuevo = nuevo.strip()
        if not nuevo:
            return
        try:
            if ruta.endswith(".md"):
                nueva = self.vault.renombrar(ruta, nuevo)
            else:   # una carpeta
                origen = self.vault.ruta_absoluta(ruta)
                destino = origen.parent / re.sub(r'[\\/:*?"<>|#^\[\]]', "", nuevo)
                if destino.exists():
                    raise FileExistsError(str(destino))
                origen.rename(destino)
                nueva = str(destino.relative_to(self.raiz)).replace("\\", "/")
                if self._nota.startswith(ruta + "/"):
                    self._nota = nueva + self._nota[len(ruta):]
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return
        if ruta == self._nota:
            self._nota = nueva
            try:
                self._texto = self.vault.leer(nueva)
            except OSError:
                pass
            self.notaCambio.emit()
        self.recargar()
        self._paneles()
        self.pedirGrafo()

    @Slot(str)
    def borrar(self, ruta: str) -> None:
        try:
            self.vault.borrar(ruta)
        except Exception as e:   # noqa: BLE001
            self.p.general.aviso.emit(_mensaje_error(e), "error")
            return
        if self._nota == ruta or self._nota.startswith(ruta + "/"):
            self._nota, self._texto = "", ""
            self._retro, self._enlaces, self._etiquetas_nota = [], [], []
            self.notaCambio.emit()
        self.p.general.aviso.emit(f"Lo mandé a la papelera del vault (.trash): {Path(ruta).stem}", "info")
        self.recargar()
        self.pedirGrafo()

    def guardar_chat(self, titulo: str, texto: str) -> str:
        if not self.vault.existe("Chats"):
            try:
                self.vault.crear_carpeta("Chats")
            except Exception:   # noqa: BLE001
                pass
        ruta = self.vault.crear(titulo, "Chats")
        self.vault.guardar(ruta, texto)
        self.recargar()
        self.pedirGrafo()
        return ruta

    # --- búsqueda y autocompletado ---
    def _poner_busqueda(self, texto: str) -> None:
        if texto == self._busqueda:
            return
        self._busqueda = texto
        try:
            self._resultados = [{"ruta": r["ruta"], "titulo": r.get("titulo") or Path(r["ruta"]).stem,
                                 "fragmento": r.get("fragmento", "")} for r in self.vault.buscar(texto)] if texto.strip() else []
        except Exception:   # noqa: BLE001
            self._resultados = []
        self.resultadosCambio.emit()

    @Slot(str, result="QStringList")
    def sugerir(self, prefijo: str) -> list[str]:
        """Las notas para el autocompletado de [[: primero las que empiezan así, luego las que lo tienen."""
        clave = sin_acentos(prefijo.strip())
        empiezan = [t["titulo"] for t in self._titulos if t["clave"].startswith(clave)]
        tienen = [t["titulo"] for t in self._titulos if clave and clave in t["clave"] and not t["clave"].startswith(clave)]
        vistos: list[str] = []
        for t in sorted(empiezan, key=str.lower) + sorted(tienen, key=str.lower):
            if t not in vistos:
                vistos.append(t)
        return vistos[:8]

    @Slot(str, result=str)
    def vista(self, texto: str) -> str:
        return markdown_con_enlaces(texto)

    # --- grafo ---
    @Slot()
    def pedirGrafo(self) -> None:
        if self.vault is None:
            return
        v = self.vault
        self.p.hilos.correr(v.grafo, self._poner_grafo, lambda e: None)

    def _poner_grafo(self, g: dict) -> None:
        g = g or {}
        nodos = [{"id": n.get("id", ""), "titulo": n.get("titulo", ""), "grado": int(n.get("grado") or 0),
                  "existe": bool(n.get("existe", True))} for n in g.get("nodos", [])]
        ids = {n["id"]: i for i, n in enumerate(nodos)}
        aristas = [[ids[a["origen"]], ids[a["destino"]]] for a in g.get("aristas", [])
                   if a.get("origen") in ids and a.get("destino") in ids]
        self._grafo = {"nodos": nodos, "aristas": aristas}
        self.grafoCambio.emit()

    # --- memoria para la IA ---
    @Slot()
    def actualizarMemoria(self) -> None:
        if self._memoria >= 0:
            return
        self._cancelar_memoria = cancelar = threading.Event()
        self._memoria, self._memoria_texto = 0.0, "Preparando…"
        self.memoriaCambio.emit()
        ultimo = [0.0]

        def al_progreso(hechas: int, total: int, ruta: str) -> None:
            ahora = time.monotonic()
            if ahora - ultimo[0] >= 0.1 or hechas == total:
                ultimo[0] = ahora
                self._progresoMemoria.emit(int(hechas), int(total))

        def trabajo() -> tuple[dict, str, str]:
            mem = self.p.memoria()
            r = mem.actualizar(al_progreso, cancelar) or {}
            return r, getattr(mem, "modo", ""), getattr(mem, "aviso", "") or ""

        def listo(res: tuple[dict, str, str]) -> None:
            r, modo, aviso = res
            self._memoria = -1.0
            if r.get("cancelado"):
                self._memoria_texto = "Detenida: lo que ya iba quedó guardado."
            elif modo == "palabras":
                self._memoria_texto = "Sin modelo de embeddings: la IA busca por palabras."
            else:
                self._memoria_texto = f"Lista: {r.get('notas', 0)} notas, {r.get('fragmentos', 0)} fragmentos."
            self.memoriaCambio.emit()
            if aviso:
                self.p.general.aviso.emit(aviso, "info")
            elif not r.get("cancelado"):
                self.p.general.aviso.emit("Listo: la IA ya conoce tus notas.", "exito")

        def fallo(e: Exception) -> None:
            self._memoria = -1.0
            self._memoria_texto = ""
            self.memoriaCambio.emit()
            self.p.general.aviso.emit("No pude actualizar la memoria: " + _mensaje_error(e), "error")

        self.p.hilos.correr(trabajo, listo, fallo)

    @Slot()
    def cancelarMemoria(self) -> None:
        self._cancelar_memoria.set()

    @Slot(int, int)
    def _alProgresoMemoria(self, hechas: int, total: int) -> None:
        if self._memoria < 0:
            return
        self._memoria = hechas / total if total else 0.0
        self._memoria_texto = f"Leyendo {hechas} de {total} notas…"
        self.memoriaCambio.emit()

    nombre = Property(str, lambda s: s.raiz.name, notify=raizCambio)
    ruta = Property(str, lambda s: str(s.raiz), notify=raizCambio)
    nota = Property(str, lambda s: s._nota, notify=notaCambio)
    titulo = Property(str, lambda s: Path(s._nota).stem if s._nota else "", notify=notaCambio)
    carpetaNota = Property(str, lambda s: s._nota.rpartition("/")[0], notify=notaCambio)
    texto = Property(str, lambda s: s._texto, notify=notaCambio)
    retroenlaces = Property("QVariantList", lambda s: s._retro, notify=panelesCambio)
    enlaces = Property("QVariantList", lambda s: s._enlaces, notify=panelesCambio)
    etiquetasNota = Property("QStringList", lambda s: s._etiquetas_nota, notify=panelesCambio)
    etiquetas = Property("QVariantList", lambda s: s._etiquetas, notify=panelesCambio)
    totalNotas = Property(int, lambda s: len(s._titulos), notify=panelesCambio)
    busqueda = Property(str, lambda s: s._busqueda, _poner_busqueda, notify=resultadosCambio)
    resultados = Property("QVariantList", lambda s: s._resultados, notify=resultadosCambio)
    grafo = Property("QVariantMap", lambda s: s._grafo, notify=grafoCambio)
    memoriaProgreso = Property(float, lambda s: s._memoria, notify=memoriaCambio)
    memoriaTexto = Property(str, lambda s: s._memoria_texto, notify=memoriaCambio)
    lista = Property(QObject, lambda s: s.arbol, constant=True)


# --- todo junto ---

class Puente(QObject):
    """Arma los cuatro objetos y los arranca. `ollama` y `vram` se pueden cambiar (pruebas)."""

    def __init__(self, parent: QObject | None = None, ollama: Any = None, vram: dict | None = None,
                 arrancar: bool = True):
        super().__init__(parent)
        from . import VERSION
        self.version = VERSION
        self.config = Config()
        self.ollama = ollama if ollama is not None else Ollama()
        self.hilos = _Hilos(self)
        self.general = General(self)
        self.modelos = Modelos(self)
        self.chat = Chat(self)
        self.boveda = Boveda(self)
        self._vram = vram
        if arrancar:
            self.arrancar()

    def arrancar(self) -> None:
        self.boveda.arrancar()
        self.modelos.arrancar(self._vram)
        self.chat.arrancar()
        self.general.arrancar()

    def memoria(self):
        """Una Memoria nueva con su propio Vault (cada hilo los suyos: SQLite y la caché del vault no se
        comparten entre hilos)."""
        from .memoria import Memoria
        from .vault import Vault
        return Memoria(Vault(self.boveda.raiz), self.ollama, modelo_embeddings=self.config.get("modelo_embeddings") or None)

    @property
    def mostrarVentana(self) -> Signal:
        return self.general.mostrarVentana

    def cerrar(self) -> None:
        self.chat.detener()
        self.boveda.cancelarMemoria()
        for b in list(self.modelos._bajando.values()):
            b["cancelar"].set()
        parar = getattr(self.ollama, "parar_propio", None)
        if parar:
            try:
                parar()
            except Exception:   # noqa: BLE001
                pass
