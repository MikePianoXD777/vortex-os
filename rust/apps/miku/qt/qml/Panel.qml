// El panel compacto del asistente de voz (sin avatar): indicador de escucha, la respuesta en texto, un cuadro
// para escribir y el micrófono. Se cierra solo 20 s después de la última respuesta si no lo tocan (lo decide
// el asistente); Esc lo cierra ya. Los colores siguen al fondo de pantalla y el tamaño a general.uiScale.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Window {
    id: w

    readonly property real s: mk.escala
    readonly property var pal: mk.paleta
    readonly property real radio: 18
    function px(n) { return Math.max(1, Math.round(n * s)) }

    objectName: "ventanaPanel"
    title: "Asistente de Vortex"
    width: px(420)
    height: px(240)
    minimumWidth: width
    maximumWidth: width
    minimumHeight: height
    maximumHeight: height
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    visible: true

    Rectangle {
        id: tarjeta
        objectName: "tarjeta"
        anchors.fill: parent
        color: w.pal.flotante
        border.color: w.pal.borde
        border.width: 1
        radius: w.px(w.radio)
        focus: true
        Keys.onEscapePressed: mk.cerrar()

        TapHandler { onPressedChanged: if (pressed) mk.tocada() }

        ColumnLayout {
            anchors.fill: parent
            anchors.leftMargin: w.px(14)
            anchors.rightMargin: w.px(14)
            anchors.topMargin: w.px(10)
            anchors.bottomMargin: w.px(12)
            spacing: w.px(8)

            // Arriba: indicador, estado y cerrar
            RowLayout {
                Layout.fillWidth: true
                spacing: w.px(8)
                Item {
                    id: punto
                    objectName: "punto"
                    Layout.preferredWidth: w.px(14)
                    Layout.preferredHeight: w.px(14)
                    property real pulso: 0
                    readonly property bool activo: ["escuchando", "transcribiendo", "pensando", "viendo"].indexOf(mk.estado) >= 0
                    readonly property color color: mk.estado === "escuchando" ? w.pal.peligro : mk.estado !== "reposo" ? w.pal.azul : w.pal.tenue
                    SequentialAnimation on pulso {
                        running: punto.activo
                        loops: Animation.Infinite
                        NumberAnimation { to: 0.5; duration: 312 }
                        NumberAnimation { to: 0; duration: 312 }
                    }
                    Rectangle {
                        anchors.centerIn: parent
                        readonly property real r: w.px(4.5 + 1.5 * (1 - (punto.activo ? punto.pulso : 0)))
                        width: r * 2
                        height: r * 2
                        radius: r
                        color: punto.color
                        opacity: 0.45 + 0.55 * (1 - (punto.activo ? punto.pulso : 0))
                    }
                }
                Text {
                    objectName: "estadoTexto"
                    Layout.fillWidth: true
                    text: mk.estadoTexto
                    color: w.pal.tenue
                    font.pixelSize: w.px(12)
                }
                Text {
                    objectName: "camara"
                    visible: mk.camara
                    text: "● Cámara"
                    color: w.pal.peligro
                    font.pixelSize: w.px(11)
                }
                AbstractButton {
                    objectName: "cerrar"
                    Layout.preferredWidth: w.px(26)
                    Layout.preferredHeight: w.px(26)
                    hoverEnabled: true
                    onClicked: mk.cerrar()
                    contentItem: Text {
                        text: "✕"
                        horizontalAlignment: Text.AlignHCenter
                        verticalAlignment: Text.AlignVCenter
                        color: parent.hovered ? w.pal.texto : w.pal.tenue
                        font.pixelSize: w.px(14)
                    }
                    background: null
                    ToolTip.visible: hovered
                    ToolTip.text: "Cerrar (Esc)"
                }
            }

            // El cuerpo
            Flickable {
                id: cuerpo
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentWidth: width
                contentHeight: columna.height
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar {
                    implicitWidth: w.px(6)
                    contentItem: Rectangle { implicitWidth: w.px(6); radius: w.px(3); color: w.pal.borde }
                    background: null
                }
                onContentHeightChanged: contentY = Math.max(0, contentHeight - height)

                Column {
                    id: columna
                    width: cuerpo.width - w.px(4)
                    spacing: w.px(2)

                    Text {
                        objectName: "tu"
                        width: parent.width
                        visible: text !== ""
                        text: mk.tu
                        wrapMode: Text.Wrap
                        color: w.pal.tenue
                        font.pixelSize: w.px(11)
                        font.italic: true
                    }
                    TextEdit {
                        objectName: "respuesta"
                        width: parent.width
                        text: mk.respuesta
                        readOnly: true
                        selectByMouse: true
                        textFormat: TextEdit.PlainText   // nunca HTML: lo que dice Claude (y los permisos) se ve tal cual
                        wrapMode: TextEdit.Wrap
                        color: mk.esAviso ? w.pal.tenue : w.pal.texto
                        selectionColor: w.pal.realceFuerte
                        font.pixelSize: w.px(mk.esAviso ? 13 : 14)
                    }
                    // El recorte de pantalla de un permiso de clic, con el punto.
                    Image {
                        id: vista
                        objectName: "vista"
                        width: parent.width
                        visible: mk.vistaRuta !== "" && status === Image.Ready
                        source: mk.vistaRuta !== "" ? "file://" + encodeURI(mk.vistaRuta) : ""
                        fillMode: Image.PreserveAspectFit
                        height: visible ? width * sourceSize.height / Math.max(1, sourceSize.width) : 0
                        Canvas {
                            anchors.fill: parent
                            visible: vista.visible
                            onPaint: {
                                var c = getContext("2d")
                                c.clearRect(0, 0, width, height)
                                var f = width / Math.max(1, vista.sourceSize.width)
                                var x = mk.vistaX * f, y = mk.vistaY * f, r = w.px(14)
                                var trazos = [["rgba(0,0,0,0.78)", w.px(5)], ["#ffffff", w.px(2)]]
                                for (var i = 0; i < trazos.length; i++) {
                                    c.strokeStyle = trazos[i][0]
                                    c.lineWidth = trazos[i][1]
                                    c.beginPath(); c.arc(x, y, r, 0, 2 * Math.PI); c.stroke()
                                    c.beginPath()
                                    c.moveTo(x - r * 1.6, y); c.lineTo(x - r * 0.5, y)
                                    c.moveTo(x + r * 0.5, y); c.lineTo(x + r * 1.6, y)
                                    c.moveTo(x, y - r * 1.6); c.lineTo(x, y - r * 0.5)
                                    c.moveTo(x, y + r * 0.5); c.lineTo(x, y + r * 1.6)
                                    c.stroke()
                                }
                            }
                            Connections {
                                target: mk
                                function onVistaXChanged() { requestPaint() }
                                function onVistaYChanged() { requestPaint() }
                            }
                        }
                    }
                    // La descarga de los modelos (solo la primera vez).
                    Column {
                        objectName: "preparando"
                        width: parent.width
                        visible: mk.progreso >= 0
                        spacing: w.px(4)
                        topPadding: w.px(2)
                        Text {
                            objectName: "progresoTitulo"
                            width: parent.width
                            text: mk.progresoTitulo
                            wrapMode: Text.Wrap
                            color: w.pal.texto
                            font.pixelSize: w.px(13)
                        }
                        Rectangle {
                            objectName: "progresoBarra"
                            property real fraccion: Math.max(0, Math.min(1, mk.progreso))
                            width: parent.width
                            height: w.px(8)
                            radius: height / 2
                            color: w.pal.relleno
                            Rectangle {
                                width: Math.max(parent.height, parent.width * parent.fraccion)
                                height: parent.height
                                radius: height / 2
                                color: w.pal.azul
                            }
                        }
                        Text {
                            objectName: "progresoDetalle"
                            width: parent.width
                            text: mk.progresoDetalle
                            wrapMode: Text.Wrap
                            color: w.pal.tenue
                            font.pixelSize: w.px(11)
                        }
                    }
                    // Botones bajo la respuesta (Activar Claude / Iniciar sesión).
                    Row {
                        objectName: "botones"
                        visible: botonesRep.count > 0
                        spacing: w.px(8)
                        topPadding: w.px(2)
                        Repeater {
                            id: botonesRep
                            model: mk.acciones
                            delegate: AbstractButton {
                                objectName: "boton" + index
                                property string texto: modelData.texto
                                property string idAccion: modelData.id
                                height: w.px(34)
                                hoverEnabled: true
                                leftPadding: w.px(16)
                                rightPadding: w.px(16)
                                onClicked: mk.accion(idAccion)
                                contentItem: Text {
                                    text: parent.texto
                                    verticalAlignment: Text.AlignVCenter
                                    color: w.pal.sobreAcento
                                    font.pixelSize: w.px(13)
                                    font.weight: Font.DemiBold
                                }
                                background: Rectangle {
                                    radius: w.px(w.radio - 2)
                                    color: parent.down ? w.pal.azulPresion : parent.hovered ? w.pal.azulHover : w.pal.azul
                                }
                            }
                        }
                    }
                }
            }

            // Abajo: escribir y micrófono
            RowLayout {
                Layout.fillWidth: true
                spacing: w.px(8)
                TextField {
                    id: entrada
                    objectName: "entrada"
                    Layout.fillWidth: true
                    Layout.preferredHeight: w.px(40)
                    placeholderText: "Escribe aquí..."
                    placeholderTextColor: w.pal.tenue
                    maximumLength: 600
                    color: w.pal.texto
                    selectionColor: w.pal.realceFuerte
                    selectedTextColor: w.pal.texto
                    font.pixelSize: w.px(14)
                    leftPadding: w.px(14)
                    rightPadding: w.px(14)
                    verticalAlignment: TextInput.AlignVCenter
                    background: Rectangle {
                        radius: w.px(w.radio + 2)
                        color: w.pal.campo
                        border.width: 1
                        border.color: entrada.activeFocus ? w.pal.azul : w.pal.bordeCampo
                    }
                    onAccepted: {
                        var t = text.trim()
                        if (t !== "") {
                            text = ""
                            mk.enviarTexto(t)
                        }
                    }
                    onTextEdited: mk.tocada()
                }
                AbstractButton {
                    id: mic
                    objectName: "mic"
                    Layout.preferredWidth: w.px(40)
                    Layout.preferredHeight: w.px(40)
                    property bool activo: mk.estado === "escuchando"
                    property real nivel: activo ? mk.nivel : 0
                    onActivoChanged: lienzo.requestPaint()
                    onNivelChanged: lienzo.requestPaint()
                    onClicked: { mk.tocada(); mk.microfono() }
                    hoverEnabled: true
                    ToolTip.visible: hovered
                    ToolTip.text: "Hablar (otro clic para terminar)"
                    background: null
                    contentItem: Canvas {
                        id: lienzo
                        // Micrófono dibujado a mano; con anillo que crece con tu voz al escuchar.
                        onPaint: {
                            var c = getContext("2d")
                            c.reset()
                            c.scale(w.s, w.s)
                            if (mic.activo) {
                                c.strokeStyle = Qt.rgba(w.pal.peligro.r, w.pal.peligro.g, w.pal.peligro.b, 0.7)
                                c.lineWidth = 2
                                c.beginPath(); c.arc(20, 20, 17 + 3 * mic.nivel, 0, 2 * Math.PI); c.stroke()
                            }
                            c.lineWidth = 1
                            c.strokeStyle = w.pal.bordeCampo
                            c.fillStyle = mic.activo ? w.pal.peligro : w.pal.campo
                            c.beginPath(); c.arc(20, 20, 16, 0, 2 * Math.PI); c.fill(); c.stroke()
                            c.strokeStyle = mic.activo ? w.pal.fondo : w.pal.texto
                            c.lineWidth = 1.8
                            c.lineCap = "round"
                            c.beginPath(); c.roundedRect(16.5, 10, 7, 12, 3.5, 3.5); c.stroke()
                            c.beginPath(); c.arc(20, 20, 7, 0, Math.PI); c.stroke()
                            c.beginPath(); c.moveTo(20, 26); c.lineTo(20, 29.5); c.stroke()
                        }
                        Connections {
                            target: mk
                            function onPaletaChanged() { lienzo.requestPaint() }
                        }
                    }
                }
            }
        }
    }
}
