import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.UPower
import "widgets"

ShellRoot {
    readonly property bool performanceMode: !!(Config.getSetting("general", {}).performance)
    readonly property bool quickactionsEnabled: Config.getSetting("general", {}).quickactions !== false
    readonly property bool dockEnabled: Config.getSetting("dock", {}).enabled !== false

    Connections {
        target: Quickshell
        function onReloadCompleted() { Quickshell.inhibitReloadPopup() }
        function onReloadFailed(errorString) { Quickshell.inhibitReloadPopup() }
    }

    LazyLoader {
        id: screenshotLoader
        ScreenshotOverlay {}
    }

    function ensureScreenshotOverlay() {
        if (!screenshotLoader.item) screenshotLoader.active = true;
        return screenshotLoader.item;
    }

    IpcHandler {
        target: "screenshotOverlay"

        function toggle(img: string, editMode: string, audioPrefs: string, cGeom: string, cGeomVideo: string, cMode: string, cBackend: string, targetMon: string): void {
            let overlay = ensureScreenshotOverlay();
            if (overlay) overlay.toggleOverlay(img, editMode, audioPrefs, cGeom, cGeomVideo, cMode, cBackend, targetMon);
        }

        function activate(img: string, editMode: string, audioPrefs: string, cGeom: string, cGeomVideo: string, cMode: string, cBackend: string, targetMon: string): void {
            let overlay = ensureScreenshotOverlay();
            if (overlay) overlay.activate(img, editMode, audioPrefs, cGeom, cGeomVideo, cMode, cBackend, targetMon);
        }

        function deactivate(): void {
            if (screenshotLoader.item) screenshotLoader.item.deactivate();
        }
    }

    Main { id: mainShell }

    // Vortex 1.7.5: bin/vortex-gestor, cuando el sistema se queda sin memoria, llama a
    // «quickshell ipc call vortex soltarMemoria» (antes no había quién contestara).
    IpcHandler {
        target: "vortex"

        function soltarMemoria(): void {
            mainShell.soltarMemoria();
        }
    }

    // Vortex 1.7.5: el JavaScript del shell junta basura en reposo (eventos de ventanas, música,
    // bandeja) y solo se recogía al cerrar Ajustes; cada 5 min se recoge (es rápido, ~ms).
    Timer {
        interval: 300000
        running: true
        repeat: true
        onTriggered: gc()
    }
    Bar {}
    Lock {}
    WidgetRedactor {}
    LockRedactor {}

    Launcher {}
    Clipboard {}    

    Polkit {}
    PopoutManager {}

    Loader {
        active: dockEnabled
        sourceComponent: Dock {}
    }

    Loader {
        active: !performanceMode
        sourceComponent: Idle {}
    }
    Variants {
        model: performanceMode ? [] : Quickshell.screens
        delegate: WidgetLoader {
            required property var modelData
            screen: modelData
            monitorName: modelData.name
        }
    }
    Loader {
        active: !performanceMode
        sourceComponent: WallpaperEngine {}
    }
    Loader {
        active: !performanceMode && quickactionsEnabled
        sourceComponent: Floating {}
    }

    // Vortex 1.8: modo laptop. Al desenchufar o enchufar la laptop, «vortex bateria on|off» (bin/vortex):
    // ahorro de energía, sin animaciones, brillo y Hz más bajos; al enchufar vuelve todo. También al
    // arrancar, por si se entró ya desenchufada. En una PC de escritorio onBattery nunca es true.
    Item {
        id: vortexBateria
        readonly property bool enBateria: UPower.onBattery
        function avisar() {
            Quickshell.execDetached(["vortex", "bateria", vortexBateria.enBateria ? "on" : "off"]);
        }
        onEnBateriaChanged: avisar()
        Component.onCompleted: if (enBateria) avisar()
    }

    Component.onCompleted: {
        Qt.application.organization = "serpantinum";
        Qt.application.domain = "serpantinum.org";
        Qt.application.name = "serpantinum";
        FirstLaunch.checkFirstLaunch();
        SysNotif.checkBattery();
    }
}
