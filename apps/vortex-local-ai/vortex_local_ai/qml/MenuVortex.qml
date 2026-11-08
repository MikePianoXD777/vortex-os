// Menú (clic derecho) con los colores de Vortexine's.
import QtQuick
import QtQuick.Controls.Basic

Menu {
    id: raiz
    padding: 6
    implicitWidth: 250
    delegate: OpcionMenu {}
    background: Rectangle {
        implicitWidth: 250
        radius: 16
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde
    }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 110 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 90 } }
}
