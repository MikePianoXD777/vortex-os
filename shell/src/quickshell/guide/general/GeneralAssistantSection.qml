import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../../"
import "../../reusables"
import "../../reusables/guide"

// Vortex: sección «Asistente». Un interruptor «Miku en 3D» que llama a vortex-miku (el paquete
// vortex-miku, el asistente de voz de Vortex; Super + O lo abre y lo cierra): encendido, que es
// como viene de fábrica, sale Miku Nakano en 3D al lado de la pantalla; apagado queda solo como
// asistente de voz, en un panel chico, y gasta menos RAM. El comando hace todo el trabajo
// («vortex-miku avatar on|off» guarda la opción en ~/.config/vortex/miku.json y, si está abierta,
// la reabre en el modo nuevo); aquí solo se refleja su estado, que se lee con «vortex-miku estado»
// (dos líneas: avatar=on|off y abierta=si|no). Miku piensa con Claude: si está apagado (viene
// apagado de fábrica, ver GeneralClaudeSection.qml), la descripción lo dice.
SettingsBlob {
    id: section
    required property var tab
    readonly property var rootObj: tab.rootObj

    // true = Miku en 3D (el valor de fábrica); false = solo la voz.
    property bool avatarOn: true
    // false = no hay vortex-miku (el paquete no está instalado) o no contestó.
    property bool available: true
    // false = Claude está apagado («vortex claude activado» sale con 1). Sin respuesta de «vortex»
    // no se avisa de nada: no se inventa.
    property bool claudeOff: false
    readonly property bool busy: toggleProc.running

    function refreshState() {
        stateProc.running = false;
        stateProc.running = true;
    }

    // Al cerrar Ajustes el shell descarga esta sección y mata a lo que su Process tenga corriendo:
    // sin más, cerrarla mientras Miku se reabre dejaría el cambio a medias. Por eso el comando va
    // en su propia sesión (setsid) y sin pegarse a las tuberías del Process (no lo mata un
    // SIGPIPE): sigue solo hasta el final, y al volver a abrir Ajustes se lee el estado. Si el
    // comando no está (un sistema donde no se instaló el paquete) avisa en vez de callar.
    readonly property string runScript: 'command -v vortex-miku >/dev/null || { notify-send -a "Vortex OS" "Miku" "Falta el asistente (paquete vortex-miku). En la carpeta de Vortex: cd packaging/vortex-miku && makepkg -sif" 2>/dev/null; exit 127; }; ' + 'setsid -w vortex-miku avatar "$1" </dev/null >/dev/null 2>&1 & wait $!'

    // El estado se lee con una sola salida: las líneas de «vortex-miku estado», «rc=N» con su
    // código (127 si falta el comando, 124 si no contestó en 5 s) y «claude=si|no|?» (si Claude
    // está activado; «?» si no hay comando vortex). Así basta un evento (el fin del flujo) y no
    // hay que esperar a varias señales distintas.
    readonly property string stateScript: 'if command -v vortex-miku >/dev/null; then timeout 5 vortex-miku estado 2>/dev/null; echo rc=$?; else echo rc=127; fi; ' + 'if command -v vortex >/dev/null; then if timeout 5 vortex claude activado >/dev/null 2>&1; then echo claude=si; else echo claude=no; fi; else echo claude=?; fi'

    function setAvatar(on) {
        if (toggleProc.running)
            return;
        toggleProc.command = ["bash", "-c", section.runScript, "vortex-miku", on ? "on" : "off"];
        toggleProc.running = true;
        // Si el comando no pudo ni arrancar, no hay "terminó": se vuelve a leer el estado real.
        recheckTimer.restart();
    }

    function applyState(text) {
        let rc = -1;
        let avatar = "";
        let claude = "?";
        let lines = text.split("\n");
        for (let i = 0; i < lines.length; i++) {
            let l = lines[i].trim();
            if (l.indexOf("rc=") === 0)
                rc = parseInt(l.substring(3));
            else if (l.indexOf("avatar=") === 0)
                avatar = l.substring(7);
            else if (l.indexOf("claude=") === 0)
                claude = l.substring(7);
        }
        section.available = (rc === 0);
        section.claudeOff = (claude === "no");
        // Sin respuesta se deja lo que había (de fábrica, encendido): no se inventa un cambio.
        if (rc === 0 && (avatar === "on" || avatar === "off"))
            section.avatarOn = (avatar === "on");
        avatarToggle.checked = section.avatarOn;
    }

    Process {
        id: stateProc
        command: ["bash", "-c", section.stateScript]
        stdout: StdioCollector {
            onStreamFinished: section.applyState(this.text)
        }
    }

    // El comando puede tardar (cierra y vuelve a abrir a Miku): el interruptor queda quieto
    // mientras corre.
    Process {
        id: toggleProc
        onRunningChanged: {
            if (!running)
                section.refreshState();
        }
    }

    Timer {
        id: recheckTimer
        interval: 800
        repeat: false
        onTriggered: {
            if (!toggleProc.running)
                section.refreshState();
        }
    }

    Component.onCompleted: section.refreshState()

    Connections {
        target: section.tab
        function onVisibleChanged() {
            if (section.tab.visible)
                section.refreshState();
        }
    }

    SettingsRow {
        rootObj: section.tab.rootObj
        settingId: "miku_avatar"
        icon: "󰍬"
        wrapText: true
        title: I18n.t("guide.general.assistant.title") || "Miku in 3D"
        description: section.busy ? (I18n.t("guide.general.assistant.working") || "Switching…") : (!section.available ? (I18n.t("guide.general.assistant.missing") || "The assistant is not installed (vortex-miku package)") : ((I18n.t("guide.general.assistant.desc") || "Super + O opens and closes Miku, the voice assistant") + (section.claudeOff ? " " + (I18n.t("guide.general.assistant.claude_off") || "Claude is off: turn it on in the section above") : "")))
        searchKeywords: "asistente voz miku nakano claude microfono micrófono avatar 3d modelo vrm assistant voice microphone super o"

        Toggle {
            id: avatarToggle
            Layout.alignment: Qt.AlignRight | Qt.AlignVCenter
            enabled: !section.busy
            checked: section.avatarOn
            accentColor: ThemeBackend.mauve
            baseColor: ThemeBackend.surface1
            handleColor: ThemeBackend.crust
            handleOffColor: ThemeBackend.text
            onToggled: function (on) {
                section.setAvatar(on);
            }
        }
    }
}
