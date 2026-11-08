// Botón en píldora. tipo: «primario» (azul), «secundario», «peligro» o «fantasma».
import QtQuick
import QtQuick.Controls.Basic

Button {
    id: raiz
    property string tipo: "primario"
    property string icono: ""
    property bool grande: false

    readonly property color colorTexto: !enabled ? Tema.tenue
        : tipo === "primario" ? Tema.sobreAcento
        : tipo === "peligro" ? Tema.peligro
        : tipo === "fantasma" ? Tema.suave : Tema.texto

    implicitHeight: grande ? 46 : 36
    implicitWidth: Math.max(grande ? 120 : 84, contentItem.implicitWidth + leftPadding + rightPadding)
    leftPadding: grande ? 24 : 16
    rightPadding: leftPadding
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    font.pixelSize: grande ? 15 : 13
    font.weight: Font.DemiBold

    background: Rectangle {
        radius: height / 2
        color: {
            if (raiz.tipo === "primario")
                return !raiz.enabled ? Tema.borde : raiz.down ? Tema.azulPresion : raiz.hovered ? Tema.azulHover : Tema.azul
            if (raiz.tipo === "peligro")
                return raiz.hovered && raiz.enabled ? Tema.peligroFondo : "transparent"
            if (raiz.tipo === "fantasma")
                return raiz.hovered && raiz.enabled ? Tema.realce : "transparent"
            return raiz.hovered && raiz.enabled ? Tema.realceFuerte : Tema.realce
        }
        border.width: raiz.visualFocus ? 2 : (raiz.tipo === "secundario" || raiz.tipo === "peligro" ? 1 : 0)
        border.color: raiz.visualFocus ? Tema.violeta
            : raiz.tipo === "peligro" ? Tema.peligroBorde : Tema.bordeCampo
        Behavior on color { ColorAnimation { duration: 120 } }
    }

    contentItem: Item {
        implicitWidth: fila.implicitWidth
        implicitHeight: fila.implicitHeight
        Row {
            id: fila
            anchors.centerIn: parent
            spacing: 8
            Icono {
                anchors.verticalCenter: parent.verticalCenter
                visible: raiz.icono !== ""
                nombre: raiz.icono
                color: raiz.colorTexto
                width: raiz.grande ? 20 : 17
                height: width
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: raiz.text
                color: raiz.colorTexto
                font: raiz.font
                elide: Text.ElideRight
            }
        }
    }
}
