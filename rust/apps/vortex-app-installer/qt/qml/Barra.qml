// Barra de desplazamiento fina con los colores de la tienda.
import QtQuick
import QtQuick.Controls.Basic

ScrollBar {
    id: raiz
    policy: size < 1 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
    interactive: true
    implicitWidth: 10
    contentItem: Rectangle {
        implicitWidth: 6
        radius: 3
        color: raiz.pressed ? Tema.azul : raiz.hovered ? Tema.bordeHover : Tema.bordeCampo
        opacity: raiz.active || raiz.hovered ? 1 : 0.6
    }
    background: Item {}
}
