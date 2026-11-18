// Tarjeta de una app: ícono, nombre (con sello de verificado), desarrollador, resumen en una línea
// y, abajo, el botón Instalar / Abrir o su barra de progreso. Toda la tarjeta abre el detalle.
import QtQuick

Rectangle {
    id: raiz
    property var app: ({})
    implicitHeight: 152
    radius: Tema.radioTarjeta
    color: zona.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
    border.width: 1
    border.color: zona.containsMouse ? Tema.bordeHover : Tema.borde
    Behavior on color { ColorAnimation { duration: 120 } }
    Behavior on border.color { ColorAnimation { duration: 120 } }

    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: Nav.detalle(raiz.app.id)
    }

    IconoApp {
        id: icono
        x: 16
        y: 16
        tam: 60
        url: raiz.app.icono
        nombre: raiz.app.nombre
    }

    Column {
        id: textos
        anchors.left: icono.right
        anchors.leftMargin: 14
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.top: parent.top
        anchors.topMargin: 15
        spacing: 2
        Row {
            width: parent.width
            spacing: 6
            Text {
                id: nombre
                width: Math.min(implicitWidth, parent.width - (sello.visible ? sello.width + parent.spacing : 0))
                text: raiz.app.nombre
                color: Tema.texto
                font.pixelSize: 16
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Sello {
                id: sello
                visible: raiz.app.verificado === true
                anchors.verticalCenter: nombre.verticalCenter
            }
        }
        Text {
            width: parent.width
            text: raiz.app.desarrollador
            visible: text !== ""
            color: Tema.tenue
            font.pixelSize: 12
            elide: Text.ElideRight
        }
        Text {
            width: parent.width
            topPadding: 3
            text: raiz.app.resumen
            color: Tema.suave
            font.pixelSize: 13
            maximumLineCount: 1
            elide: Text.ElideRight
        }
    }

    AccionApp {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 16
        anchors.bottomMargin: 14
        appId: raiz.app.id
        nombre: raiz.app.nombre
        modo: "tarjeta"
    }
}
