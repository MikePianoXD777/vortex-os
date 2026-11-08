// Entrada de la barra lateral: ícono, texto y, si hace falta, un contador. Una entrada «destacada»
// (la categoría IA) lleva el ícono en violeta y la etiqueta «Nuevo».
import QtQuick

Rectangle {
    id: raiz
    property string icono: ""
    property string texto: ""
    property int contador: 0
    property bool seleccionado: false
    property bool destacado: false
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
        color: raiz.seleccionado ? Tema.azul : raiz.destacado ? Tema.violeta : Tema.suave
    }
    Text {
        anchors.left: icon.right
        anchors.leftMargin: 12
        anchors.right: insignia.visible ? insignia.left : nuevo.visible ? nuevo.left : parent.right
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
    Rectangle {
        id: nuevo
        visible: raiz.destacado && !insignia.visible
        anchors.right: parent.right
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        width: etiquetaNuevo.implicitWidth + 14
        height: 20
        radius: 10
        color: "transparent"
        border.width: 1
        border.color: Tema.violeta
        Text {
            id: etiquetaNuevo
            anchors.centerIn: parent
            text: "Nuevo"
            color: Tema.violeta
            font.pixelSize: 11
            font.weight: Font.DemiBold
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
