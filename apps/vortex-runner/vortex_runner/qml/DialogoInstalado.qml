// Después de un instalador: «¿Cuál es el programa?», con los .exe nuevos (el más probable primero).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Popup {
    id: raiz
    property var opciones: []
    property int elegido: 0

    function abrir(lista) {
        opciones = lista
        elegido = 0
        open()
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 520
    modal: true
    focus: true
    padding: 26
    closePolicy: Popup.CloseOnEscape
    Overlay.modal: Rectangle { color: Tema.velo }
    background: Rectangle { radius: Tema.radioTarjeta; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }

    contentItem: ColumnLayout {
        spacing: 10
        Text { text: "¡Se instaló!"; color: Tema.texto; font.pixelSize: 18; font.weight: Font.Bold }
        Text {
            Layout.fillWidth: true
            text: "¿Cuál es el programa? Lo agrego a tu biblioteca."
            color: Tema.suave
            font.pixelSize: 13
            wrapMode: Text.Wrap
        }
        Repeater {
            model: raiz.opciones
            delegate: Rectangle {
                id: op
                required property var modelData
                required property int index
                Layout.fillWidth: true
                implicitHeight: 52
                radius: 14
                color: raiz.elegido === index ? Tema.realceFuerte : opMa.containsMouse ? Tema.realce : "transparent"
                border.width: raiz.elegido === index ? 1 : 0
                border.color: Tema.azul
                MouseArea { id: opMa; anchors.fill: parent; hoverEnabled: true; onClicked: raiz.elegido = op.index }
                ColumnLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 14
                    spacing: 0
                    Item { Layout.fillHeight: true }
                    Text { Layout.fillWidth: true; text: op.modelData.nombre; color: Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideRight }
                    Text { Layout.fillWidth: true; text: op.modelData.exe; color: Tema.tenue; font.pixelSize: 11; elide: Text.ElideMiddle }
                    Item { Layout.fillHeight: true }
                }
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 10
            spacing: 10
            Item { Layout.fillWidth: true }
            Boton { tipo: "fantasma"; text: "Ninguno"; onClicked: raiz.close() }
            Boton {
                text: "Agregar"
                icono: "mas"
                onClicked: {
                    var o = raiz.opciones[raiz.elegido]
                    runner.agregarInstalado(o.exe, o.nombre)
                    raiz.close()
                }
            }
        }
    }
}
