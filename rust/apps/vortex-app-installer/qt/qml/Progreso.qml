// Barra de progreso en píldora con el degradado de Vortexine's. `valor` de 0 a 1; si es negativo no se
// sabe cuánto falta y la barra va y viene.
import QtQuick

Rectangle {
    id: raiz
    property real valor: -1
    readonly property bool indeterminada: valor < 0
    implicitHeight: 10
    radius: height / 2
    color: Tema.realceFuerte   // el riel: que se vea contra la tarjeta
    clip: true

    Rectangle {
        id: relleno
        height: parent.height
        radius: height / 2
        width: raiz.indeterminada ? parent.width * 0.3 : Math.max(height, parent.width * Math.min(1, raiz.valor))
        x: raiz.indeterminada ? vaiven.x : 0
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: Tema.azul }
            GradientStop { position: 1; color: Tema.violeta }
        }
        Behavior on width { enabled: !raiz.indeterminada; NumberAnimation { duration: 180 } }
    }
    Item {
        id: vaiven
        SequentialAnimation on x {
            running: raiz.indeterminada && raiz.visible
            loops: Animation.Infinite
            NumberAnimation { from: -raiz.width * 0.3; to: raiz.width; duration: 1100; easing.type: Easing.InOutQuad }
        }
    }
}
