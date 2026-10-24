"""Panel compacto del asistente de voz (sin avatar): Qt Widgets puro, sin QtWebEngine ni 3D.

Indicador de escucha, la respuesta en texto, un cuadro para escribir y el micrófono. Se cierra solo
20 s después de la última respuesta si no lo tocan (lo decide el Asistente); Esc lo cierra ya.
Los colores siguen al fondo de pantalla (colores.py) y el tamaño a general.uiScale de Ajustes.
"""

from PySide6.QtCore import QPointF, QRectF, Qt, QTimer
from PySide6.QtGui import QColor, QKeySequence, QPainter, QPainterPath, QPen, QShortcut
from PySide6.QtWidgets import (QHBoxLayout, QLabel, QLineEdit, QPushButton, QScrollArea, QVBoxLayout,
                               QWidget)

from . import colores

ANCHO, ALTO = 420, 240

ETIQUETAS = {
    "reposo": "Lista", "escuchando": "Escuchando...", "transcribiendo": "Un momento...",
    "pensando": "Pensando...", "viendo": "Mirando tu pantalla...", "hablando": "Hablando",
}
FUENTE = '"Google Sans Flex", "Noto Sans", "Cantarell", "DejaVu Sans", sans-serif'


def _qc(color: str, alfa: float | None = None) -> QColor:
    """Un color de la paleta («#rrggbb» o «#aarrggbb») como QColor; `alfa` (0..1) lo cambia."""
    c = QColor(color)
    if alfa is not None:
        c.setAlphaF(alfa)
    return c


