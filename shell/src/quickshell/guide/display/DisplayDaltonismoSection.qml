import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 4.4: el filtro de pantalla para daltonismo. Un shader de Hyprland corrige los colores de todo lo que
// sale en pantalla (escritorio, apps y juegos) según qué color cuesta distinguir: rojo (protanopia), verde
// (deuteranopia) o azul (tritanopia); o todo en grises. El selector llama a «vortex daltonismo <filtro>», que
// lo pone al instante y lo deja guardado (el modo juego y cada recarga de Hyprland lo respetan).
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj
    readonly property var filtros: ["no", "protanopia", "deuteranopia", "tritanopia", "grises"]

    property string filtro: "no"
    readonly property bool busy: switchProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    function setFiltro(nuevo) {
        if (switchProc.running || nuevo === section.filtro) return;
        section.filtro = nuevo;
        switchProc.command = ["vortex", "daltonismo", nuevo];
        switchProc.running = true;
    }

    Process {
        id: stateProc
        command: ["bash", "-c", "vortex daltonismo estado 2>/dev/null"]
        stdout: StdioCollector {
            onStreamFinished: {
                let leido = text.trim();
                section.filtro = section.filtros.indexOf(leido) >= 0 ? leido : "no";
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
        settingId: "colorblind_filter"
        icon: "󰈈"
        wrapText: true
        title: I18n.t("guide.display.colorblind.title", "Color blindness")
        description: I18n.t("guide.display.colorblind." + section.filtro + "_desc", "")
        searchKeywords: "daltonismo daltonico daltónico colores accesibilidad protanopia deuteranopia tritanopia grises rojo verde azul color blind colorblind accessibility filter grayscale"

        Switch {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            implicitWidth: rootObj.s(340)
            implicitHeight: rootObj.s(32)
            enabled: !section.busy
            options: [I18n.t("guide.display.colorblind.no", "Off"), I18n.t("guide.display.colorblind.protanopia", "Red"),
                      I18n.t("guide.display.colorblind.deuteranopia", "Green"), I18n.t("guide.display.colorblind.tritanopia", "Blue"),
                      I18n.t("guide.display.colorblind.grises", "Gray")]
            currentIndex: section.filtros.indexOf(section.filtro)
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface0
            textColor: ThemeBackend.subtext0
            activeTextColor: ThemeBackend.crust
            cornerRadius: ThemeBackend.borderRadius
            fontPixelSize: rootObj.s(11)
            onToggled: function(index) {
                section.setFiltro(section.filtros[index]);
            }
        }
    }
}
