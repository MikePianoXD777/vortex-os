"""La ventana de Miku con avatar: una página local (three.js + three-vrm) en QtWebEngine, sin borde y transparente.

Solo se importa cuando el 3D está encendido: sin avatar no se carga QtWebEngine (ahorra ~500 MB de RAM).
La página habla con Python por QWebChannel (objeto «puente») y Python con la página por runJavaScript.
Los colores siguen al fondo de pantalla (colores.py) y el tamaño a general.uiScale de Ajustes.
"""

import json
import os
import sys
from pathlib import Path

from PySide6.QtCore import QObject, Qt, QUrl, QUrlQuery, Slot
from PySide6.QtGui import QColor
from PySide6.QtWebChannel import QWebChannel
from PySide6.QtWebEngineCore import QWebEnginePage, QWebEngineProfile, QWebEngineSettings
from PySide6.QtWebEngineWidgets import QWebEngineView

from . import colores, rutas

ANCHO, ALTO = 460, 760


class Pagina(QWebEnginePage):
    """Solo deja abrir archivos locales: la página es nuestra y nunca sale a internet."""

    def acceptNavigationRequest(self, url, tipo, principal):
        return url.scheme() == "file"

    def javaScriptConsoleMessage(self, nivel, mensaje, linea, origen):
        if nivel != QWebEnginePage.JavaScriptConsoleMessageLevel.InfoMessageLevel:
            print(f"miku[js]: {mensaje} ({origen.rsplit('/', 1)[-1]}:{linea})", file=sys.stderr, flush=True)


class Puente(QObject):
    """Lo que la página puede pedirle a Python."""

    def __init__(self, ventana):
        super().__init__(ventana)
        self.v = ventana

    @Slot()
    def listo(self):
        self.v.pagina_lista()

    @Slot(str)
    def enviarTexto(self, texto):
        self.v.asistente.enviar(texto)

    @Slot()
    def microfono(self):
        self.v.al_microfono()

    @Slot()
    def cerrar(self):
        self.v.cerrar_ventana()

    @Slot()
    def tocada(self):
        self.v.asistente.tocada()

    @Slot(str)
    def accion(self, id_accion):
        self.v.asistente.tocada()
        self.v.asistente.accion(id_accion)


