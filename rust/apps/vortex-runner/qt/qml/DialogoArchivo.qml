// Abriste un .exe nuevo (doble clic en Thunar o lo arrastraste): ¿instalarlo o agregarlo y abrirlo?
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: raiz
    property string ruta: ""
    property string nombre: ""
    property bool instalador: false

    function abrir(r, n, inst) {
        ruta = r; nombre = n; instalador = inst
        open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 480
    modal: true
    focus: true
    padding: 26
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Overlay.modal: Rectangle { color: Tema.velo }
    background: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }

    contentItem: ColumnLayout {
        spacing: 10
        Text {
            Layout.fillWidth: true
            text: raiz.nombre
            color: Tema.texto
            font.pixelSize: 18
            font.weight: Font.Bold
            elide: Text.ElideRight
        }
        Text {
            Layout.fillWidth: true
            text: raiz.instalador
                ? "Parece un instalador. Lo instalo en su propio espacio y, al terminar, el programa queda en tu biblioteca."
                : "Lo agrego a tu biblioteca y lo abro. Si es un instalador, mejor elige «Instalar»."
            color: Tema.suave
            font.pixelSize: 13
            wrapMode: Text.Wrap
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            spacing: 10
            Item { Layout.fillWidth: true }
            Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: raiz.close() }
            Boton {
                tipo: raiz.instalador ? "secundario" : "primario"
                text: "Agregar y abrir"
                icono: raiz.instalador ? "" : "play"
                onClicked: {
                    var id = runner.agregar(raiz.ruta)
                    raiz.close()
                    if (id !== "") runner.jugar(id)
                }
            }
            Boton {
                tipo: raiz.instalador ? "primario" : "secundario"
                text: "Instalar"
                icono: raiz.instalador ? "bajar" : ""
                onClicked: { runner.instalar(raiz.ruta); raiz.close() }
            }
        }
    }
}
