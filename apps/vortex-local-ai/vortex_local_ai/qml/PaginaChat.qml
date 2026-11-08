// Chat: a la izquierda tus chats; a la derecha el modelo, los mensajes (que salen mientras escribe) y la
// caja para escribir. «Usar mi vault» hace que la IA lea tus notas y las cite como chips que se tocan.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: raiz
    property bool activa: false
    signal irA(string pagina)

    function mandar() {
        var t = caja.text.trim()
        if (t === "" || chat.generando)
            return
        chat.enviar(t)
        caja.text = ""
        mensajes.pegado = true
    }
    function abrirNota(nombre, ruta) {
        if (ruta) boveda.abrir(ruta)
        else boveda.abrirEnlace(nombre)
        raiz.irA("vault")
    }
    function sincronizarModelo() {
        selector.currentIndex = modelos.nombresChat.indexOf(chat.modelo)
    }

    onActivaChanged: if (activa) caja.forceActiveFocus()
    Connections { target: chat; function onModeloCambio() { raiz.sincronizarModelo() } }
    Connections { target: modelos; function onInstaladosCambio() { Qt.callLater(raiz.sincronizarModelo) } }

    Dialogo { id: dialogo }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // --- lista de chats ---
        Rectangle {
            Layout.preferredWidth: 250
            Layout.fillHeight: true
            color: Tema.fondo
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 10
                Boton {
                    Layout.fillWidth: true
                    tipo: "secundario"
                    icono: "mas"
                    text: "Chat nuevo"
                    onClicked: { chat.nuevo(); caja.forceActiveFocus() }
                }
                Text {
                    Layout.topMargin: 6
                    Layout.leftMargin: 6
                    text: "Tus chats"
                    color: Tema.tenue
                    font.pixelSize: 11
                    font.weight: Font.DemiBold
                    font.letterSpacing: 1
                    font.capitalization: Font.AllUppercase
                }
                ListView {
                    id: listaChats
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 2
                    model: chat.lista
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    delegate: Rectangle {
                        id: filaChat
                        required property string clave
                        required property string titulo
                        required property string fecha
                        required property bool vault
                        readonly property bool elegido: chat.actual === clave
                        width: ListView.view.width
                        height: 44
                        radius: 12
                        color: elegido ? Tema.barraSel : zonaChat.containsMouse ? Tema.barraHover : "transparent"
                        MouseArea {
                            id: zonaChat
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: chat.abrir(filaChat.clave)
                        }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 6
                            spacing: 8
                            Icono {
                                Layout.preferredWidth: 15; Layout.preferredHeight: 15
                                nombre: filaChat.vault ? "vault" : "chat"
                                color: filaChat.elegido ? Tema.azul : Tema.tenue
                            }
                            Text {
                                Layout.fillWidth: true
                                text: filaChat.titulo
                                color: filaChat.elegido ? Tema.texto : Tema.suave
                                font.pixelSize: 13
                                font.weight: filaChat.elegido ? Font.DemiBold : Font.Normal
                                elide: Text.ElideRight
                            }
                            Text {
                                visible: !zonaChat.containsMouse && !borrarChat.hovered
                                text: filaChat.fecha
                                color: Tema.tenue
                                font.pixelSize: 11
                            }
                            BotonIcono {
                                id: borrarChat
                                visible: zonaChat.containsMouse || hovered
                                icono: "basura"
                                tam: 30
                                ayuda: "Borrar este chat"
                                onClicked: dialogo.confirmar("¿Borrar «" + filaChat.titulo + "»?",
                                    "Se borra este chat para siempre. Tus notas del vault no se tocan.", "Borrar", true,
                                    function () { chat.borrar(filaChat.clave) })
                            }
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        width: parent.width - 20
                        visible: listaChats.count === 0
                        text: "Aquí van a salir tus chats."
                        color: Tema.tenue
                        font.pixelSize: 12
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                    }
                }
            }
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Tema.borde }
        }

        // --- conversación ---
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // barra de arriba: modelo, vault y guardar
            RowLayout {
                Layout.fillWidth: true
                Layout.margins: 14
                Layout.bottomMargin: 6
                spacing: 10
                Selector {
                    id: selector
                    visible: modelos.hayInstalados
                    Layout.preferredWidth: 280
                    model: modelos.nombresChat
                    onActivated: (i) => chat.modelo = modelos.nombresChat[i]
                    Component.onCompleted: raiz.sincronizarModelo()
                    onModelChanged: Qt.callLater(raiz.sincronizarModelo)
                }
                Boton {
                    visible: !modelos.hayInstalados
                    icono: "bajar"
                    text: "Bajar un modelo"
                    onClicked: raiz.irA("modelos")
                }
                Text {
                    Layout.fillWidth: true
                    text: chat.titulo
                    color: Tema.tenue
                    font.pixelSize: 13
                    elide: Text.ElideRight
                    horizontalAlignment: Text.AlignHCenter
                }
                Rectangle {
                    implicitWidth: filaVault.implicitWidth + 24
                    implicitHeight: 38
                    radius: 19
                    color: chat.usarVault ? Tema.realceFuerte : Tema.realce
                    border.width: 1
                    border.color: chat.usarVault ? Tema.bordeHover : Tema.borde
                    RowLayout {
                        id: filaVault
                        anchors.centerIn: parent
                        spacing: 8
                        Icono { Layout.preferredWidth: 16; Layout.preferredHeight: 16; nombre: "vault"; color: chat.usarVault ? Tema.azul : Tema.suave }
                        Text { text: "Usar mi vault"; color: Tema.texto; font.pixelSize: 13 }
                        Interruptor { id: interruptorVault; Layout.preferredWidth: 40; Layout.preferredHeight: 22; checked: chat.usarVault; onToggled: chat.usarVault = checked }
                    }
                    MouseArea {
                        anchors.fill: parent
                        z: -1
                        cursorShape: Qt.PointingHandCursor
                        onClicked: chat.usarVault = !chat.usarVault
                    }
                    ToolTip.visible: zonaAyudaVault.hovered
                    ToolTip.delay: 600
                    ToolTip.text: "La IA lee tus notas para contestar y te dice de cuáles sacó la respuesta"
                    HoverHandler { id: zonaAyudaVault }
                }
                BotonIcono {
                    icono: "guardar"
                    tam: 38
                    enabled: mensajes.count > 0 && !chat.generando
                    ayuda: "Guardar en el vault (como nota)"
                    onClicked: chat.guardarEnVault()
                }
            }

            // mensajes
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                // Sin mensajes: la bienvenida
                ColumnLayout {
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 60, 520)
                    visible: mensajes.count === 0
                    spacing: 14
                    Image {
                        Layout.alignment: Qt.AlignHCenter
                        source: "logo.svg"
                        sourceSize.width: 144; sourceSize.height: 144
                        Layout.preferredWidth: 72; Layout.preferredHeight: 72
                    }
                    Text {
                        Layout.fillWidth: true
                        text: "¿En qué te ayudo?"
                        color: Tema.texto
                        font.pixelSize: 26
                        font.weight: Font.Bold
                        horizontalAlignment: Text.AlignHCenter
                    }
                    Text {
                        Layout.fillWidth: true
                        text: !modelos.hayInstalados
                            ? "Para empezar, baja un modelo: en Modelos te digo cuáles le caben a tu gráfica."
                            : chat.usarVault
                                ? "Pregúntale a tu vault: la IA busca en tus notas y te dice de cuáles sacó la respuesta."
                                : "Todo corre en tu PC: lo que escribes no sale a internet."
                        color: Tema.suave
                        font.pixelSize: 14
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.Wrap
                    }
                    Boton {
                        Layout.alignment: Qt.AlignHCenter
                        visible: !modelos.hayInstalados
                        grande: true
                        icono: "bajar"
                        text: "Bajar un modelo"
                        onClicked: raiz.irA("modelos")
                    }
                }

                ListView {
                    id: mensajes
                    property bool pegado: true      // seguir el final mientras escribe
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    clip: true
                    spacing: 6
                    model: chat.listaMensajes
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    header: Item { height: 10; width: 1 }
                    footer: Item { height: 16; width: 1 }
                    onMovementEnded: pegado = atYEnd
                    onContentHeightChanged: if (pegado) Qt.callLater(positionViewAtEnd)
                    onCountChanged: if (pegado) Qt.callLater(positionViewAtEnd)
                    WheelHandler { onWheel: (e) => { if (e.angleDelta.y > 0) mensajes.pegado = false } }

                    delegate: Item {
                        id: msg
                        required property int index
                        required property string rol
                        required property string contenido
                        required property string pensamiento
                        required property string error
                        required property string modelo
                        required property string datos
                        required property var citas
                        required property bool escribiendo
                        required property bool pensando
                        readonly property bool esUltimo: index === mensajes.count - 1
                        readonly property real ancho: Math.min(mensajes.width - 32, 820)
                        width: mensajes.width
                        height: (rol === "user" ? burbuja.height : respuesta.implicitHeight) + 12

                        // --- lo que escribiste ---
                        Rectangle {
                            id: burbuja
                            visible: msg.rol === "user"
                            anchors.right: parent.right
                            anchors.rightMargin: Math.max(16, (mensajes.width - msg.ancho) / 2)
                            y: 6
                            width: Math.min(textoUsuario.implicitWidth + 32, msg.ancho * 0.78)
                            height: textoUsuario.implicitHeight + 22
                            radius: 20
                            color: Tema.realceFuerte
                            TextEdit {
                                id: textoUsuario
                                anchors.fill: parent
                                anchors.margins: 11
                                anchors.leftMargin: 16
                                anchors.rightMargin: 16
                                text: msg.rol === "user" ? msg.contenido : ""
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.Wrap
                                color: Tema.texto
                                selectionColor: Tema.azul
                                selectedTextColor: Tema.sobreAcento
                                font.pixelSize: 14
                            }
                        }

                        // --- lo que contesta la IA ---
                        RowLayout {
                            id: respuesta
                            visible: msg.rol === "assistant"
                            x: Math.max(16, (mensajes.width - msg.ancho) / 2)
                            y: 6
                            width: msg.ancho
                            spacing: 14
                            Rectangle {
                                Layout.alignment: Qt.AlignTop
                                Layout.preferredWidth: 32; Layout.preferredHeight: 32
                                radius: 16
                                color: Tema.realce
                                border.width: 1
                                border.color: Tema.borde
                                Icono {
                                    anchors.centerIn: parent
                                    width: 17; height: 17
                                    nombre: "chispa"
                                    color: Tema.violeta
                                    SequentialAnimation on opacity {
                                        running: msg.escribiendo
                                        loops: Animation.Infinite
                                        alwaysRunToEnd: true
                                        NumberAnimation { from: 1; to: 0.35; duration: 550 }
                                        NumberAnimation { from: 0.35; to: 1; duration: 550 }
                                    }
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 8

                                // El pensamiento, plegable
                                Rectangle {
                                    id: cajaPensar
                                    property bool abierta: msg.pensando
                                    visible: msg.pensamiento !== ""
                                    Layout.fillWidth: true
                                    implicitHeight: columnaPensar.implicitHeight + 16
                                    radius: 14
                                    color: Tema.tarjeta
                                    border.width: 1
                                    border.color: Tema.borde
                                    Connections { target: msg; function onPensandoChanged() { cajaPensar.abierta = msg.pensando } }
                                    ColumnLayout {
                                        id: columnaPensar
                                        anchors.left: parent.left
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        anchors.margins: 8
                                        anchors.leftMargin: 12
                                        spacing: 6
                                        RowLayout {
                                            Layout.fillWidth: true
                                            spacing: 8
                                            Icono { Layout.preferredWidth: 15; Layout.preferredHeight: 15; nombre: "pensar"; color: Tema.violeta }
                                            Text {
                                                Layout.fillWidth: true
                                                text: msg.pensando ? "Pensando…" : "Lo que pensó"
                                                color: Tema.suave
                                                font.pixelSize: 12
                                                font.weight: Font.DemiBold
                                            }
                                            Icono {
                                                Layout.preferredWidth: 14; Layout.preferredHeight: 14
                                                nombre: "derecha"
                                                rotation: cajaPensar.abierta ? 90 : 0
                                                color: Tema.tenue
                                                Behavior on rotation { NumberAnimation { duration: 120 } }
                                            }
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            visible: cajaPensar.abierta
                                            text: msg.pensamiento
                                            color: Tema.tenue
                                            font.pixelSize: 12
                                            font.italic: true
                                            wrapMode: Text.Wrap
                                            textFormat: Text.PlainText
                                        }
                                    }
                                    MouseArea {
                                        anchors.left: parent.left
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        height: 32
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: cajaPensar.abierta = !cajaPensar.abierta
                                    }
                                }

                                // «Escribiendo…» mientras no llega nada
                                Row {
                                    visible: msg.escribiendo && msg.contenido === "" && !msg.pensando
                                    spacing: 5
                                    Layout.topMargin: 8
                                    Repeater {
                                        model: 3
                                        Rectangle {
                                            required property int index
                                            width: 7; height: 7; radius: 3.5
                                            color: Tema.tenue
                                            SequentialAnimation on opacity {
                                                running: msg.escribiendo
                                                loops: Animation.Infinite
                                                PauseAnimation { duration: index * 160 }
                                                NumberAnimation { from: 0.25; to: 1; duration: 300 }
                                                NumberAnimation { from: 1; to: 0.25; duration: 300 }
                                                PauseAnimation { duration: (2 - index) * 160 }
                                            }
                                        }
                                    }
                                }

                                Markdown {
                                    Layout.fillWidth: true
                                    visible: msg.contenido !== ""
                                    markdown: msg.rol === "assistant" ? boveda.vista(msg.contenido) : ""
                                    onNotaPedida: (nombre) => raiz.abrirNota(nombre, "")
                                }

                                Rectangle {
                                    visible: msg.error !== ""
                                    Layout.fillWidth: true
                                    implicitHeight: textoError.implicitHeight + 20
                                    radius: 14
                                    color: Tema.peligroFondo
                                    border.width: 1
                                    border.color: Tema.peligroBorde
                                    RowLayout {
                                        anchors.fill: parent
                                        anchors.margins: 10
                                        spacing: 10
                                        Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; Layout.alignment: Qt.AlignTop; nombre: "alerta"; color: Tema.peligro }
                                        Text {
                                            id: textoError
                                            Layout.fillWidth: true
                                            text: msg.error
                                            color: Tema.texto
                                            font.pixelSize: 13
                                            wrapMode: Text.Wrap
                                        }
                                    }
                                }

                                // Las notas que usó, como chips que abren la nota
                                Flow {
                                    visible: msg.citas.length > 0
                                    Layout.fillWidth: true
                                    spacing: 6
                                    Text {
                                        text: "De tus notas:"
                                        color: Tema.tenue
                                        font.pixelSize: 12
                                        height: 28
                                        verticalAlignment: Text.AlignVCenter
                                    }
                                    Repeater {
                                        model: msg.citas
                                        Chip {
                                            required property var modelData
                                            icono: "nota"
                                            texto: modelData.titulo
                                            onPulsado: raiz.abrirNota(modelData.titulo, modelData.ruta)
                                        }
                                    }
                                }

                                // Copiar y regenerar (en la última respuesta)
                                RowLayout {
                                    visible: !msg.escribiendo && (msg.contenido !== "" || msg.error !== "")
                                    spacing: 2
                                    BotonIcono {
                                        icono: "copiar"
                                        tam: 30
                                        ayuda: "Copiar la respuesta"
                                        visible: msg.contenido !== ""
                                        onClicked: ia.copiar(msg.contenido)
                                    }
                                    BotonIcono {
                                        icono: "girar"
                                        tam: 30
                                        ayuda: "Volver a contestar"
                                        visible: msg.esUltimo && !chat.generando
                                        onClicked: { mensajes.pegado = true; chat.regenerar() }
                                    }
                                    Text {
                                        Layout.leftMargin: 6
                                        text: msg.modelo + (msg.datos !== "" ? " · " + msg.datos : "")
                                        color: Tema.tenue
                                        font.pixelSize: 11
                                    }
                                }
                            }
                        }
                    }
                }

                // Volver al final
                BotonIcono {
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.bottom: parent.bottom
                    anchors.bottomMargin: 10
                    visible: !mensajes.pegado && mensajes.count > 0 && !mensajes.atYEnd
                    icono: "derecha"
                    rotation: 90
                    tam: 36
                    ayuda: "Ir al final"
                    background: Rectangle { radius: 18; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
                    onClicked: { mensajes.pegado = true; mensajes.positionViewAtEnd() }
                }
            }

            // caja para escribir
            Rectangle {
                Layout.fillWidth: true
                Layout.maximumWidth: 860
                Layout.alignment: Qt.AlignHCenter
                Layout.leftMargin: 20
                Layout.rightMargin: 20
                Layout.bottomMargin: 18
                implicitHeight: Math.min(220, Math.max(56, caja.implicitHeight + 18))
                radius: 26
                color: Tema.tarjeta
                border.width: caja.activeFocus ? 2 : 1
                border.color: caja.activeFocus ? Tema.bordeHover : Tema.borde
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 18
                    anchors.rightMargin: 9
                    anchors.topMargin: 9
                    anchors.bottomMargin: 9
                    spacing: 8
                    ScrollView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        TextArea {
                            id: caja
                            placeholderText: !modelos.hayInstalados ? "Primero baja un modelo…"
                                : chat.usarVault ? "Pregúntale a tu vault…" : "Escribe un mensaje…"
                            placeholderTextColor: Tema.tenue
                            color: Tema.texto
                            selectionColor: Tema.realceFuerte
                            selectedTextColor: Tema.texto
                            font.pixelSize: 14
                            wrapMode: TextArea.Wrap
                            verticalAlignment: TextArea.AlignVCenter
                            background: null
                            topPadding: 8
                            bottomPadding: 8
                            Keys.onPressed: (e) => {
                                if ((e.key === Qt.Key_Return || e.key === Qt.Key_Enter) && !(e.modifiers & Qt.ShiftModifier)) {
                                    e.accepted = true
                                    raiz.mandar()
                                }
                            }
                        }
                    }
                    BotonIcono {
                        Layout.alignment: Qt.AlignBottom
                        icono: chat.generando ? "detener" : "enviar"
                        relleno: chat.generando || caja.text.trim() !== ""
                        enabled: chat.generando || caja.text.trim() !== ""
                        tam: 38
                        ayuda: chat.generando ? "Detener" : "Mandar (Enter)"
                        onClicked: chat.generando ? chat.detener() : raiz.mandar()
                    }
                }
            }
        }
    }
}
