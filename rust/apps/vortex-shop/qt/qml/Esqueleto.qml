// Tarjeta «de mentira» que late mientras se carga el catálogo.
import QtQuick

Rectangle {
    id: raiz
    implicitHeight: 152
    radius: Tema.radioTarjeta
    color: Tema.tarjeta
    border.width: 1
    border.color: Tema.borde

    SequentialAnimation on opacity {
        running: raiz.visible
        loops: Animation.Infinite
        NumberAnimation { from: 1; to: 0.55; duration: 800; easing.type: Easing.InOutSine }
        NumberAnimation { from: 0.55; to: 1; duration: 800; easing.type: Easing.InOutSine }
    }

    Rectangle { x: 16; y: 16; width: 60; height: 60; radius: 14; color: Tema.relleno }
    Rectangle { x: 90; y: 20; width: Math.min(150, raiz.width - 110); height: 14; radius: 7; color: Tema.relleno }
    Rectangle { x: 90; y: 42; width: Math.min(90, raiz.width - 110); height: 10; radius: 5; color: Tema.realce }
    Rectangle { x: 90; y: 60; width: Math.min(190, raiz.width - 110); height: 10; radius: 5; color: Tema.realce }
    Rectangle { x: 16; y: raiz.height - 50; width: 100; height: 36; radius: 18; color: Tema.relleno }
}
