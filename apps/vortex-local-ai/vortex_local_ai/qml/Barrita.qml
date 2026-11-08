// Barra de progreso fina (bajar un modelo, actualizar la memoria). valor < 0: sin saber cuánto falta.
import QtQuick

Rectangle {
    id: raiz
    property real valor: 0
    property color tinta: Tema.azul
    implicitHeight: 6
    radius: height / 2
    color: Tema.relleno
    clip: true
    Rectangle {
        visible: raiz.valor >= 0
        height: parent.height
        radius: parent.radius
        width: parent.width * Math.min(1, Math.max(0.02, raiz.valor))
        color: raiz.tinta
        Behavior on width { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
    }
    Rectangle {   // sin porcentaje: un trozo que va y viene
        id: vaiven
        visible: raiz.valor < 0
        height: parent.height
        radius: parent.radius
        width: parent.width * 0.3
        color: raiz.tinta
        NumberAnimation on x {
            running: vaiven.visible
            loops: Animation.Infinite
            from: -vaiven.width
            to: raiz.width
            duration: 1200
        }
    }
}
