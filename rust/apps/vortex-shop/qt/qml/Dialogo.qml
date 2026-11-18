// Cuadro de confirmación sobre toda la ventana. Se abre con `abierto: true` y avisa con
// `aceptado()` o `cancelado()`; Esc cancela.
import QtQuick
import QtQuick.Controls.Basic

Item {
    id: raiz
    property bool abierto: false
    property string titulo: ""
    property string texto: ""
    property string textoAceptar: "Aceptar"
    property string textoCancelar: "Cancelar"
    property string tipoAceptar: "primario"
    signal aceptado()
    signal cancelado()

    anchors.fill: parent
    visible: opacity > 0
    opacity: abierto ? 1 : 0
    z: 100
    Behavior on opacity { NumberAnimation { duration: 140 } }

    Rectangle {   // velo
        anchors.fill: parent
        color: Tema.velo
    }
    MouseArea {   // lo de atrás no recibe clics
        anchors.fill: parent
        onClicked: raiz.cancelado()
    }
    Shortcut {
        sequence: "Esc"
        enabled: raiz.abierto
        onActivated: raiz.cancelado()
    }

    Rectangle {
        anchors.centerIn: parent
        width: Math.min(raiz.width - 48, 440)
        height: contenido.implicitHeight + 56
        radius: Tema.radioGrande
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.bordeCampo
        scale: raiz.abierto ? 1 : 0.96
        Behavior on scale { NumberAnimation { duration: 140 } }
        MouseArea { anchors.fill: parent }   // clics dentro no cierran

        Column {
            id: contenido
            x: 28
            y: 28
            width: parent.width - 56
            spacing: 12
            Text {
                width: parent.width
                text: raiz.titulo
                color: Tema.texto
                font.pixelSize: 20
                font.weight: Font.DemiBold
                wrapMode: Text.WordWrap
            }
            Text {
                width: parent.width
                text: raiz.texto
                color: Tema.suave
                font.pixelSize: 14
                lineHeight: 1.25
                wrapMode: Text.WordWrap
            }
            Row {
                anchors.right: parent.right
                spacing: 10
                topPadding: 10
                Boton {
                    text: raiz.textoCancelar
                    tipo: "secundario"
                    grande: true
                    onClicked: raiz.cancelado()
                }
                Boton {
                    text: raiz.textoAceptar
                    tipo: raiz.tipoAceptar
                    grande: true
                    onClicked: raiz.aceptado()
                }
            }
        }
    }
}
