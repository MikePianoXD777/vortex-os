// Ventanita para confirmar («¿Borrar este modelo?») o pedir un nombre («Nueva carpeta»).
// pedir(): con campo de texto; confirmar(): solo los botones; preguntar(): sí o no, y las dos respuestas cuentan.
// Al aceptar sale «aceptado(texto)».
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: raiz
    property string titulo: ""
    property string mensaje: ""
    property string textoAceptar: "Aceptar"
    property bool peligro: false
    property bool conCampo: false
    property var alAceptar: null
    property string textoCancelar: "Cancelar"
    property var alRechazar: null
    signal aceptado(string texto)

    function confirmar(t, m, boton, peligroso, accion) {
        titulo = t; mensaje = m; textoAceptar = boton || "Aceptar"; peligro = !!peligroso
        conCampo = false; alAceptar = accion || null
        textoCancelar = "Cancelar"; alRechazar = null
        open()
    }
    function preguntar(t, m, si, no, accionSi, accionNo) {
        titulo = t; mensaje = m; textoAceptar = si; peligro = false
        conCampo = false; alAceptar = accionSi
        textoCancelar = no; alRechazar = accionNo
        open()
    }
    function _rechazar() {
        close()
        if (alRechazar) alRechazar()
    }
    function pedir(t, m, valor, boton, accion) {
        titulo = t; mensaje = m || ""; textoAceptar = boton || "Aceptar"; peligro = false
        conCampo = true; alAceptar = accion || null
        textoCancelar = "Cancelar"; alRechazar = null
        campo.text = valor || ""
        open()
        campo.forceActiveFocus()
        campo.selectAll()
    }
    function _aceptar() {
        if (conCampo && campo.text.trim() === "")
            return
        var t = campo.text.trim()
        close()
        if (alAceptar) alAceptar(t)
        aceptado(t)
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 440
    modal: true
    focus: true
    padding: 24
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Overlay.modal: Rectangle { color: Tema.velo }
    background: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
    enter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 120 } }
    exit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 } }

    contentItem: ColumnLayout {
        spacing: 14
        Text {
            Layout.fillWidth: true
            text: raiz.titulo
            color: Tema.texto
            font.pixelSize: 17
            font.weight: Font.Bold
            wrapMode: Text.Wrap
        }
        Text {
            Layout.fillWidth: true
            visible: raiz.mensaje !== ""
            text: raiz.mensaje
            color: Tema.suave
            font.pixelSize: 13
            wrapMode: Text.Wrap
            lineHeight: 1.2
        }
        CampoTexto {
            id: campo
            Layout.fillWidth: true
            visible: raiz.conCampo
            onAccepted: raiz._aceptar()
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            spacing: 10
            Item { Layout.fillWidth: true }
            Boton { tipo: "fantasma"; text: raiz.textoCancelar; onClicked: raiz._rechazar() }
            Boton {
                tipo: raiz.peligro ? "peligro" : "primario"
                text: raiz.textoAceptar
                enabled: !raiz.conCampo || campo.text.trim() !== ""
                onClicked: raiz._aceptar()
            }
        }
    }
}
