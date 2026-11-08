// Lista desplegable con los colores de Vortexine's (la de Qt Basic sale gris claro).
import QtQuick
import QtQuick.Controls.Basic

ComboBox {
    id: raiz
    implicitHeight: 38
    hoverEnabled: true
    font.pixelSize: 13
    contentItem: Text {
        leftPadding: 14
        rightPadding: 30
        text: raiz.displayText
        color: Tema.texto
        font: raiz.font
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    indicator: Icono {
        x: raiz.width - width - 12
        y: (raiz.height - height) / 2
        width: 16; height: 16
        nombre: "derecha"
        rotation: raiz.popup.visible ? -90 : 90
        color: Tema.suave
    }
    background: Rectangle {
        radius: 19
        color: raiz.hovered ? Tema.realceFuerte : Tema.campo
        border.width: raiz.visualFocus ? 2 : 1
        border.color: raiz.visualFocus ? Tema.violeta : Tema.bordeCampo
    }
    delegate: ItemDelegate {
        id: opcion
        required property int index
        required property var modelData
        width: raiz.width - 8
        x: 4
        height: 36
        hoverEnabled: true
        contentItem: Text {
            text: raiz.textRole ? opcion.modelData[raiz.textRole] : (raiz.textoDe ? raiz.textoDe(opcion.modelData) : opcion.modelData)
            color: raiz.currentIndex === opcion.index ? Tema.azul : Tema.texto
            font.pixelSize: 13
            font.weight: raiz.currentIndex === opcion.index ? Font.DemiBold : Font.Normal
            verticalAlignment: Text.AlignVCenter
            leftPadding: 6
        }
        background: Rectangle { radius: 10; color: opcion.hovered ? Tema.realce : "transparent" }
    }
    property var textoDe: null
    popup: Popup {
        y: raiz.height + 4
        width: raiz.width
        padding: 4
        implicitHeight: Math.min(contentItem.implicitHeight + 8, 320)
        contentItem: ListView {
            clip: true
            implicitHeight: contentHeight
            model: raiz.popup.visible ? raiz.delegateModel : null
            currentIndex: raiz.highlightedIndex
        }
        background: Rectangle { radius: 14; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
    }
}
