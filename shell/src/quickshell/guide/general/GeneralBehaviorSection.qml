import QtQuick
import "../../"
import "../../reusables"
import "../../reusables/guide"

SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    SettingsRow {
        rootObj: section.tab.rootObj
        icon: "󱓞"
        title: I18n.t("guide.general.quickactions.title") || "Quickactions"
        description: I18n.t("guide.general.quickactions.desc") || "Enable floating quickactions overlay"

        Toggle {
            checked: section.tab.quickactions
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(c) {
                section.tab.quickactions = c;
                section.tab.updateGeneralSettings();
            }
        }
    }

    SettingsRow {
        rootObj: section.tab.rootObj
        icon: "󰖨"
        title: I18n.t("guide.general.bienvenida.title", "Welcome when you sign in")
        description: I18n.t("guide.general.bienvenida.desc", "A card with the weather, your tasks for today and your music")

        Toggle {
            checked: section.tab.bienvenida
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(c) {
                section.tab.bienvenida = c;
                section.tab.updateGeneralSettings();
            }
        }
    }
}
