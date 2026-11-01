// Botón redondo con solo un ícono (los del reproductor). «activo» lo pinta del acento.
import QtQuick
import QtQuick.Controls.Basic

Button {
    id: raiz
    property string icono: ""
    property bool activo: false
    property bool relleno: false        // el botón grande de play: círculo del color del texto
    property bool acento: false         // 2.4: con relleno, del color de acento (el verde de Spotify)
    property int tam: 36
    property string ayuda: ""
    implicitWidth: tam
    implicitHeight: tam
    // 2.4: al presionarlo se hunde y regresa con rebote
    scale: down ? 0.88 : 1
    Behavior on scale { NumberAnimation { duration: 180; easing.type: Easing.OutBack; easing.overshoot: 2.5 } }
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    ToolTip.visible: ayuda !== "" && hovered
    ToolTip.delay: 600
    ToolTip.text: ayuda
    background: Rectangle {
        radius: width / 2
        color: raiz.relleno && raiz.acento ? (raiz.down ? Tema.azulPresion : raiz.hovered ? Tema.azulHover : Tema.azul)
            : raiz.relleno ? (raiz.down ? Tema.suave : Tema.texto)
            : raiz.hovered && raiz.enabled ? Tema.realce : "transparent"
        scale: raiz.relleno && raiz.hovered ? 1.06 : 1
        Behavior on scale { NumberAnimation { duration: 120 } }
        Behavior on color { ColorAnimation { duration: 120 } }
    }
    contentItem: Item {
        Icono {
            anchors.centerIn: parent
            width: Math.round(raiz.tam * 0.5)
            height: width
            nombre: raiz.icono
            trazo: raiz.icono === "pausa" ? 3 : 2
            color: !raiz.enabled ? Tema.borde : raiz.relleno && raiz.acento ? Tema.sobreAcento : raiz.relleno ? Tema.fondo : raiz.activo ? Tema.azul
                : raiz.hovered ? Tema.texto : Tema.suave
        }
        Rectangle {   // el puntito de «activo», como en Spotify
            visible: raiz.activo && !raiz.relleno
            width: 4; height: 4; radius: 2
            color: Tema.azul
            anchors.horizontalCenter: parent.horizontalCenter
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 1
        }
    }
}
