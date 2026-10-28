import QtQuick
import Quickshell
import Quickshell.Io
import "../../../reusables"
import "../../../"
import "Html.js" as Html

// Vortex 1.10: widget «HTML». Muestra el archivo .html de su ruta (la guarda como imagePath, igual que el
// widget de imagen) y se vuelve a dibujar en cuanto lo guardas. Se crea y se edita en el lanzador →
// «Widgets en HTML». Qué HTML sirve y los datos que se llenan solos: Html.js.
Item {
    id: root
    anchors.fill: parent
    clip: true

    property real minWidth: 80
    property real minHeight: 40
    property real maxWidth: 4000
    property real maxHeight: 4000
    property real minAspect: 0.05
    property real maxAspect: 20.0

    property string imagePath: (typeof model !== "undefined" && model && model.wImagePath) ? model.wImagePath : (parent && parent.wImagePath ? parent.wImagePath : "")
    readonly property string ruta: imagePath.startsWith("file://") ? imagePath.substring(7) : imagePath

    property string fuente: ""
    property date ahora: new Date()
    readonly property bool conDatos: Html.tieneDatos(fuente)
    property string equipo: ""

    FileView {
        id: archivo
        path: root.ruta
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.fuente = text()
        onLoadFailed: root.fuente = ""
    }

    FileView {
        id: hostname
        path: "/etc/hostname"
        printErrors: false
        onLoaded: root.equipo = text().trim()
    }

    Timer {
        interval: 1000
        repeat: true
        running: root.visible && root.conDatos
        onTriggered: root.ahora = new Date()
    }

    Rectangle {
        anchors.fill: parent
        radius: ThemeBackend.borderRadius
        color: Qt.alpha(ThemeBackend.base, 0.55)
        border.width: 1
        border.color: Qt.alpha(ThemeBackend.surface1, 0.4)
    }

    Text {
        anchors.fill: parent
        anchors.margins: 14
        visible: root.ruta !== "" && root.fuente !== ""
        textFormat: Text.RichText
        wrapMode: Text.Wrap
        elide: Text.ElideRight
        color: ThemeBackend.text
        linkColor: ThemeBackend.blue
        font.family: ThemeBackend.fontFamily
        font.pixelSize: 14
        text: Html.llenar(root.fuente, root.ahora, Quickshell.env("USER"), root.equipo)
        onLinkActivated: function(link) { Qt.openUrlExternally(link); }

        HoverHandler {
            cursorShape: parent.hoveredLink !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
        }
    }

    Column {
        anchors.centerIn: parent
        width: parent.width - 28
        spacing: 6
        visible: root.ruta === "" || root.fuente === ""
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: "󰌝"
            font.family: ThemeBackend.iconFont
            font.pixelSize: Math.max(16, Math.min(root.width, root.height) * 0.25)
            color: ThemeBackend.subtext0
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            text: I18n.t("htmlwidgets.empty_widget", "Create it in the launcher → «HTML widgets»")
            font.family: ThemeBackend.fontFamily
            font.pixelSize: 12
            color: ThemeBackend.subtext0
        }
    }
}
