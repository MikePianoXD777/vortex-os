import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Pipewire
import "../../../"

// 4.5: el aviso de micrófono y cámara en uso, junto al reloj. Sale solo mientras alguna app te oye o te ve
// (Discord, Zoom, Meet, OBS, el dictado…) y al pasar el mouse dice cuál. El micrófono se ve por PipeWire:
// un enlace de un micrófono de verdad (no el «monitor» de las bocinas, que usa el visualizador) a una app.
// La cámara, por PipeWire y por el gestor, que vigila /dev/videoN y deja en $XDG_RUNTIME_DIR/vortex/camara
// quién la tiene abierta (Discord, el navegador y Zoom la abren directo).
Row {
    id: root

    property real tam: 12
    property real tamIcono: 14

    spacing: tam * 0.35
    visible: microfono.length > 0 || camara.length > 0

    function esFuente(n, clase) {
        if (!n || n.isStream || n.isSink) return false;
        return (n.type & clase) && (n.type & PwNodeType.Source);
    }

    function nombreDe(n) {
        if (!n) return "";
        let p = n.properties || {};
        return p["application.name"] || n.description || n.name || "";
    }

    // Las apps que reciben de un micrófono o de una cámara por PipeWire.
    readonly property var enlaces: Pipewire.links ? (Pipewire.links.values || []) : []
    readonly property var streamsMic: enlaces.filter(l => l && l.target && l.target.isStream && esFuente(l.source, PwNodeType.Audio)).map(l => l.target)
    readonly property var streamsCam: enlaces.filter(l => l && l.target && l.target.isStream && esFuente(l.source, PwNodeType.Video)).map(l => l.target)

    // Para leer «application.name» hay que atar los nodos.
    PwObjectTracker { objects: root.streamsMic.concat(root.streamsCam) }

    function unicos(lista) {
        let r = [];
        for (let i = 0; i < lista.length; i++) {
            let n = (lista[i] || "").trim();
            if (n && r.indexOf(n) === -1) r.push(n);
        }
        return r;
    }

    readonly property var microfono: unicos(streamsMic.map(nombreDe))

    FileView {
        id: archivoCamara
        path: (Quickshell.env("XDG_RUNTIME_DIR") || "") + "/vortex/camara"
        watchChanges: true
        blockWrites: true
        printErrors: false
        onFileChanged: reload()
    }
    readonly property var camara: unicos(streamsCam.map(nombreDe).concat((archivoCamara.text() || "").split("\n")))

    // Al pasar el mouse se despliega quién es.
    property bool abierto: false
    readonly property string quien: microfono.concat(camara).join(", ")

    Rectangle {
        id: fondo
        anchors.verticalCenter: parent.verticalCenter
        height: root.tamIcono + root.tam * 0.7
        width: fila.implicitWidth + root.tam * 1.1
        radius: height / 2
        color: Qt.rgba(ThemeBackend.red.r, ThemeBackend.red.g, ThemeBackend.red.b, 0.16)
        border.width: 1
        border.color: Qt.rgba(ThemeBackend.red.r, ThemeBackend.red.g, ThemeBackend.red.b, 0.45)

        Row {
            id: fila
            anchors.centerIn: parent
            spacing: root.tam * 0.35

            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.microfono.length > 0
                text: "󰍬"
                font.family: ThemeBackend.iconFont
                font.pixelSize: root.tamIcono
                color: ThemeBackend.red
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: root.camara.length > 0
                text: "󰄀"
                font.family: ThemeBackend.iconFont
                font.pixelSize: root.tamIcono
                color: ThemeBackend.red
            }
        }

        MouseArea {
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.NoButton // el clic sigue abriendo el calendario
            onEntered: root.abierto = true
            onExited: root.abierto = false
        }
    }

    // Quién es, en una etiqueta que no mueve la barra.
    PopupWindow {
        visible: root.abierto && root.visible && root.quien !== ""
        anchor.item: fondo
        anchor.rect.x: fondo.width / 2 - implicitWidth / 2
        anchor.rect.y: fondo.height + root.tam * 0.6
        implicitWidth: etiqueta.implicitWidth + root.tam * 1.6
        implicitHeight: etiqueta.implicitHeight + root.tam * 0.9
        color: "transparent"

        Rectangle {
            anchors.fill: parent
            radius: root.tam * 0.6
            color: ThemeBackend.base
            border.width: 1
            border.color: Qt.rgba(ThemeBackend.red.r, ThemeBackend.red.g, ThemeBackend.red.b, 0.45)

            Text {
                id: etiqueta
                anchors.centerIn: parent
                text: (root.microfono.length ? "󰍬 " + root.microfono.join(", ") : "")
                    + (root.microfono.length && root.camara.length ? "   " : "")
                    + (root.camara.length ? "󰄀 " + root.camara.join(", ") : "")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: root.tam
                color: ThemeBackend.text
            }
        }
    }
}
