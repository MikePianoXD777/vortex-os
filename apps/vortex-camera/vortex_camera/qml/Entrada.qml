// Campo de texto en píldora con su etiqueta arriba (los formularios). «secreto» lo esconde y le pone
// el ojito para verlo.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ColumnLayout {
    id: raiz
    property string etiqueta: ""
    property string ayuda: ""
    property bool secreto: false
    property bool mostrar: false
    property alias text: campo.text
    property alias placeholderText: campo.placeholderText
    property alias validator: campo.validator
    property alias campo: campo
    signal aceptado()
    spacing: 6

    Text {
        visible: raiz.etiqueta !== ""
        text: raiz.etiqueta
        color: Tema.suave
        font.pixelSize: 12
        font.weight: Font.DemiBold
    }
    TextField {
        id: campo
        Layout.fillWidth: true
        implicitHeight: 40
        leftPadding: 16
        rightPadding: raiz.secreto ? 44 : 16
        color: Tema.texto
        placeholderTextColor: Tema.tenue
        selectionColor: Tema.realceFuerte
        selectedTextColor: Tema.texto
        font.pixelSize: 14
        echoMode: raiz.secreto && !raiz.mostrar ? TextInput.Password : TextInput.Normal
        onAccepted: raiz.aceptado()
        inputMethodHints: raiz.secreto ? Qt.ImhSensitiveData | Qt.ImhNoPredictiveText : Qt.ImhNone
        background: Rectangle {
            radius: height / 2
            color: Tema.campo
            border.width: campo.activeFocus ? 2 : 1
            border.color: campo.activeFocus ? Tema.azul : campo.hovered ? Tema.bordeHover : Tema.bordeCampo
            Behavior on border.color { ColorAnimation { duration: 120 } }
        }
        BotonIcono {
            visible: raiz.secreto
            anchors.right: parent.right
            anchors.rightMargin: 5
            anchors.verticalCenter: parent.verticalCenter
            tam: 30
            icono: raiz.mostrar ? "ojoNo" : "ojo"
            ayuda: raiz.mostrar ? "Esconder" : "Ver el código"
            onClicked: raiz.mostrar = !raiz.mostrar
        }
    }
    Text {
        visible: raiz.ayuda !== ""
        Layout.fillWidth: true
        text: raiz.ayuda
        color: Tema.tenue
        font.pixelSize: 11
        wrapMode: Text.Wrap
    }
}
