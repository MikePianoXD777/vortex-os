// 2.4: lo que entra o sale con desenfoque de movimiento: se desvanece, se acomoda (escala o desliza) y se
// desenfoca mientras se mueve. La capa con el desenfoque solo existe mientras dura la animación: quieto no
// cuesta nada. `visible` sigue a `mostrado` (al terminar de salir se oculta).
import QtQuick
import QtQuick.Effects

Item {
    id: raiz
    property bool mostrado: true
    property real desde: 0.97       // escala al entrar
    property real deLado: 0          // px de desplazamiento al entrar (0 = sin deslizar)
    property int duracion: 320
    readonly property bool moviendose: animOpacidad.running
    opacity: mostrado ? 1 : 0
    visible: opacity > 0
    scale: mostrado ? 1 : desde
    transform: Translate { x: raiz.mostrado ? 0 : raiz.deLado; Behavior on x { NumberAnimation { duration: raiz.duracion; easing.type: Easing.OutCubic } } }
    Behavior on opacity { NumberAnimation { id: animOpacidad; duration: raiz.duracion; easing.type: Easing.OutCubic } }
    Behavior on scale { NumberAnimation { duration: raiz.duracion; easing.type: Easing.OutCubic } }
    layer.enabled: moviendose
    layer.effect: MultiEffect {
        blurEnabled: true
        blurMax: 32
        blur: 1 - raiz.opacity
    }
}
