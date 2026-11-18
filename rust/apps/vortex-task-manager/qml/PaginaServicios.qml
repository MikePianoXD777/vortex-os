// Servicios: los tuyos y los del sistema (systemd). Los que fallaron salen primero, en rojo. Cada uno
// con su interruptor de «Al iniciar» y botones para iniciar, detener o reiniciar; los del sistema
// piden tu contraseña (polkit).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: pagina
    property bool activa: false
    property bool deUsuario: true
    property string filtro: ""
    readonly property var todos: deUsuario ? tm.serviciosUsuario : tm.serviciosSistema
    readonly property var lista: {
        var f = filtro.trim().toLowerCase()
        return f ? todos.filter(s => s.nombre.toLowerCase().indexOf(f) >= 0 || s.descripcion.toLowerCase().indexOf(f) >= 0) : todos
    }
    readonly property int fallaron: todos.filter(s => s.activo === "failed").length

    onActivaChanged: if (activa) tm.cargarServicios()
    Timer { interval: 5000; repeat: true; running: pagina.activa; onTriggered: tm.cargarServicios() }
    Shortcut { enabled: pagina.activa; sequence: "Ctrl+F"; onActivated: buscador.forceActiveFocus() }

    function estado(s) {
        if (s.activo === "failed") return ["Falló", Tema.peligro]
        if (s.activo === "active") return [s.sub === "exited" ? "Hecho" : "Corriendo", Tema.exito]
        if (s.activo === "activating") return ["Arrancando", Tema.azul]
        if (s.activo === "deactivating") return ["Deteniéndose", Tema.tenue]
        return ["Detenido", Tema.tenue]
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        anchors.topMargin: 20
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: false
            spacing: 12
            Text { text: "Servicios"; color: Tema.texto; font.pixelSize: 26; font.weight: Font.Bold }
            Rectangle {   // Tuyos / Del sistema
                Layout.leftMargin: 8
                implicitWidth: filaTipo.implicitWidth + 8
                implicitHeight: 38
                radius: 19
                color: Tema.campo
                Row {
                    id: filaTipo
                    anchors.centerIn: parent
                    spacing: 2
                    Repeater {
                        model: [["Tuyos", true], ["Del sistema", false]]
                        Rectangle {
                            required property var modelData
                            readonly property bool sel: pagina.deUsuario === modelData[1]
                            width: textoTipo.implicitWidth + 30
                            height: 32
                            radius: 16
                            color: sel ? Tema.azul : zonaTipo.containsMouse ? Tema.realce : "transparent"
                            Text { id: textoTipo; anchors.centerIn: parent; text: modelData[0]; color: sel ? Tema.sobreAcento : Tema.suave; font.pixelSize: 13; font.weight: Font.DemiBold }
                            MouseArea { id: zonaTipo; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: pagina.deUsuario = modelData[1] }
                        }
                    }
                }
            }
            Rectangle {
                visible: pagina.fallaron > 0
                implicitWidth: textoFallo.implicitWidth + 22
                implicitHeight: 28
                radius: 14
                color: Tema.peligroFondo
                border.width: 1
                border.color: Tema.peligroBorde
                Text { id: textoFallo; anchors.centerIn: parent; text: pagina.fallaron + (pagina.fallaron === 1 ? " falló" : " fallaron"); color: Tema.peligro; font.pixelSize: 12; font.weight: Font.DemiBold }
            }
            Item { Layout.fillWidth: true }
            Campo {
                id: buscador
                Layout.preferredWidth: 300
                placeholderText: "Buscar servicio  (Ctrl+F)"
                onTextChanged: pagina.filtro = text
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            clip: true

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 6
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    Layout.preferredHeight: 44
                    Layout.leftMargin: 16
                    Layout.rightMargin: 16
                    spacing: 12
                    Text { Layout.fillWidth: true; text: "Nombre"; color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold }
                    Text { Layout.preferredWidth: 110; text: "Estado"; color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold }
                    Text { Layout.preferredWidth: 80; text: "Al iniciar"; color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold }
                    Item { Layout.preferredWidth: 76 }
                }
                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Tema.borde }

                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: pagina.lista
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { contentItem: Rectangle { implicitWidth: 6; radius: 3; color: Tema.bordeCampo } }

                    Text {
                        anchors.centerIn: parent
                        visible: lista.count === 0
                        text: tm.cargandoServicios && pagina.todos.length === 0 ? "Leyendo servicios…"
                            : pagina.filtro ? "Ningún servicio coincide con «" + pagina.filtro + "»" : "No hay servicios"
                        color: Tema.tenue
                        font.pixelSize: 14
                    }

                    delegate: Item {
                        id: fila
                        required property var modelData
                        readonly property var s: modelData
                        readonly property var e: pagina.estado(s)
                        readonly property bool corre: s.activo === "active" || s.activo === "activating"
                        width: ListView.view.width - 10
                        height: 54
                        Rectangle {
                            anchors.fill: parent
                            anchors.margins: 1
                            radius: 10
                            color: zona.containsMouse ? Tema.realce : "transparent"
                        }
                        MouseArea { id: zona; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 16
                            anchors.rightMargin: 10
                            spacing: 12
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text { Layout.fillWidth: true; text: fila.s.nombre; color: fila.s.activo === "failed" ? Tema.peligro : Tema.texto; font.pixelSize: 13; font.weight: Font.Medium; elide: Text.ElideRight }
                                Text { Layout.fillWidth: true; text: fila.s.descripcion || " "; color: Tema.tenue; font.pixelSize: 12; elide: Text.ElideRight }
                            }
                            Row {
                                Layout.preferredWidth: 110
                                spacing: 8
                                Rectangle { width: 8; height: 8; radius: 4; color: fila.e[1]; anchors.verticalCenter: parent.verticalCenter }
                                Text { text: fila.e[0]; color: fila.e[1] === Tema.tenue ? Tema.tenue : Tema.texto; font.pixelSize: 12 }
                            }
                            Item {
                                Layout.preferredWidth: 80
                                Layout.fillHeight: true
                                Interruptor {
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: fila.s.cambiable
                                    checked: fila.s.alInicio === "enabled"
                                    onToggled: tm.servicio(fila.s.unidad, checked ? "enable" : "disable", pagina.deUsuario)
                                    ToolTip.visible: hovered
                                    ToolTip.delay: 600
                                    ToolTip.text: checked ? "Arranca solo al prender" : "No arranca solo"
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: !fila.s.cambiable
                                    text: fila.s.alInicio === "static" ? "Lo pide otro" : fila.s.alInicio === "masked" ? "Bloqueado" : "—"
                                    color: Tema.tenue
                                    font.pixelSize: 12
                                }
                            }
                            Row {
                                Layout.preferredWidth: 76
                                spacing: 4
                                BotonIcono {
                                    tam: 34
                                    icono: fila.corre ? "detener" : "play"
                                    ayuda: fila.corre ? "Detener" : "Iniciar"
                                    onClicked: tm.servicio(fila.s.unidad, fila.corre ? "stop" : "start", pagina.deUsuario)
                                }
                                BotonIcono {
                                    tam: 34
                                    icono: "girar"
                                    ayuda: "Reiniciar"
                                    onClicked: tm.servicio(fila.s.unidad, "restart", pagina.deUsuario)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
