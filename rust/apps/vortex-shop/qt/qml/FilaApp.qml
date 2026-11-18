// Fila de una app para las listas de Instaladas y Actualizaciones.
import QtQuick

Rectangle {
    id: raiz
    property var app: ({})
    property string modo: "instalada"     // «instalada» o «actualizacion»
    implicitHeight: 80
    radius: Tema.radioTarjeta
    color: zona.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
    border.width: 1
    border.color: zona.containsMouse ? Tema.bordeHover : Tema.borde
    Behavior on color { ColorAnimation { duration: 120 } }

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
        anchors.verticalCenter: parent.verticalCenter
        tam: 48
        url: raiz.app.icono
        nombre: raiz.app.nombre
    }


    Column {
        anchors.left: icono.right
        anchors.leftMargin: 14
        anchors.right: acciones.left
        anchors.rightMargin: 14
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2
        Row {
            width: parent.width
            spacing: 6
            Text {
                id: nombre
                width: Math.min(implicitWidth, parent.width - (sello.visible ? sello.width + parent.spacing : 0))
                text: raiz.app.nombre
                color: Tema.texto
                font.pixelSize: 15
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Sello {
                id: sello
                visible: raiz.app.verificado === true
                tam: 14
                anchors.verticalCenter: nombre.verticalCenter
            }
        }
        Text {
            width: parent.width
            text: raiz.app.resumen !== "" ? raiz.app.resumen : raiz.app.desarrollador
            visible: text !== ""
            color: Tema.suave
            font.pixelSize: 13
            maximumLineCount: 1
            elide: Text.ElideRight
        }
    }

    AccionApp {
        id: acciones
        anchors.right: parent.right
        anchors.rightMargin: 16
        anchors.verticalCenter: parent.verticalCenter
        width: enCurso ? 230 : implicitWidth
        appId: raiz.app.id
        nombre: raiz.app.nombre
        modo: raiz.modo
    }
}
