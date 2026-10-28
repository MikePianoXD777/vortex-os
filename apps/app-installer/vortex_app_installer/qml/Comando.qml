// Un comando para copiar y pegar en una terminal (lo que necesita contraseña de administrador: la app
// nunca lo corre por su cuenta), con su botón «Copiar».
import QtQuick
import QtQuick.Layouts

Rectangle {
    id: raiz
    property string titulo: ""
    property string texto: ""
    property string comando: ""
    implicitHeight: col.implicitHeight + 32
    radius: Tema.radioCampo
    color: Tema.tarjeta
    border.width: 1
    border.color: Tema.borde

    ColumnLayout {
        id: col
        x: 16
        y: 16
        width: parent.width - 32
        spacing: 10
        Text {
            visible: raiz.titulo !== ""
            Layout.fillWidth: true
            text: raiz.titulo
            color: Tema.texto
            font.pixelSize: 15
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }
        Text {
            visible: raiz.texto !== ""
            Layout.fillWidth: true
            text: raiz.texto
            color: Tema.suave
            font.pixelSize: 13
            wrapMode: Text.WordWrap
        }
        RowLayout {
            Layout.fillWidth: true
            spacing: 10
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: linea.implicitHeight + 18
                radius: 12
                color: Tema.campo
                border.width: 1
                border.color: Tema.bordeCampo
                TextEdit {
                    id: linea
                    x: 12
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 24
                    text: raiz.comando
                    readOnly: true
                    selectByMouse: true
                    color: Tema.texto
                    selectionColor: Tema.bordeHover
                    font.family: "monospace"
                    font.pixelSize: 13
                    wrapMode: TextEdit.WrapAnywhere
                }
            }
            Boton {
                id: copiar
                property bool hecho: false
                text: hecho ? instalador.t.copiado : instalador.t.boton_copiar
                icono: hecho ? "visto" : "copiar"
                tipo: "secundario"
                onClicked: { instalador.copiar(raiz.comando); hecho = true; volver.restart() }
                Timer { id: volver; interval: 2000; onTriggered: copiar.hecho = false }
            }
        }
    }
}
