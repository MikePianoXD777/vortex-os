import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex (1.7.5): el gestor de ventanas, mosaico (como qtile, de fábrica) o KDE (ventanas flotantes con
// barra de título, para quien viene de KDE o Windows). El selector llama a «vortex ventanas mosaico|kde»,
// que recarga Hyprland y reinicia el shell (este panel se cierra con él). El estado lo dice
// «vortex ventanas estado». Mismo patrón que GeneralClaudeSection.qml: el comando va en su propia
// sesión para que el reinicio del shell no lo corte.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    property string mode: "mosaico"
    readonly property bool busy: switchProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    readonly property string runScript: 'command -v vortex >/dev/null || { notify-send -a "Vortexine\'s OS" "Gestor de ventanas" "Falta el comando vortex." 2>/dev/null; exit 127; }; '
        + 'setsid -w vortex ventanas "$1" </dev/null >/dev/null 2>&1 & wait $!'

    function setMode(newMode) {
        if (switchProc.running || newMode === section.mode) return;
        section.mode = newMode;
        switchProc.command = ["bash", "-c", section.runScript, "vortex", newMode];
        switchProc.running = true;
    }

    Process {
        id: stateProc
        command: ["bash", "-c", "vortex ventanas estado 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                section.mode = text.trim() === "kde" ? "kde" : "mosaico";
            }
        }
    }

    // Compilar la barra de título la primera vez tarda un minuto: el selector queda quieto mientras.
    Process {
        id: switchProc
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
        settingId: "window_manager"
        icon: "󰕰"
        wrapText: true
        title: I18n.t("guide.display.window_manager.title") || "Window manager"
        description: section.busy ? (I18n.t("guide.display.window_manager.working") || "Switching…") : (I18n.t("guide.display.window_manager.desc") || "Tiling or floating windows")
        searchKeywords: "gestor ventanas window manager mosaico tiling qtile kde windows flotantes floating barra titulo"

        Switch {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            implicitWidth: rootObj.s(170)
            implicitHeight: rootObj.s(32)
            enabled: !section.busy
            options: [I18n.t("guide.display.window_manager.tiling") || "Tiling", I18n.t("guide.display.window_manager.floating") || "KDE"]
            currentIndex: section.mode === "kde" ? 1 : 0
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface0
            textColor: ThemeBackend.subtext0
            activeTextColor: ThemeBackend.crust
            cornerRadius: ThemeBackend.borderRadius
            fontPixelSize: rootObj.s(11)
            onToggled: function(index) {
                section.setMode(index === 1 ? "kde" : "mosaico");
            }
        }
    }
}
