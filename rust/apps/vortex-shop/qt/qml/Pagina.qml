// Página que se desplaza: una columna con márgenes que se achican en ventanas angostas.
// Lo que se ponga dentro va a la columna.
import QtQuick
import QtQuick.Layouts
import QtQuick.Controls.Basic

Flickable {
    id: raiz
    default property alias cuerpo: columna.data
    property alias spacing: columna.spacing
    readonly property int margen: width < 1000 ? 24 : 36

    contentWidth: width
    contentHeight: columna.implicitHeight + 60
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    flickableDirection: Flickable.VerticalFlick
    ScrollBar.vertical: Barra {}

    ColumnLayout {
        id: columna
        x: raiz.margen
        y: 20
        width: raiz.width - raiz.margen * 2
        spacing: 28
    }
}