class Indicador(QWidget):
    """Punto de estado: rojo y latiendo mientras el micrófono está abierto."""

    def __init__(self, ventana):
        super().__init__()
        self.v = ventana
        self.setFixedSize(ventana.px(14), ventana.px(14))
        self.estado = "reposo"
        self.fase = 0.0
        self.reloj = QTimer(self)
        self.reloj.timeout.connect(self._latir)

    def poner(self, estado: str) -> None:
        self.estado = estado
        if estado in ("escuchando", "transcribiendo", "pensando", "viendo"):
            self.reloj.start(50)
        else:
            self.reloj.stop()
            self.fase = 0.0
        self.update()

    def _latir(self) -> None:
        self.fase = (self.fase + 0.08) % 1.0
        self.update()

    def paintEvent(self, _ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        P = self.v.P
        color = P["peligro"] if self.estado == "escuchando" else P["azul"] if self.estado != "reposo" else P["tenue"]
        pulso = 0.5 - 0.5 * abs(2 * self.fase - 1) if self.reloj.isActive() else 0.0
        p.setBrush(_qc(color, 0.45 + 0.55 * (1 - pulso)))
        p.setPen(Qt.PenStyle.NoPen)
        r = self.v.px(4.5 + 1.5 * (1 - pulso))
        c = self.width() / 2
        p.drawEllipse(QPointF(c, c), r, r)


class BotonMicrofono(QPushButton):
    """Micrófono dibujado a mano (sin iconos del tema); con anillo que crece con tu voz al escuchar."""

    def __init__(self, ventana):
        super().__init__()
        self.v = ventana
        self.setFixedSize(ventana.px(40), ventana.px(40))
        self.setCursor(Qt.CursorShape.PointingHandCursor)
        self.setToolTip("Hablar (otro clic para terminar)")
        self.activo = False
        self.nivel = 0.0

    def poner(self, activo: bool) -> None:
        self.activo = activo
        if not activo:
            self.nivel = 0.0
        self.update()

    def paintEvent(self, _ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        P, s = self.v.P, self.v.escala
        p.scale(s, s)
        centro = QPointF(20, 20)
        if self.activo:
            p.setPen(QPen(_qc(P["peligro"], 0.7), 2))
            p.setBrush(Qt.BrushStyle.NoBrush)
            r = 17 + 3 * self.nivel
            p.drawEllipse(centro, r, r)
        p.setPen(QPen(_qc(P["bordeCampo"]), 1))
        p.setBrush(_qc(P["peligro"]) if self.activo else _qc(P["campo"]))
        p.drawEllipse(centro, 16, 16)
        icono = QPen(_qc(P["fondo"]) if self.activo else _qc(P["texto"]), 1.8)
        icono.setCapStyle(Qt.PenCapStyle.RoundCap)
        p.setPen(icono)
        p.setBrush(Qt.BrushStyle.NoBrush)
        p.drawRoundedRect(QRectF(16.5, 10, 7, 12), 3.5, 3.5)
        arco = QPainterPath()
        arco.moveTo(13, 20)
        arco.arcTo(QRectF(13, 14, 14, 12), 180, 180)
        p.drawPath(arco)
        p.drawLine(QPointF(20, 26), QPointF(20, 29.5))


class Barra(QWidget):
    """Barra de progreso de la descarga de los modelos."""

    def __init__(self, ventana):
        super().__init__()
        self.v = ventana
        self.fraccion = 0.0
        self.setFixedHeight(ventana.px(8))

    def poner(self, fraccion: float) -> None:
        self.fraccion = max(0.0, min(1.0, fraccion))
        self.update()

    def paintEvent(self, _ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        P = self.v.P
        r = self.height() / 2
        p.setPen(Qt.PenStyle.NoPen)
        p.setBrush(_qc(P["relleno"]))
        p.drawRoundedRect(QRectF(self.rect()), r, r)
        p.setBrush(_qc(P["azul"]))
        p.drawRoundedRect(QRectF(0, 0, max(self.height(), self.width() * self.fraccion), self.height()), r, r)


class PanelVoz(QWidget):
    def __init__(self, asistente, on_cerrar, escala: float | None = None, paleta: dict | None = None):
        super().__init__()
        self.asistente = asistente
        self.on_cerrar = on_cerrar
        self.escala = escala if escala is not None else colores.escala_ui()
        self.P = paleta or colores.leer()
        self._aviso_activo = False
        self.setWindowTitle("Asistente de Vortex")
        self.setFixedSize(self.px(ANCHO), self.px(ALTO))
        self.setAttribute(Qt.WidgetAttribute.WA_TranslucentBackground)
        self.setWindowFlags(Qt.WindowType.FramelessWindowHint)

        raiz = QVBoxLayout(self)
        raiz.setContentsMargins(self.px(14), self.px(10), self.px(14), self.px(12))
        raiz.setSpacing(self.px(8))

        # Fila de arriba: indicador, estado y cerrar
        arriba = QHBoxLayout()
        arriba.setSpacing(self.px(8))
        self.punto = Indicador(self)
        self.estado = QLabel("Lista")
        arriba.addWidget(self.punto)
        arriba.addWidget(self.estado, 1)
        self.cerrar = QPushButton("✕")
        self.cerrar.setFixedSize(self.px(26), self.px(26))
        self.cerrar.setCursor(Qt.CursorShape.PointingHandCursor)
        self.cerrar.setToolTip("Cerrar (Esc)")
        self.cerrar.clicked.connect(self.on_cerrar)
        arriba.addWidget(self.cerrar)
        raiz.addLayout(arriba)

        # El cuerpo: lo que dijiste, la respuesta, la descarga de los modelos y los botones
        self.tu = QLabel("")
        self.tu.setWordWrap(True)
        self.respuesta = QLabel("")
        self.respuesta.setTextFormat(Qt.TextFormat.PlainText)      # nunca HTML: lo que dice Claude (y los permisos) se ve tal cual
        self.respuesta.setWordWrap(True)
        self.respuesta.setTextInteractionFlags(Qt.TextInteractionFlag.TextSelectableByMouse)
        self.respuesta.setAlignment(Qt.AlignmentFlag.AlignTop | Qt.AlignmentFlag.AlignLeft)
        self.prep = QWidget()
        self.prep_titulo = QLabel("")
        self.prep_titulo.setWordWrap(True)
        self.prep_barra = Barra(self)
        self.prep_detalle = QLabel("")
        lp = QVBoxLayout(self.prep)
        lp.setContentsMargins(0, self.px(2), 0, 0)
        lp.setSpacing(self.px(4))
        for w in (self.prep_titulo, self.prep_barra, self.prep_detalle):
            lp.addWidget(w)
        self.prep.hide()
        self.botones = QWidget()
        self.fila_botones = QHBoxLayout(self.botones)
        self.fila_botones.setContentsMargins(0, self.px(2), 0, 0)
        self.fila_botones.setSpacing(self.px(8))
        self.botones.hide()
        contenido = QWidget()
        lv = QVBoxLayout(contenido)
        lv.setContentsMargins(0, 0, self.px(4), 0)
        lv.setSpacing(self.px(2))
        for w in (self.tu, self.respuesta, self.prep, self.botones):
            lv.addWidget(w)
        lv.addStretch(1)
        self.scroll = QScrollArea()
        self.scroll.setWidgetResizable(True)
        self.scroll.setFrameShape(QScrollArea.Shape.NoFrame)
        self.scroll.setWidget(contenido)
        self.scroll.viewport().setStyleSheet("background: transparent;")
        contenido.setStyleSheet("background: transparent;")
        raiz.addWidget(self.scroll, 1)

        # Fila de abajo: escribir y micrófono
        abajo = QHBoxLayout()
        abajo.setSpacing(self.px(8))
        self.entrada = QLineEdit()
        self.entrada.setPlaceholderText("Escribe aquí...")
        self.entrada.setMaxLength(600)
        self.entrada.setFixedHeight(self.px(40))
        self.entrada.returnPressed.connect(self._enviar)
        self.entrada.textEdited.connect(lambda _t: self.asistente.tocada())
        self.mic = BotonMicrofono(self)
        self.mic.clicked.connect(self._microfono)
        abajo.addWidget(self.entrada, 1)
        abajo.addWidget(self.mic)
        raiz.addLayout(abajo)

        self.aplicar_paleta(self.P)
        QShortcut(QKeySequence(Qt.Key.Key_Escape), self, activated=self.on_cerrar)
        a = asistente
        a.estado.connect(self._estado)
        a.usuario.connect(lambda t: (self.tu.setText(f"Tú: {t}"), self.respuesta.setText(""), self._tocada()))
        a.respuesta.connect(self._respuesta)
        a.aviso.connect(self._aviso)
        a.nivel.connect(self._nivel)
        a.progreso.connect(self._progreso)
        a.acciones.connect(self._acciones)
        self._vigilante = colores.vigilante(self.aplicar_paleta)       # el fondo cambió → cambian los colores

    # ---- Tamaño y colores ---------------------------------------------------------------------------

    def px(self, n: float) -> int:
        return max(1, round(n * self.escala))

    def aplicar_paleta(self, P: dict) -> None:
        self.P = P
        px = self.px
        self.setStyleSheet(f"QWidget {{ font-family: {FUENTE}; }}")
        self.estado.setStyleSheet(f"color: {P['tenue']}; font-size: {px(12)}px;")
        self.cerrar.setStyleSheet(f"QPushButton {{ color: {P['tenue']}; border: none; background: transparent; font-size: {px(14)}px; }}"
                                  f"QPushButton:hover {{ color: {P['texto']}; }}")
        self.tu.setStyleSheet(f"color: {P['tenue']}; font-size: {px(11)}px; font-style: italic;")
        self._estilo_respuesta(self._aviso_activo)
        self.prep_titulo.setStyleSheet(f"color: {P['texto']}; font-size: {px(13)}px;")
        self.prep_detalle.setStyleSheet(f"color: {P['tenue']}; font-size: {px(11)}px;")
        self.entrada.setStyleSheet(
            f"QLineEdit {{ color: {P['texto']}; background: {P['campo']}; border: 1px solid {P['bordeCampo']}; "
            f"border-radius: {px(colores.RADIO + 2)}px; padding: 0 {px(14)}px; font-size: {px(14)}px; selection-background-color: {P['realceFuerte']}; }}"
            f"QLineEdit:focus {{ border-color: {P['azul']}; }}")
        self.scroll.setStyleSheet(f"QScrollArea {{ background: transparent; }} QScrollBar:vertical {{ width: {px(6)}px; background: transparent; }}"
                                  f"QScrollBar::handle:vertical {{ background: {P['borde']}; border-radius: {px(3)}px; }}"
                                  "QScrollBar::add-line, QScrollBar::sub-line { height: 0; }")
        for b in self.botones.findChildren(QPushButton):
            self._estilo_boton(b)
        self.update()
        self.punto.update()
        self.mic.update()
        self.prep_barra.update()

    def _estilo_respuesta(self, aviso: bool) -> None:
        P = self.P
        self.respuesta.setStyleSheet(f"color: {P['tenue'] if aviso else P['texto']}; font-size: {self.px(13 if aviso else 14)}px;")

    def _estilo_boton(self, b: QPushButton) -> None:
        P, px = self.P, self.px
        b.setStyleSheet(f"QPushButton {{ color: {P['sobreAcento']}; background: {P['azul']}; border: none; border-radius: {px(colores.RADIO - 2)}px; "
                        f"padding: 0 {px(16)}px; font-size: {px(13)}px; font-weight: 600; }}"
                        f"QPushButton:hover {{ background: {P['azulHover']}; }} QPushButton:pressed {{ background: {P['azulPresion']}; }}")

    # ---- Dibujo -----------------------------------------------------------------------------------

    def paintEvent(self, _ev):
        p = QPainter(self)
        p.setRenderHint(QPainter.RenderHint.Antialiasing)
        p.setBrush(_qc(self.P["flotante"]))
        p.setPen(QPen(_qc(self.P["borde"]), 1))
        r = self.px(colores.RADIO)
        p.drawRoundedRect(QRectF(0.5, 0.5, self.width() - 1, self.height() - 1), r, r)

    # ---- Eventos del asistente ------------------------------------------------------------------

    def _estado(self, estado: str, texto: str) -> None:
        self.estado.setText(texto or ETIQUETAS.get(estado, estado))
        self.punto.poner(estado)
        self.mic.poner(estado == "escuchando")

    def _respuesta(self, texto: str, final: bool) -> None:
        self._aviso_activo = False
        self._estilo_respuesta(False)
        self.respuesta.setText(texto or ("..." if not final else ""))
        QTimer.singleShot(0, lambda: self.scroll.verticalScrollBar().setValue(self.scroll.verticalScrollBar().maximum()))

    def _aviso(self, texto: str) -> None:
        self._aviso_activo = True
        self._estilo_respuesta(True)
        self.respuesta.setText(texto)

    def _nivel(self, v: float) -> None:
        self.mic.nivel = v
        self.mic.update()

    def _progreso(self, titulo: str, detalle: str, fraccion: float) -> None:
        """La descarga de los modelos (solo la primera vez): título, barra y cuántos MB van."""
        if fraccion < 0:
            self.prep.hide()
            return
        self.prep_titulo.setText(titulo)
        self.prep_detalle.setText(detalle)
        self.prep_barra.poner(fraccion)
        self.prep.show()

    def _acciones(self, botones: list) -> None:
        """Botones bajo la respuesta (Activar Claude / Iniciar sesión)."""
        while self.fila_botones.count():
            w = self.fila_botones.takeAt(0).widget()
            if w is not None:
                w.setParent(None)
                w.deleteLater()
        for id_accion, texto in botones:
            b = QPushButton(texto)
            b.setFixedHeight(self.px(34))
            b.setCursor(Qt.CursorShape.PointingHandCursor)
            self._estilo_boton(b)
            b.clicked.connect(lambda _=False, i=id_accion: (self._tocada(), self.asistente.accion(i)))
            self.fila_botones.addWidget(b)
        self.fila_botones.addStretch(1)
        self.botones.setVisible(bool(botones))

    # ---- Acciones de la persona -----------------------------------------------------------------

    def _tocada(self) -> None:
        self.asistente.tocada()

    def _enviar(self) -> None:
        t = self.entrada.text().strip()
        if t:
            self.entrada.clear()
            self.asistente.enviar(t)

    def _microfono(self) -> None:
        self.asistente.tocada()
        if self.asistente.estado_actual == "escuchando":
            self.asistente.cortar_escucha()
        else:
            self.asistente.escuchar()

    def mousePressEvent(self, ev):
        self.asistente.tocada()
        super().mousePressEvent(ev)

    def closeEvent(self, ev):
        self.on_cerrar()
        ev.accept()
