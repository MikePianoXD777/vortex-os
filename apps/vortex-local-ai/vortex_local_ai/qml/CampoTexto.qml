// Campo de texto de una línea (nombres de notas, carpetas…) con los colores de Vortexine's.
import QtQuick
import QtQuick.Controls.Basic

TextField {
    id: raiz
    implicitHeight: 40
    leftPadding: 14
    rightPadding: 14
    color: Tema.texto
    placeholderTextColor: Tema.tenue
    selectionColor: Tema.realceFuerte
    selectedTextColor: Tema.texto
    font.pixelSize: 14
    background: Rectangle {
        radius: 14
        color: Tema.campo
        border.width: raiz.activeFocus ? 2 : 1
        border.color: raiz.activeFocus ? Tema.azul : raiz.hovered ? Tema.bordeHover : Tema.bordeCampo
        Behavior on border.color { ColorAnimation { duration: 120 } }
    }
}
