// Botón redondo sobre la imagen de la cámara: con velo oscuro para que se vea sobre cualquier cosa.
import QtQuick
import QtQuick.Controls.Basic

AbstractButton {
    id: raiz
    property string icono: ""
    property string ayuda: ""
    property string etiqueta: ""
    property bool elegido: false
    implicitWidth: 48
    implicitHeight: 48
    hoverEnabled: true
    focusPolicy: Qt.NoFocus
    opacity: enabled ? 1 : 0.4
    ToolTip.visible: hovered && ayuda !== ""
    ToolTip.text: ayuda
    ToolTip.delay: 500
    Accessible.name: ayuda
    background: Rectangle {
        radius: width / 2
        color: raiz.elegido ? Tema.azul : raiz.pressed ? Qt.rgba(1, 1, 1, 0.28) : raiz.hovered ? Qt.rgba(1, 1, 1, 0.2) : Qt.rgba(0, 0, 0, 0.5)
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    contentItem: Item {
        Icono {
            anchors.centerIn: parent
            width: 22; height: 22
            visible: raiz.etiqueta === ""
            nombre: raiz.icono
            color: raiz.elegido ? Tema.sobreAcento : "#ffffff"
        }
        Text {
            anchors.centerIn: parent
            visible: raiz.etiqueta !== ""
            text: raiz.etiqueta
            color: raiz.elegido ? Tema.sobreAcento : "#ffffff"
            font.pixelSize: 15
            font.weight: Font.DemiBold
        }
    }
}
