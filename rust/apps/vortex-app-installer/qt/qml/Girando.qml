// Indicador de «trabajando»: un arco que gira.
import QtQuick

Icono {
    id: raiz
    width: 28
    height: 28
    nombre: "girar"
    color: Tema.azul
    trazo: 2.4
    RotationAnimator on rotation {
        running: raiz.visible
        from: 0
        to: 360
        duration: 900
        loops: Animation.Infinite
    }
}
