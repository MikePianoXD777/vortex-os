import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 4.1: «Compañera de escritorio» (vortex-mascota, del paquete vortex-rust). El interruptor llama
// a «vortex mascota on|off» (on la deja para los próximos inicios de sesión) y el estado lo dice
// «vortex mascota estado» («encendida …» o «apagada …»). Viene apagada. Mismo patrón que
// GeneralFpsSection.qml. Si todavía no está vortex-mascota (falta «vortex actualizar»), se dice en la
// descripción y el interruptor queda quieto.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    property bool mascotaOn: false
    property bool available: true

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    Process {
        id: stateProc
        command: ["bash", "-c", "vortex mascota estado 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                const t = this.text.trim();
                section.available = t.startsWith("encendida") || t.startsWith("apagada");
                section.mascotaOn = t.startsWith("encendida");
                mascotaToggle.checked = section.mascotaOn;
            }
        }
    }

    Process {
        id: toggleProc
        onRunningChanged: {
            if (!running) section.refreshState();
        }
    }

    Component.onCompleted: section.refreshState()

    Connections {
        target: section.tab
        function onVisibleChanged() {
            if (section.tab.visible) section.refreshState();
        }
    }

    SettingsRow {
        rootObj: section.tab.rootObj
        settingId: "desktop_companion"
        icon: "󰏩"  // md-paw
        wrapText: true
        title: I18n.t("guide.general.mascota.title", "Desktop companion")
        description: section.available
            ? I18n.t("guide.general.mascota.desc", "A pixel-art character that walks on your screen and climbs your windows. Click it to talk, double click opens Miku, right click sends it away. It hides while you play")
            : I18n.t("guide.general.mascota.missing", "Not installed yet: run «vortex actualizar»")
        searchKeywords: "companion mascot pet sprite pixel desktop shimeji compañera mascota escritorio personaje miku"

        Toggle {
            id: mascotaToggle
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: section.available && !toggleProc.running
            checked: section.mascotaOn
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) {
                if (toggleProc.running) return;
                toggleProc.command = ["vortex", "mascota", on ? "on" : "off"];
                toggleProc.running = true;
            }
        }
    }
}
