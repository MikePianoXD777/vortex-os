// Pastilla pequeña que se puede tocar: las notas citadas en el chat, las etiquetas, los filtros.
import QtQuick
import QtQuick.Layouts

Rectangle {
    id: raiz
    property string texto: ""
    property string icono: ""
    property bool elegido: false
    property bool tocable: true
    property color tinta: elegido ? Tema.sobreAcento : Tema.suave
    signal pulsado()
    implicitWidth: fila.implicitWidth + 22
    implicitHeight: 28
    radius: height / 2
    color: elegido ? Tema.azul : zona.containsMouse && tocable ? Tema.realceFuerte : Tema.realce
    border.width: elegido ? 0 : 1
    border.color: zona.containsMouse && tocable ? Tema.bordeHover : Tema.borde
    Behavior on color { ColorAnimation { duration: 120 } }
    RowLayout {
        id: fila
        anchors.centerIn: parent
        spacing: 6
        Icono {
            visible: raiz.icono !== ""
            Layout.preferredWidth: 14; Layout.preferredHeight: 14
            nombre: raiz.icono
            color: raiz.tinta
        }
        Text {
            text: raiz.texto
            color: raiz.tinta
            font.pixelSize: 12
            font.weight: raiz.elegido ? Font.DemiBold : Font.Medium
            elide: Text.ElideRight
            Layout.maximumWidth: 260
        }
    }
    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        enabled: raiz.tocable
        cursorShape: Qt.PointingHandCursor
        onClicked: raiz.pulsado()
    }
}
