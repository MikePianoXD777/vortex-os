// El botón grande. Foto: círculo blanco. Video: punto rojo; grabando, un cuadro rojo.
import QtQuick
import QtQuick.Controls.Basic

AbstractButton {
    id: raiz
    property bool video: false
    property bool grabando: false
    readonly property color rojo: "#ff4d5e"
    implicitWidth: 76
    implicitHeight: 76
    hoverEnabled: true
    focusPolicy: Qt.NoFocus
    opacity: enabled ? 1 : 0.4
    Accessible.name: grabando ? "Detener" : video ? "Grabar" : "Tomar foto"
    background: Rectangle {
        radius: width / 2
        color: "transparent"
        border.width: 4
        border.color: "#ffffff"
    }
    contentItem: Item {
        Rectangle {
            anchors.centerIn: parent
            readonly property real lado: raiz.grabando ? 30 : raiz.pressed ? 50 : raiz.hovered ? 60 : 58
            width: lado; height: lado
            radius: raiz.grabando ? 7 : lado / 2
            color: raiz.video ? raiz.rojo : "#ffffff"
            Behavior on width { NumberAnimation { duration: 120 } }
            Behavior on height { NumberAnimation { duration: 120 } }
            Behavior on radius { NumberAnimation { duration: 120 } }
            Behavior on color { ColorAnimation { duration: 120 } }
        }
    }
}
