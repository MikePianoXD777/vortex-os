// Encabezado de sección (etiqueta violeta + título + subtítulo, y lo que se quiera a la derecha)
// seguido de su contenido.
import QtQuick
import QtQuick.Layouts

ColumnLayout {
    id: raiz
    property string etiqueta: ""
    property string titulo: ""
    property string subtitulo: ""
    property int tamTitulo: 20
    default property alias cuerpo: contenido.data
    property alias derecha: filaDerecha.data
    spacing: 14

    RowLayout {
        Layout.fillWidth: true
        spacing: 12
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 3
            Text {
                visible: raiz.etiqueta !== ""
                text: raiz.etiqueta
                color: Tema.violeta
                font.pixelSize: 12
                font.weight: Font.DemiBold
                font.letterSpacing: 1.4
                font.capitalization: Font.AllUppercase
            }
            Text {
                Layout.fillWidth: true
                text: raiz.titulo
                color: Tema.texto
                font.pixelSize: raiz.tamTitulo
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Text {
                Layout.fillWidth: true
                visible: raiz.subtitulo !== ""
                text: raiz.subtitulo
                color: Tema.suave
                font.pixelSize: 14
                wrapMode: Text.WordWrap
            }
        }
        Row {
            id: filaDerecha
            Layout.alignment: Qt.AlignBottom
            spacing: 8
        }
    }
    ColumnLayout {
        id: contenido
        Layout.fillWidth: true
        spacing: 12
    }
}
