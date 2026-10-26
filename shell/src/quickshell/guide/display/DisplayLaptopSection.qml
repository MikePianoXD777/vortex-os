import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Services.UPower
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 1.8: «Modo laptop». Encendido (de fábrica), al desenchufar la laptop se ahorra energía
// («vortex bateria on», lo dispara Shell.qml): perfil de ahorro, sin animaciones, brillo a 60 % y la
// pantalla a 60 Hz; al enchufarla vuelve todo. Se guarda en «modoLaptop» de settings.json. Solo se ve
// en equipos con batería.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj
    visible: UPower.displayDevice.ready && UPower.displayDevice.isLaptopBattery

    property int rev: 0
    readonly property bool encendido: {
        let dummy = section.rev;
        return Config.getSetting("modoLaptop", true) !== false;
    }

    Connections {
        target: Config
        function onSettingsLoaded() { section.rev++; }
    }

    SettingsRow {
        rootObj: section.rootObj
        settingId: "laptop_mode"
        icon: "󰂄"
        title: I18n.t("guide.display.laptop.title", "Laptop mode")
        description: I18n.t("guide.display.laptop.desc", "When unplugged: power saving, no animations, dimmer screen and 60 Hz. Everything comes back when you plug it in")
        searchKeywords: "laptop battery power saving unplugged energy brightness hz bateria ahorro energia desenchufar portatil"
        wrapText: true

        Toggle {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            checked: section.encendido
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) {
                Config.setSetting("modoLaptop", on);
                section.rev++;
                // Si ya está desenchufada, el cambio vale de inmediato.
                if (UPower.onBattery) Quickshell.execDetached(["vortex", "bateria", on ? "on" : "off"]);
            }
        }
    }
}
