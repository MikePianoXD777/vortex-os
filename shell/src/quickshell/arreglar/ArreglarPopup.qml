import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.9: «Algo no funciona» (lanzador → «Algo no funciona», o «vortex arreglar»). Al abrirse
// revisa sonido, micrófono, internet, Bluetooth, disco, hora y actualizaciones trabadas
// (vortex-arreglar revisar) y, en lo que falla, ofrece «Arreglar» (vortex-arreglar arreglar ID).
// Después de cada arreglo vuelve a revisar, para que se vea si quedó.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("arreglar." + clave, porDefecto);
    }

    property var revisiones: []
    property bool cargado: false
    property string arreglando: ""
    property string mensaje: ""

    readonly property int problemas: revisiones.filter(r => r.estado === "mal").length
    readonly property int avisos: revisiones.filter(r => r.estado === "aviso").length

    function revisar() {
        if (revisarProc.running) return;
        revisarProc.running = true;
    }

    function arreglar(id) {
        if (arreglarProc.running) return;
        window.arreglando = id;
        window.mensaje = "";
        arreglarProc.command = ["vortex-arreglar", "arreglar", id];
        arreglarProc.running = true;
    }

    Process {
        id: revisarProc
        command: ["vortex-arreglar", "revisar"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.revisiones = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.revisiones = [];
                }
                window.cargado = true;
            }
        }
    }

    Process {
        id: arreglarProc
        stdout: StdioCollector {
            onStreamFinished: {
                let txt = this.text.trim();
                if (txt !== "") window.mensaje = txt;
            }
        }
        onExited: function(exitCode, exitStatus) {
            window.arreglando = "";
            window.revisar();
        }
    }

    property var exitCallback: null

    function startExit(onFinished) {
        introAnim.stop();
        exitCallback = onFinished;
        exitAnim.restart();
    }

    function resetAndPlayIntro() {
        exitAnim.stop();
        introMain = 0;
        introAnim.restart();
    }

    onVisibleChanged: {
        if (visible) {
            forceActiveFocus();
            mensaje = "";
            revisar();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        revisar();
        if (visible) resetAndPlayIntro();
    }

    property real introMain: 0

    NumberAnimation {
        id: introAnim
        target: window; property: "introMain"; from: 0; to: 1.0; duration: 700; easing.type: Easing.OutExpo
    }

    SequentialAnimation {
        id: exitAnim
        NumberAnimation { target: window; property: "introMain"; to: 0.0; duration: 180; easing.type: Easing.InQuart }
        ScriptAction {
            script: {
                if (window.exitCallback) {
                    let cb = window.exitCallback;
                    window.exitCallback = null;
                    cb();
                }
            }
        }
    }

    function colorDe(estado) {
        return estado === "bien" ? ThemeBackend.green : estado === "aviso" ? ThemeBackend.peach : ThemeBackend.red;
    }

    function iconoDe(id) {
        let iconos = { sonido: "󰕾", microfono: "󰍬", internet: "󰖩", bluetooth: "󰂯", disco: "󰋊",
                       hora: "󰥔", pacman: "󰏗", servicios: "󰒓" };
        return iconos[id] || "󰋼";
    }

    Item {
        anchors.fill: parent
        scale: 0.95 + (0.05 * introMain)
        opacity: introMain

        Rectangle {
            anchors.fill: parent
            radius: ThemeBackend.borderRadius
            color: Qt.alpha(ThemeBackend.base, ThemeBackend.elementOpacity)
            clip: true

            WallpaperBlur {
                z: -1
                anchors.fill: parent
                topLeftRadius: parent.topLeftRadius
                topRightRadius: parent.topRightRadius
                bottomLeftRadius: parent.bottomLeftRadius
                bottomRightRadius: parent.bottomRightRadius
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: window.s(24)
            spacing: window.s(16)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Rectangle {
                    Layout.preferredWidth: window.s(52)
                    Layout.preferredHeight: window.s(52)
                    radius: width / 2
                    color: Qt.alpha(!window.cargado ? ThemeBackend.blue
                                    : window.problemas > 0 ? ThemeBackend.red
                                    : window.avisos > 0 ? ThemeBackend.peach : ThemeBackend.green, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: !window.cargado || revisarProc.running ? "󰔟" : window.problemas > 0 ? "󰀦" : "󰄬"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(26)
                        color: !window.cargado ? ThemeBackend.blue
                               : window.problemas > 0 ? ThemeBackend.red
                               : window.avisos > 0 ? ThemeBackend.peach : ThemeBackend.green
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Something's not working?")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: !window.cargado ? window.t("checking", "Checking your computer…")
                            : window.problemas === 0 ? (window.avisos === 0 ? window.t("all_good", "Everything looks fine.")
                                                        : window.t("all_good_notes", "Everything works; there are a few notes."))
                            : window.problemas === 1 ? window.t("one_problem", "I found 1 problem.")
                            : window.t("problems", "I found {n} problems.").replace("{n}", window.problemas)
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }
                }
            }

            Flickable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentHeight: lista.height
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: lista
                    width: parent.width
                    spacing: window.s(8)

                    Repeater {
                        model: window.revisiones
                        delegate: Rectangle {
                            id: fila
                            required property var modelData
                            readonly property var r: modelData
                            readonly property bool sePuede: r.arreglo !== "" && r.estado !== "bien"
                            width: lista.width
                            height: contenido.implicitHeight + window.s(24)
                            radius: window.s(12)
                            color: r.estado === "bien" ? Qt.alpha(ThemeBackend.surface0, 0.4)
                                                       : Qt.alpha(window.colorDe(r.estado), 0.10)
                            border.width: r.estado === "bien" ? 0 : 1
                            border.color: Qt.alpha(window.colorDe(r.estado), 0.4)

                            RowLayout {
                                id: contenido
                                x: window.s(14)
                                y: window.s(12)
                                width: parent.width - window.s(28)
                                spacing: window.s(12)

                                Text {
                                    text: window.iconoDe(fila.r.id)
                                    font.family: ThemeBackend.iconFont
                                    font.pixelSize: window.s(22)
                                    color: window.colorDe(fila.r.estado)
                                    Layout.alignment: Qt.AlignTop
                                }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: window.s(2)
                                    Text {
                                        text: fila.r.titulo
                                        font.family: ThemeBackend.fontFamily
                                        font.pixelSize: window.s(14)
                                        font.weight: Font.Bold
                                        color: ThemeBackend.text
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        wrapMode: Text.WordWrap
                                        text: fila.r.detalle
                                        font.family: ThemeBackend.fontFamily
                                        font.pixelSize: window.s(12)
                                        color: ThemeBackend.subtext1
                                    }
                                }
                                Text {
                                    visible: fila.r.estado === "bien"
                                    text: "󰄬"
                                    font.family: ThemeBackend.iconFont
                                    font.pixelSize: window.s(18)
                                    color: ThemeBackend.green
                                }
                                Rectangle {
                                    visible: fila.sePuede
                                    Layout.preferredHeight: window.s(34)
                                    Layout.preferredWidth: etiqueta.implicitWidth + window.s(28)
                                    radius: height / 2
                                    opacity: window.arreglando !== "" && window.arreglando !== fila.r.arreglo ? 0.5 : 1
                                    color: zona.containsMouse ? Qt.lighter(window.colorDe(fila.r.estado), 1.1) : window.colorDe(fila.r.estado)
                                    Text {
                                        id: etiqueta
                                        anchors.centerIn: parent
                                        text: window.arreglando === fila.r.arreglo ? window.t("fixing", "Fixing…")
                                            : fila.r.arreglo === "portal" || fila.r.arreglo === "elegir-red" ? window.t("open", "Open")
                                            : window.t("fix", "Fix it")
                                        font.family: ThemeBackend.fontFamily
                                        font.pixelSize: window.s(12)
                                        font.weight: Font.Bold
                                        color: ThemeBackend.crust
                                    }
                                    MouseArea {
                                        id: zona
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        enabled: window.arreglando === ""
                                        onClicked: window.arreglar(fila.r.arreglo)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {
                visible: window.mensaje !== ""
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                text: window.mensaje
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                color: ThemeBackend.green
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(10)

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: window.t("hint", "If it keeps failing, restarting the computer fixes most things.")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: ThemeBackend.overlay1
                }
                Rectangle {
                    Layout.preferredHeight: window.s(36)
                    Layout.preferredWidth: otraVez.implicitWidth + window.s(32)
                    radius: height / 2
                    color: zonaOtra.containsMouse ? ThemeBackend.surface2 : ThemeBackend.surface1
                    Text {
                        id: otraVez
                        anchors.centerIn: parent
                        text: "󰑐  " + window.t("again", "Check again")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        font.weight: Font.DemiBold
                        color: ThemeBackend.text
                    }
                    MouseArea {
                        id: zonaOtra
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { window.mensaje = ""; window.revisar(); }
                    }
                }
            }
        }
    }
}
