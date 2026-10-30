import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj
    property alias soundDropdown: soundDropdown

    SettingsRow {
        rootObj: section.tab.rootObj
        icon: "󰂚"
        title: I18n.t("guide.notifications.sound.title")
        description: I18n.t("guide.notifications.sound.desc")

        Toggle {
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            checked: section.tab.soundEnabled
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(c) {
                section.tab.soundEnabled = c;
                section.tab.updateNotifSetting("sound", c);
            }
        }
    }

    SettingsRow {
        rootObj: section.tab.rootObj
        visible: section.tab.soundEnabled
        icon: "󰎆"
        title: I18n.t("guide.notifications.sound_file.title")
        description: I18n.t("guide.notifications.sound_file.desc")

        IconButton {
            Layout.preferredWidth: rootObj.s(32)
            Layout.preferredHeight: rootObj.s(32)
            Layout.alignment: Qt.AlignVCenter
            cornerRadius: rootObj.s(6)
            buttonIcon: "󰉋"
            iconFontSize: rootObj.s(14)
            accentColor: ThemeBackend.surface0
            textColor: isHoveredOrHighlighted ? ThemeBackend.text : ThemeBackend.overlay2
            onClicked: {
                let userSoundsPath = Caching.stateDir ? (Caching.stateDir + "/sounds/notifications") : (Caching.home + "/.local/state/serpantinum/sounds/notifications");
                let escapeBash = function(str) { return String(str).replace(/(["\\$`])/g, '\\$1'); };
                Quickshell.execDetached(["bash", "-c", "mkdir -p \"" + escapeBash(userSoundsPath) + "\" && xdg-open \"" + escapeBash(userSoundsPath) + "\""]);
            }
        }

        IconButton {
            Layout.preferredWidth: rootObj.s(32)
            Layout.preferredHeight: rootObj.s(32)
            Layout.alignment: Qt.AlignVCenter
            cornerRadius: rootObj.s(6)
            buttonIcon: "󰐕"
            iconFontSize: rootObj.s(14)
            accentColor: ThemeBackend.surface0
            textColor: isHoveredOrHighlighted ? ThemeBackend.text : ThemeBackend.overlay2
            onClicked: section.tab.soundPickerPopup.openPicker()
        }

        Dropdown {
            id: soundDropdown
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredWidth: rootObj.s(180)
            Layout.preferredHeight: rootObj.s(32)
            options: section.tab.availableSounds
            currentIndex: {
                for (let i = 0; i < section.tab.soundList.length; i++) {
                    if (section.tab.soundList[i].path === section.tab.selectedSound) {
                        return i;
                    }
                }
                return 0;
            }
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface0
            hoverColor: ThemeBackend.surface1
            dropdownColor: ThemeBackend.surface0
            borderColor: Qt.alpha(ThemeBackend.surface2, 0.6)
            textColor: ThemeBackend.text
            activeTextColor: ThemeBackend.crust
            cornerRadius: ThemeBackend.borderRadius
            fontPixelSize: rootObj.s(11)
            onSelected: function(index, value) {
                if (index >= 0 && index < section.tab.soundList.length) {
                    let targetPath = section.tab.soundList[index].path;
                    section.tab.selectedSound = targetPath;
                    section.tab.updateNotifSetting("soundFile", targetPath);
                    if (typeof Sounds !== "undefined" && typeof Sounds.play === "function") {
                        Sounds.play(targetPath);
                    }
                }
            }
        }
    }

    // 2.2: pega el enlace de un botón de myinstants.com o de un mp3/wav y queda elegido
    SettingsRow {
        rootObj: section.tab.rootObj
        visible: section.tab.soundEnabled
        icon: "󰌹"
        title: I18n.t("guide.notifications.sound_link.title", "Add from a link")
        description: section.linkMessage !== "" ? section.linkMessage : I18n.t("guide.notifications.sound_link.desc", "A myinstants.com button or a link to an mp3, wav or ogg")
        descriptionColor: section.linkError ? ThemeBackend.red : ThemeBackend.subtext0
        wrapText: true

        Input {
            id: linkInput
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            implicitWidth: rootObj.s(220)
            implicitHeight: rootObj.s(32)
            placeholderText: "myinstants.com/…/instant/…"
            enabled: !section.linkBusy
            isBusy: section.linkBusy
            hasError: section.linkError
            baseColor: ThemeBackend.surface0
            accentColor: ThemeBackend.mauve
            textColor: ThemeBackend.text
            subTextColor: ThemeBackend.subtext0
            borderColor: Qt.alpha(ThemeBackend.surface2, 0.6)
            cornerRadius: ThemeBackend.borderRadius
            fontPixelSize: rootObj.s(11)
            onTextEdited: function(newText) { linkInput.text = newText; }
            onAccepted: function(finalText) { section.addLink(finalText); }
        }

        ClickButton {
            Layout.alignment: Qt.AlignVCenter
            Layout.preferredHeight: rootObj.s(32)
            cornerRadius: ThemeBackend.borderRadius
            buttonIcon: section.linkBusy ? "󰇚" : "󰐕"
            buttonText: I18n.t("guide.notifications.sound_link.add", "Add")
            iconFontSize: rootObj.s(13)
            textFontSize: rootObj.s(11)
            accentColor: ThemeBackend.surface0
            textColor: ThemeBackend.text
            onClicked: section.addLink(linkInput.text)
        }
    }

    // Sonidos desde un enlace (2.2): `vortex sonidos agregar` baja el botón de myinstants.com o el mp3/wav y
    // deja su ruta en la última línea; aquí se elige y se toca. Los tuyos (no los que trae el shell) se pueden borrar.
    readonly property string userSoundsPath: Caching.stateDir ? (Caching.stateDir + "/sounds/notifications") : (Caching.home + "/.local/state/serpantinum/sounds/notifications")
    readonly property var userSounds: section.tab.soundList.filter(s => s.path.indexOf(userSoundsPath + "/") === 0)
    property bool linkBusy: false
    property string linkMessage: ""
    property bool linkError: false

    function addLink(link) {
        link = String(link || "").trim();
        if (link === "" || linkBusy) return;
        linkBusy = true;
        linkError = false;
        linkMessage = I18n.t("guide.notifications.sound_link.busy", "Downloading…");
        linkInput.clear();
        linkProcess.command = ["vortex", "sonidos", "agregar", link];
        linkProcess.running = true;
    }

    function removeSound(path) {
        let file = String(path).substring(String(path).lastIndexOf("/") + 1);
        removeProcess.command = ["vortex", "sonidos", "quitar", file];
        removeProcess.running = true;
    }

    function useSound(path) {
        section.tab.selectedSound = path;
        section.tab.updateNotifSetting("soundFile", path);
        if (typeof Sounds !== "undefined" && typeof Sounds.play === "function") Sounds.play(path);
    }

    // «vortex: no pude bajar…» → «No pude bajar…»
    function cleanVortexMessage(txt) {
        let lines = String(txt).trim().split("\n");
        let last = lines[lines.length - 1].replace(/^vortex: /, "");
        return last.charAt(0).toUpperCase() + last.substring(1);
    }

    Process {
        id: linkProcess
        running: false
        stdout: StdioCollector { id: linkOut }
        stderr: StdioCollector { id: linkErr }
        onExited: function(exitCode, exitStatus) {
            section.linkBusy = false;
            let lines = linkOut.text.trim().split("\n");
            let sPath = lines[lines.length - 1].trim();
            if (exitCode === 0 && sPath.indexOf("/") === 0) {
                section.linkError = false;
                section.linkMessage = I18n.t("guide.notifications.sound_link.added", "Added «%1»").replace("%1", sPath.substring(sPath.lastIndexOf("/") + 1).replace(/\.[^.]*$/, ""));
                section.useSound(sPath);
                section.tab.reloadSounds();
            } else {
                section.linkError = true;
                section.linkMessage = linkErr.text.trim() !== "" ? section.cleanVortexMessage(linkErr.text)
                                                                : I18n.t("guide.notifications.sound_link.failed", "Couldn't add that sound");
            }
        }
    }

    Process {
        id: removeProcess
        running: false
        onExited: section.tab.reloadSounds()
    }

    // Tus sonidos (los de la carpeta tuya): escuchar, usar o borrar
    Repeater {
        model: section.tab.soundEnabled ? section.userSounds : []

        delegate: SettingsRow {
            required property var modelData
            readonly property bool inUse: modelData.path === section.tab.selectedSound
            rootObj: section.tab.rootObj
            searchable: false
            icon: inUse ? "󰄬" : "󰎆"
            title: modelData.name
            description: inUse ? I18n.t("guide.notifications.sound_mine.in_use", "In use") : I18n.t("guide.notifications.sound_mine.desc", "Yours")

            IconButton {
                Layout.preferredWidth: rootObj.s(32)
                Layout.preferredHeight: rootObj.s(32)
                Layout.alignment: Qt.AlignVCenter
                cornerRadius: rootObj.s(6)
                buttonIcon: "󰐊"
                iconFontSize: rootObj.s(14)
                accentColor: ThemeBackend.surface0
                textColor: isHoveredOrHighlighted ? ThemeBackend.text : ThemeBackend.overlay2
                onClicked: if (typeof Sounds !== "undefined" && typeof Sounds.play === "function") Sounds.play(modelData.path)
            }

            ClickButton {
                visible: !inUse
                Layout.alignment: Qt.AlignVCenter
                Layout.preferredHeight: rootObj.s(32)
                cornerRadius: ThemeBackend.borderRadius
                buttonText: I18n.t("guide.notifications.sound_mine.use", "Use")
                textFontSize: rootObj.s(11)
                accentColor: ThemeBackend.surface0
                textColor: ThemeBackend.text
                onClicked: section.useSound(modelData.path)
            }

            IconButton {
                Layout.preferredWidth: rootObj.s(32)
                Layout.preferredHeight: rootObj.s(32)
                Layout.alignment: Qt.AlignVCenter
                cornerRadius: rootObj.s(6)
                buttonIcon: "󰆴"
                iconFontSize: rootObj.s(14)
                accentColor: ThemeBackend.surface0
                textColor: isHoveredOrHighlighted ? ThemeBackend.red : ThemeBackend.overlay2
                onClicked: section.removeSound(modelData.path)
            }
        }
    }
}
