// La ventana de Vortexine Task Manager: a la izquierda la navegación y un vistazo en vivo (CPU,
// memoria, tarjeta de video), al centro la página. Ctrl+F busca, Supr finaliza lo elegido.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 1180
    height: 780
    minimumWidth: 900
    minimumHeight: 560
    visible: true
    title: "Vortexine Task Manager"
    color: Tema.fondo

    property string pagina: "procesos"   // procesos | rendimiento | servicios
    readonly property var rend: tm.rendimiento

    onActiveChanged: tm.enfocada = active
    onPaginaChanged: tm.verProcesos = pagina === "procesos"

    Connections {
        target: tm
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
        function onAviso(texto) { aviso.mostrar(texto) }
    }

    Shortcut { sequence: "Ctrl+1"; onActivated: ventana.pagina = "procesos" }
    Shortcut { sequence: "Ctrl+2"; onActivated: ventana.pagina = "rendimiento" }
    Shortcut { sequence: "Ctrl+3"; onActivated: ventana.pagina = "servicios" }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }

    component Navegar: Rectangle {
        id: nav
        property string texto: ""
        property string icono: ""
        property string atajo: ""
        property bool elegido: false
        signal pulsado()
        Layout.fillWidth: true
        implicitHeight: 42
        radius: 12
        color: elegido ? Tema.barraSel : zonaNav.containsMouse ? Tema.barraHover : "transparent"
        Behavior on color { ColorAnimation { duration: 120 } }
        Rectangle {   // la rayita del elegido
            width: 3; height: nav.elegido ? 18 : 0; radius: 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.azul
            Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 12
            spacing: 12
            Icono { Layout.preferredWidth: 19; Layout.preferredHeight: 19; nombre: nav.icono; color: nav.elegido ? Tema.azul : Tema.suave }
            Text {
                Layout.fillWidth: true
                text: nav.texto
                color: nav.elegido ? Tema.texto : Tema.suave
                font.pixelSize: 14
                font.weight: nav.elegido ? Font.DemiBold : Font.Normal
            }
            Text { text: nav.atajo; color: Tema.tenue; font.pixelSize: 11; opacity: zonaNav.containsMouse ? 1 : 0 }
        }
        MouseArea { id: zonaNav; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: nav.pulsado() }
    }

    // Medidor del lateral: etiqueta, valor y una barrita que se llena.
    component Medidor: ColumnLayout {
        id: med
        property string texto: ""
        property string valor: ""
        property real fraccion: 0
        property color tinta: Tema.azul
        Layout.fillWidth: true
        spacing: 6
        RowLayout {
            Layout.fillWidth: true
            Text { Layout.fillWidth: true; text: med.texto; color: Tema.suave; font.pixelSize: 12 }
            Text { text: med.valor; color: Tema.texto; font.pixelSize: 12; font.weight: Font.DemiBold; font.features: { "tnum": 1 } }
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 6
            radius: 3
            color: Tema.relleno
            Rectangle {
                height: parent.height
                radius: 3
                width: parent.width * Math.min(1, Math.max(0.02, med.fraccion))
                color: med.fraccion > 0.9 ? Tema.peligro : med.tinta
                Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.OutCubic } }
            }
        }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // --- lateral ---
        Rectangle {
            Layout.preferredWidth: Tema.anchoLateral
            Layout.fillHeight: true
            color: Tema.barra
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 4
                RowLayout {
                    Layout.leftMargin: 6
                    Layout.bottomMargin: 18
                    Layout.topMargin: 6
                    spacing: 10
                    Image { source: "logo.svg"; sourceSize.width: 64; sourceSize.height: 64; Layout.preferredWidth: 34; Layout.preferredHeight: 34 }
                    ColumnLayout {
                        spacing: 0
                        Text { text: "Vortexine"; color: Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 1.2; font.capitalization: Font.AllUppercase }
                        Text { text: "Task Manager"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.Bold }
                    }
                }
                Navegar { texto: "Procesos"; icono: "procesos"; atajo: "Ctrl+1"; elegido: ventana.pagina === "procesos"; onPulsado: ventana.pagina = "procesos" }
                Navegar { texto: "Rendimiento"; icono: "rendimiento"; atajo: "Ctrl+2"; elegido: ventana.pagina === "rendimiento"; onPulsado: ventana.pagina = "rendimiento" }
                Navegar { texto: "Servicios"; icono: "servicios"; atajo: "Ctrl+3"; elegido: ventana.pagina === "servicios"; onPulsado: ventana.pagina = "servicios" }

                Item { Layout.fillHeight: true }

                // Vistazo en vivo
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: vistazo.implicitHeight + 28
                    radius: Tema.radioTarjeta - 4
                    color: Tema.tarjeta
                    border.width: 1
                    border.color: Tema.borde
                    ColumnLayout {
                        id: vistazo
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 12
                        Medidor {
                            texto: "CPU"
                            valor: Fmt.pct(ventana.rend.cpu ? ventana.rend.cpu.uso : 0)
                            fraccion: ventana.rend.cpu ? ventana.rend.cpu.uso / 100 : 0
                        }
                        Medidor {
                            texto: "Memoria"
                            valor: ventana.rend.mem ? Fmt.bytes(ventana.rend.mem.usada) : ""
                            fraccion: ventana.rend.mem ? ventana.rend.mem.uso / 100 : 0
                            tinta: Tema.violeta
                        }
                        Repeater {
                            model: ventana.rend.gpus || []
                            Medidor {
                                required property var modelData
                                visible: modelData.uso >= 0
                                texto: "GPU"
                                valor: Fmt.pct(modelData.uso)
                                fraccion: modelData.uso / 100
                                tinta: Tema.exito
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            text: (ventana.rend.cpu ? ventana.rend.cpu.procesos : 0) + " procesos · encendida hace "
                                  + Fmt.tiempo(ventana.rend.cpu ? ventana.rend.cpu.encendido : 0)
                            color: Tema.tenue
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                        }
                    }
                }
            }
        }

        // --- página ---
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: ["procesos", "rendimiento", "servicios"].indexOf(ventana.pagina)
            PaginaProcesos { id: paginaProcesos; activa: ventana.pagina === "procesos" }
            PaginaRendimiento { activa: ventana.pagina === "rendimiento" }
            PaginaServicios { activa: ventana.pagina === "servicios" }
        }
    }

    // Aviso abajo al centro (errores de una acción: «No se dio el permiso.»…)
    Rectangle {
        id: aviso
        property string texto: ""
        function mostrar(t) { texto = t; opacity = 1; ocultar.restart() }
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 24
        anchors.horizontalCenterOffset: Tema.anchoLateral / 2
        width: Math.min(560, textoAviso.implicitWidth + 64)
        height: Math.max(46, textoAviso.implicitHeight + 24)
        radius: 23
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.peligroBorde
        opacity: 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 180 } }
        Row {
            anchors.centerIn: parent
            spacing: 10
            Icono { width: 18; height: 18; nombre: "alerta"; color: Tema.peligro; anchors.verticalCenter: parent.verticalCenter }
            Text {
                id: textoAviso
                width: Math.min(implicitWidth, 480)
                text: aviso.texto
                color: Tema.texto
                font.pixelSize: 13
                wrapMode: Text.Wrap
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        Timer { id: ocultar; interval: 4500; onTriggered: aviso.opacity = 0 }
    }
}
