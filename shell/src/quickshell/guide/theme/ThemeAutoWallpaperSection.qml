import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex 1.8: «Fondos que cambian solos». Guarda los minutos en «autoWallpaper.minutes» de
// settings.json; «vortex fondos-auto correr» (lo arranca hyprland.lua) lo lee cada minuto y pone un
// fondo al azar de la carpeta de fondos. 0 = apagado (de fábrica).
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    readonly property var opciones: [0, 15, 30, 60, 180]

    property int rev: 0
    readonly property int minutos: {
        let dummy = section.rev;
        let a = Config.getSetting("autoWallpaper", {});
        let m = (a && typeof a === "object") ? Number(a.minutes) : 0;
        return isNaN(m) ? 0 : m;
    }

    Connections {
        target: Config
        function onSettingsLoaded() { section.rev++; }
    }

    SettingsRow {
        rootObj: section.rootObj
        settingId: "auto_wallpaper"
        icon: "󰸉"
        title: I18n.t("guide.theme.auto_wallpaper.title", "Wallpapers that change on their own")
        description: I18n.t("guide.theme.auto_wallpaper.desc", "Every so often, a random image from your wallpaper folder")
        searchKeywords: "wallpaper change automatic random slideshow shuffle rotate fondo cambiar solo azar"
        wrapText: true

        Dropdown {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            implicitWidth: section.rootObj.s(160)
            implicitHeight: section.rootObj.s(32)
            options: [
                I18n.t("guide.theme.auto_wallpaper.off", "Off"),
                I18n.t("guide.theme.auto_wallpaper.m15", "Every 15 min"),
                I18n.t("guide.theme.auto_wallpaper.m30", "Every 30 min"),
                I18n.t("guide.theme.auto_wallpaper.h1", "Every hour"),
                I18n.t("guide.theme.auto_wallpaper.h3", "Every 3 hours")
            ]
            currentIndex: Math.max(0, section.opciones.indexOf(section.minutos))
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface0
            hoverColor: ThemeBackend.surface1
            dropdownColor: ThemeBackend.surface0
            borderColor: Qt.alpha(ThemeBackend.surface2, 0.6)
            textColor: ThemeBackend.text
            activeTextColor: ThemeBackend.crust
            fontPixelSize: section.rootObj.s(11)
            onValueChanged: function(index, value) {
                let a = Config.getSetting("autoWallpaper", {});
                if (!a || typeof a !== "object") a = {};
                a.minutes = section.opciones[index] || 0;
                Config.setSetting("autoWallpaper", a);
                section.rev++;
            }
        }
    }
}
