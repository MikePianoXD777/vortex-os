// Campo de búsqueda en píldora, con lupa y botón para borrar.
import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: raiz
    implicitHeight: 38
    leftPadding: 40
    rightPadding: text ? 36 : 14
    color: Tema.texto
    placeholderTextColor: Tema.tenue
    selectionColor: Tema.realceFuerte
    selectedTextColor: Tema.texto
    font.pixelSize: 13
    background: Rectangle {
        radius: height / 2
        color: Tema.campo
        border.width: raiz.activeFocus ? 2 : 1
        border.color: raiz.activeFocus ? Tema.azul : raiz.hovered ? Tema.bordeHover : Tema.bordeCampo
        Behavior on border.color { ColorAnimation { duration: 120 } }
        Icono {
            anchors.left: parent.left
            anchors.leftMargin: 14
            anchors.verticalCenter: parent.verticalCenter
            width: 17; height: 17
            nombre: "buscar"
            color: Tema.tenue
        }
    }
    BotonIcono {
        visible: raiz.text !== ""
        anchors.right: parent.right
        anchors.rightMargin: 5
        anchors.verticalCenter: parent.verticalCenter
        icono: "cerrar"
        tam: 28
        onClicked: raiz.text = ""
    }
    Keys.onEscapePressed: raiz.text === "" ? raiz.focus = false : raiz.text = ""
}
