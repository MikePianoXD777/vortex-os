import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex: conectar con Claude. Viene apagado; el interruptor llama a «vortex claude activar»
// (instala Claude Code para tu usuario y lo conecta con el escritorio; la primera vez que lo
// abres con Super+A te pide iniciar sesión con tu cuenta de Claude) o «vortex claude desactivar».
// El estado lo dice «vortex claude activado» (sale con 0 si está activado). Mismo patrón que
// ThemeOtakuSection.qml: el comando va en su propia sesión para que cerrar Ajustes no lo corte.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    property bool claudeOn: false
    readonly property bool busy: toggleProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    readonly property string runScript: 'command -v vortex >/dev/null || { notify-send -a "Vortexine\'s OS" "Claude" "Falta el comando vortex." 2>/dev/null; exit 127; }; '
        + 'setsid -w vortex claude "$1" </dev/null >/dev/null 2>&1 & wait $!'

    function setClaude(on) {
        if (toggleProc.running) return;
        toggleProc.command = ["bash", "-c", section.runScript, "vortex", on ? "activar" : "desactivar"];
        toggleProc.running = true;
        recheckTimer.restart();
    }

    Process {
        id: stateProc
        command: ["bash", "-c", "vortex claude activado >/dev/null 2>&1"]
        onExited: function(exitCode, exitStatus) {
            section.claudeOn = (exitCode === 0);
            claudeToggle.checked = section.claudeOn;
        }
    }

    // Activar puede tardar (baja Claude Code): el interruptor queda quieto mientras corre.
    Process {
        id: toggleProc
        onRunningChanged: {
            if (!running) section.refreshState();
        }
    }

    Timer {
        id: recheckTimer
        interval: 800
        repeat: false
        onTriggered: {
            if (!toggleProc.running) section.refreshState();
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
        settingId: "claude_connect"
        icon: "󰚩"
        wrapText: true
        title: I18n.t("guide.general.claude.title") || "Claude"
        description: section.busy ? (I18n.t("guide.general.claude.working") || "Working…") : (I18n.t("guide.general.claude.desc") || "Connect with Claude")
        searchKeywords: "claude ia ai asistente assistant anthropic microfono voz pantalla"

        Toggle {
            id: claudeToggle
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: !section.busy
            checked: section.claudeOn
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) {
                section.setClaude(on);
            }
        }
    }
}
