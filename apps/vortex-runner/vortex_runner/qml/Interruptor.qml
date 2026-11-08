// Interruptor con los colores de Vortexine's.
import QtQuick
import QtQuick.Controls.Basic

Switch {
    id: raiz
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    padding: 0
    spacing: 0
    indicator: Rectangle {
        implicitWidth: 40
        implicitHeight: 22
        x: raiz.leftPadding
        y: (raiz.height - height) / 2
        radius: height / 2
        opacity: raiz.enabled ? 1 : 0.4
        color: raiz.checked ? Tema.azul : Tema.campo
        border.width: raiz.checked ? 0 : 1
        border.color: raiz.visualFocus ? Tema.violeta : Tema.bordeCampo
        Behavior on color { ColorAnimation { duration: 140 } }
        Rectangle {
            width: 16; height: 16; radius: 8
            y: 3
            x: raiz.checked ? parent.width - width - 3 : 3
            color: raiz.checked ? Tema.sobreAcento : Tema.suave
            Behavior on x { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
        }
    }
    contentItem: Item {}
}