class VentanaAvatar(QWebEngineView):
    def __init__(self, asistente, modelo: Path | None, on_cerrar, fps: int = 30, escala: float | None = None):
        super().__init__()
        self.asistente = asistente
        self.on_cerrar = on_cerrar
        self.modelo = modelo
        self.escala = escala if escala is not None else colores.escala_ui()
        self.setWindowTitle("Miku")
        self.setFixedSize(round(ANCHO * self.escala), round(ALTO * self.escala))
        # Transparente de verdad: hace falta en la página, en el widget y en la superficie de la ventana.
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setStyleSheet("background: transparent")
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)
        self.setContextMenuPolicy(Qt.ContextMenuPolicy.NoContextMenu)

        # Perfil sin nombre = en memoria: no deja caché, cookies ni historial en el disco.
        self.perfil = QWebEngineProfile(self)
        self.pag = Pagina(self.perfil, self)
        self.setPage(self.pag)
        self.setZoomFactor(self.escala)               # la página se arma para 460x760: con otra escala, todo crece o se encoge igual
        self.pag.setBackgroundColor(QColor(0, 0, 0, 0))
        a = self.pag.settings()
        a.setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessFileUrls, True)   # imports y VRM por file://
        a.setAttribute(QWebEngineSettings.WebAttribute.LocalContentCanAccessRemoteUrls, False)
        a.setAttribute(QWebEngineSettings.WebAttribute.JavascriptCanOpenWindows, False)
        a.setAttribute(QWebEngineSettings.WebAttribute.ErrorPageEnabled, False)
        a.setAttribute(QWebEngineSettings.WebAttribute.PluginsEnabled, False)
        a.setAttribute(QWebEngineSettings.WebAttribute.ShowScrollBars, False)
        a.setAttribute(QWebEngineSettings.WebAttribute.WebGLEnabled, True)

        self.canal = QWebChannel(self.pag)
        self.puente = Puente(self)
        self.canal.registerObject("puente", self.puente)
        self.pag.setWebChannel(self.canal)
        self._pagina_lista = False
        self._pendientes: list[str] = []

        a_ = asistente
        a_.estado.connect(lambda e, t: self.js("estado", e, t))
        a_.usuario.connect(lambda t: self.js("usuario", t))
        a_.respuesta.connect(lambda t, fin: self.js("respuesta", t, fin))
        a_.aviso.connect(lambda t: self.js("aviso", t))
        a_.nivel.connect(lambda v: self.js("nivel", round(v, 2)))
        a_.boca.connect(lambda pista, hz: self.js("visemas", pista, hz))
        a_.boca_fin.connect(lambda: self.js("pararBoca"))
        a_.emocion.connect(lambda n, p: self.js("emocion", n, p))
        a_.sonrojo.connect(lambda v: self.js("sonrojo", round(v, 2)))
        a_.progreso.connect(lambda t, d, f: self.js("progreso", t, d, round(f, 3)))
        a_.acciones.connect(lambda lista: self.js("acciones", lista))
        self._vigilante = colores.vigilante(self.poner_tema)        # el fondo cambió → cambian los colores
        self._tema = self.variables_css(self._vigilante.paleta)

        url = QUrl.fromLocalFile(str(rutas.WEB / "index.html"))
        q = QUrlQuery()
        if modelo:
            q.addQueryItem("modelo", QUrl.fromLocalFile(str(modelo)).toString())
        q.addQueryItem("vendor", QUrl.fromLocalFile(str(rutas.VENDOR)).toString() + "/")
        q.addQueryItem("fps", str(fps))
        for clave in ("encuadre", "retraso", "prueba"):          # ajustes de la página para pruebas
            v = os.environ.get("VORTEX_MIKU_WEB_" + clave.upper())
            if v:
                q.addQueryItem(clave, v)
        url.setQuery(q)
        self.load(url)

    # ---- Python → página ------------------------------------------------------------------------

    def js(self, funcion: str, *args) -> None:
        codigo = f"window.miku && miku.{funcion}({', '.join(json.dumps(a, ensure_ascii=False) for a in args)})"
        if self._pagina_lista:
            self.pag.runJavaScript(codigo)
        else:
            self._pendientes.append(codigo)

    @staticmethod
    def variables_css(P: dict) -> dict:
        """La paleta → variables CSS de la página (--cristal, --texto, --acento…)."""
        return {
            "cristal": colores.a_css(P["flotante"]), "campo": P["campo"], "borde": P["borde"], "borde-campo": P["bordeCampo"],
            "texto": P["texto"], "tenue": P["tenue"], "acento": P["azul"], "acento-hover": P["azulHover"],
            "sobre-acento": P["sobreAcento"], "relleno": P["relleno"], "rojo": P["peligro"],
            "radio": f"{colores.RADIO}px",
        }

    def poner_tema(self, paleta: dict) -> None:
        self._tema = self.variables_css(paleta)
        self.js("tema", self._tema)

    def pagina_lista(self) -> None:
        """La página conectó el canal: se le manda el tema, el crédito y lo que se acumuló mientras cargaba."""
        credito = rutas.leer_credito(self.modelo)
        primera = credito.splitlines()[0] if credito else ""
        self._pagina_lista = True
        self.js("tema", self._tema)
        self.js("credito", credito, primera if len(primera) <= 90 else primera[:87] + "...")
        for c in self._pendientes:
            self.pag.runJavaScript(c)
        self._pendientes.clear()

    # ---- página → Python ------------------------------------------------------------------------

    def al_microfono(self) -> None:
        a = self.asistente
        if a.estado_actual == "escuchando":
            a.cortar_escucha()
        else:
            a.escuchar()

    def cerrar_ventana(self) -> None:
        self.on_cerrar()

    def closeEvent(self, ev):
        self.on_cerrar()
        ev.accept()
