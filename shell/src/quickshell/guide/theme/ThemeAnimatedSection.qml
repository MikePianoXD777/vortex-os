import QtQuick
import QtQuick.Layouts
import Quickshell
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex: «Fondos animados» (videos .mp4/.webm/.mkv/.mov de la carpeta de fondos). Lo lee
// wallpaper/WallpaperEngine.qml desde «animatedWallpapers» en settings.json; todo viene encendido.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    property int rev: 0
    readonly property var cfg: {
        let dummy = section.rev;
        let a = Config.getSetting("animatedWallpapers", {});
        return (a && typeof a === "object") ? a : {};
    }

    Connections {
        target: Config
        function onSettingsLoaded() { section.rev++; }
    }

    function setOption(key, value) {
        let a = Config.getSetting("animatedWallpapers", {});
        if (!a || typeof a !== "object") a = {};
        a[key] = value;
        Config.setSetting("animatedWallpapers", a);
        section.rev++;
    }

    component OptionRow: SettingsRow {
        id: optionRow
        required property string optionKey
        rootObj: section.rootObj
        wrapText: true

        Toggle {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: optionRow.optionKey === "enabled" || section.cfg.enabled !== false
            checked: section.cfg[optionRow.optionKey] !== false
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) { section.setOption(optionRow.optionKey, on); }
        }
    }

    OptionRow {
        optionKey: "enabled"
        settingId: "animated_wallpapers"
        icon: "󰕧"
        title: I18n.t("guide.theme.animated.title", "Animated wallpapers")
        description: I18n.t("guide.theme.animated.desc", "Play video wallpapers (mp4, webm…). Turned off, they stay still on one frame")
        searchKeywords: "animated live lively video wallpaper background mp4 webm"
    }
    OptionRow {
        optionKey: "pauseCovered"
        settingId: "animated_wallpapers_covered"
        icon: "󰏤"
        title: I18n.t("guide.theme.animated.covered.title", "Pause behind windows")
        description: I18n.t("guide.theme.animated.covered.desc", "They stop while a window is open on that monitor's workspace")
        searchKeywords: "animated wallpaper pause windows covered performance"
    }
    OptionRow {
        optionKey: "pauseOnBattery"
        settingId: "animated_wallpapers_battery"
        icon: "󰁹"
        title: I18n.t("guide.theme.animated.battery.title", "Pause on battery")
        description: I18n.t("guide.theme.animated.battery.desc", "On a laptop, they stop while it is not plugged in")
        searchKeywords: "animated wallpaper battery laptop pause"
    }
    OptionRow {
        optionKey: "optimize"
        settingId: "animated_wallpapers_optimize"
        icon: "󰓅"
        title: I18n.t("guide.theme.animated.optimize.title", "Optimize videos")
        description: I18n.t("guide.theme.animated.optimize.desc", "Makes a light copy at your screen size (30 fps max, no audio) and plays that one")
        searchKeywords: "animated wallpaper optimize performance gpu cpu"
    }
}
