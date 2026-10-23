import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex: «Modo otaku» (easter egg). Un interruptor que llama a vortex-otaku (bin/vortex-otaku):
// al encender baja fondos de Miku Nakano de Wallhaven (sin red, usa uno original de Vortex),
// los pone de fondo y le da un toque suave al tema; al apagar devuelve el fondo y los ajustes
// de antes. El comando hace todo el trabajo; aquí solo se refleja su estado, que es el archivo
// ~/.config/vortex/otaku (existe = encendido).
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    readonly property string markerPath: (Quickshell.env("XDG_CONFIG_HOME") || (Quickshell.env("HOME") + "/.config")) + "/vortex/otaku"
    property bool otakuOn: false
    readonly property bool busy: toggleProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    // Al cerrar Ajustes el shell descarga esta sección y mata a lo que su Process tenga corriendo:
    // sin más, cerrarla durante la descarga dejaría el modo a medias. Por eso el comando va en su
    // propia sesión (setsid) y sin pegarse a las tuberías del Process (no lo mata un SIGPIPE):
    // sigue solo hasta el final, y al volver a abrir Ajustes se lee la marca. Si el comando no
    // está (un sistema que se actualizó sin reinstalar /usr/local/bin) avisa en vez de callar.
    readonly property string runScript: 'command -v vortex-otaku >/dev/null || { notify-send -a "Vortex OS" "Modo otaku" "Falta el comando vortex-otaku. En la carpeta de Vortex: sudo install -m755 bin/vortex bin/vortex-otaku /usr/local/bin/" 2>/dev/null; exit 127; }; '
        + 'setsid -w vortex-otaku "$1" </dev/null >/dev/null 2>&1 & wait $!'

    function setOtaku(on) {
        if (toggleProc.running) return;
        toggleProc.command = ["bash", "-c", section.runScript, "vortex-otaku", on ? "on" : "off"];
        toggleProc.running = true;
        // Si el comando no pudo ni arrancar, no hay "terminó": se vuelve a leer el estado real.
        recheckTimer.restart();
    }

    // Existe la marca = encendido. «test -f» sale con 0 si el archivo está.
    Process {
        id: stateProc
        command: ["test", "-f", section.markerPath]
        onExited: function(exitCode, exitStatus) {
            section.otakuOn = (exitCode === 0);
            otakuToggle.checked = section.otakuOn;
        }
    }

    // El comando puede tardar (descarga los fondos): el interruptor queda quieto mientras corre.
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
        settingId: "otaku_mode"
        icon: "󰋋"
        wrapText: true
        title: I18n.t("guide.theme.otaku.title")
        description: section.busy ? I18n.t("guide.theme.otaku.working") : I18n.t("guide.theme.otaku.desc")
        searchKeywords: "otaku anime miku nakano wallpaper background easter egg headphones gotoubun"

        Toggle {
            id: otakuToggle
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: !section.busy
            checked: section.otakuOn
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function(on) {
                section.setOtaku(on);
            }
        }
    }
}
