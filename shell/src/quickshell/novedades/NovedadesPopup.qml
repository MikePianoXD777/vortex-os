import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 2.7: lo nuevo de la versión que acabas de instalar (lanzador → «Novedades», o sale solo después
// de «vortex actualizar»). El texto son las notas de docs/notas-*.md: «vortex novedades texto --marcar»
// da las que no has visto (o las últimas, si ya las viste) y las marca como vistas.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("novedades." + clave, porDefecto);
    }

    property string notas: ""
    property bool cargado: false

    function cargar() {
        if (notasProc.running) return;
        notasProc.running = true;
    }

    Process {
        id: notasProc
        command: ["vortex", "novedades", "texto", "--marcar"]
        stdout: StdioCollector {
            onStreamFinished: {
                window.notas = this.text.trim();
                window.cargado = true;
            }
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
            cargar();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        if (visible) {
            cargar();
            resetAndPlayIntro();
        }
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
                    color: Qt.alpha(ThemeBackend.blue, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: "󰑣"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(26)
                        color: ThemeBackend.blue
                    }
                }
                Text {
                    Layout.fillWidth: true
                    text: window.t("title", "What's new")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(22)
                    font.weight: Font.Bold
                    color: ThemeBackend.text
                }
            }

            Flickable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentHeight: texto.implicitHeight
                boundsBehavior: Flickable.StopAtBounds

                Text {
                    id: texto
                    width: parent.width
                    wrapMode: Text.WordWrap
                    textFormat: Text.MarkdownText
                    text: window.notas !== "" ? window.notas
                        : window.cargado ? window.t("empty", "This version has no notes.") : ""
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(14)
                    color: ThemeBackend.subtext1
                    linkColor: ThemeBackend.blue
                    onLinkActivated: (enlace) => Quickshell.execDetached(["xdg-open", enlace])
                }
            }
        }
    }
}
