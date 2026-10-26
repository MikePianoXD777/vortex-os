import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 1.8: «Contador de FPS» (MangoHud). El interruptor llama a «vortex fps on|off» (Steam,
// Heroic, Lutris y los juegos de Flatpak arrancan con MangoHud); el estado lo dice «vortex fps
// estado». Viene apagado. Mismo patrón que GeneralClaudeSection.qml.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    property bool fpsOn: false

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    Process {
        id: stateProc
        command: ["vortex", "fps", "estado"]
        stdout: StdioCollector {
            onStreamFinished: {
                section.fpsOn = this.text.trim() === "encendido";
                fpsToggle.checked = section.fpsOn;
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
        settingId: "fps_counter"
        icon: "󰊴"
        wrapText: true
        title: I18n.t("guide.general.fps.title", "FPS counter in games")
        description: I18n.t("guide.general.fps.desc", "FPS, GPU and CPU in the corner of your games (MangoHud). Right Shift + F12 hides it. Reopen Steam after changing it")
        searchKeywords: "fps counter mangohud games overlay frames gpu cpu contador juegos steam rendimiento"

        Toggle {
            id: fpsToggle
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: !toggleProc.running
            checked: section.fpsOn
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) {
                if (toggleProc.running) return;
                toggleProc.command = ["vortex", "fps", on ? "on" : "off"];
                toggleProc.running = true;
            }
        }
    }
}
