// Entrada de la barra lateral: ícono, texto y, si hace falta, un contador.
import QtQuick

Rectangle {
    id: raiz
    property string icono: ""
    property string texto: ""
    property int contador: 0
    property bool seleccionado: false
    signal clicked()
    implicitHeight: 40
    radius: 14
    color: seleccionado ? Tema.barraSel : zona.containsMouse ? Tema.barraHover : "transparent"
    Behavior on color { ColorAnimation { duration: 100 } }

    Rectangle {   // marca de la entrada activa
        visible: raiz.seleccionado
        x: 0
        anchors.verticalCenter: parent.verticalCenter
        width: 3
        height: 18
        radius: 2
        color: Tema.azul
    }
    Icono {
        id: icon
        x: 14
        anchors.verticalCenter: parent.verticalCenter
        width: 19
        height: 19
        nombre: raiz.icono
        color: raiz.seleccionado ? Tema.azul : Tema.suave
    }
    Text {
        anchors.left: icon.right
        anchors.leftMargin: 12
        anchors.right: insignia.visible ? insignia.left : parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        text: raiz.texto
        color: raiz.seleccionado ? Tema.texto : Tema.suave
        font.pixelSize: 14
        font.weight: raiz.seleccionado ? Font.DemiBold : Font.Normal
        elide: Text.ElideRight
    }
    Rectangle {
        id: insignia
        visible: raiz.contador > 0
        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(22, cuenta.implicitWidth + 12)
        height: 22
        radius: 11
        color: Tema.violeta
        Text {
            id: cuenta
            anchors.centerIn: parent
            text: raiz.contador > 99 ? "99+" : raiz.contador
            color: Tema.sobreAcento
            font.pixelSize: 12
            font.weight: Font.Bold
        }
    }
    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: raiz.clicked()
    }
    Accessible.role: Accessible.Button
    Accessible.name: texto
}
