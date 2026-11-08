import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 3.3: el perfil de rendimiento (Ultra, Alto, Medio o Bajo), para que cada quien decida cuánto se gasta
// en verse bonito. El selector llama a «vortex rendimiento <perfil>», que recarga Hyprland y, al entrar o salir
// de Bajo, reinicia el shell (este panel se cierra con él). Mismo patrón que DisplayWindowManagerSection.qml:
// el comando va en su propia sesión para que el reinicio no lo corte.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj
    readonly property var perfiles: ["ultra", "alto", "medio", "bajo"]

    property string perfil: "ultra"
    readonly property bool busy: switchProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    function setPerfil(nuevo) {
        if (switchProc.running || nuevo === section.perfil) return;
        section.perfil = nuevo;
        switchProc.command = ["bash", "-c", 'setsid -w vortex rendimiento "$1" </dev/null >/dev/null 2>&1 & wait $!', "vortex", nuevo];
        switchProc.running = true;
    }

    Process {
        id: stateProc
        command: ["bash", "-c", "vortex rendimiento estado 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                let leido = text.trim();
                section.perfil = section.perfiles.indexOf(leido) >= 0 ? leido : "ultra";
            }
        }
    }

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
        settingId: "performance_profile"
        icon: "󰓅"
        wrapText: true
        title: I18n.t("guide.display.performance.title") || "Performance"
        description: I18n.t("guide.display.performance." + section.perfil + "_desc") || ""
        searchKeywords: "rendimiento perfil optimizar optimizacion extremo bajos recursos memoria ram animaciones desenfoque performance profile ultra alto medio bajo high medium low"

        Switch {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            implicitWidth: rootObj.s(280)
            implicitHeight: rootObj.s(32)
            enabled: !section.busy
            options: [I18n.t("guide.display.performance.ultra") || "Ultra", I18n.t("guide.display.performance.alto") || "High",
                      I18n.t("guide.display.performance.medio") || "Medium", I18n.t("guide.display.performance.bajo") || "Low"]
            currentIndex: section.perfiles.indexOf(section.perfil)
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface0
            textColor: ThemeBackend.subtext0
            activeTextColor: ThemeBackend.crust
            cornerRadius: ThemeBackend.borderRadius
            fontPixelSize: rootObj.s(11)
            onToggled: function(index) {
                section.setPerfil(section.perfiles[index]);
            }
        }
    }
}
