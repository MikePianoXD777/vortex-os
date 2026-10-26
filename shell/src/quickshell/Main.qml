import QtQuick
import QtQuick.Window
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Hyprland
import Quickshell.Bluetooth
import Quickshell.Networking
import "WindowRegistry.js" as Registry
import "notifications" as Notifs

PanelWindow {
    id: masterWindow
    color: "transparent"

    anchors {
        top: true
        bottom: true
        left: true
        right: true
    }

    // Vortex: el panel abierto (Ajustes, red, audio, música…) se cierra al hacer clic fuera de él.
    // HyprlandFocusGrab solo se suelta con un clic en otra superficie (no al pasar el ratón,
    // aunque el foco siga al ratón): entonces se cierra y se recuerda cuál fue, para que el
    // «toggle» de su botón en la barra, que llega justo después, no lo vuelva a abrir.
    property string vortexCerradoPorClic: ""
    property double vortexCerradoEn: 0
    HyprlandFocusGrab {
        id: vortexClicFuera
        windows: [masterWindow]
        active: masterWindow.currentActive !== "hidden" && !masterWindow.isClosing
        onCleared: {
            if (masterWindow.currentActive === "hidden") return;
            masterWindow.vortexCerradoPorClic = masterWindow.currentActive;
            masterWindow.vortexCerradoEn = Date.now();
            switchWidget("hidden", "");
        }
    }

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        enabled: masterWindow.isVisible && !masterWindow.isClosing
        onActivated: switchWidget("hidden", "")
    }

    function resolveTargetScreen() {
        try {
            if (typeof Hyprland !== "undefined" && Hyprland.focusedMonitor) {
                let monName = Hyprland.focusedMonitor.name;
                let screens = Quickshell.screens || [];
                let len = screens.length !== undefined ? screens.length : (screens.count !== undefined ? screens.count : 0);
                for (let i = 0; i < len; i++) {
                    let s = screens[i] !== undefined ? screens[i] : screens.get(i);
                    if (s && s.name === monName) return s;
                }
            }
        } catch (e) {}

        try {
            if (typeof ToplevelManager !== "undefined" && ToplevelManager.activeToplevel && ToplevelManager.activeToplevel.screens && ToplevelManager.activeToplevel.screens.length > 0) {
                return ToplevelManager.activeToplevel.screens[0];
            }
        } catch (e) {}

        if (masterWindow.screen) return masterWindow.screen;
        return (Quickshell.screens && (Quickshell.screens.length > 0 || Quickshell.screens.count > 0))
            ? (Quickshell.screens[0] || Quickshell.screens.get(0))
            : null;
    }

    function reportWidgetState() {
        if (typeof Caching === "undefined" || !Caching.runDir) return;
        let sName = (masterWindow.currentActive === "hidden" || !masterWindow.screen) ? "" : (masterWindow.screen.name || "");
        let payload = JSON.stringify({
            widget: masterWindow.currentActive,
            screen: sName
        });
        // Vortex: se escribe directo, sin lanzar «bash -c echo» (un doble fork del shell
        // entero) cada vez que se abre o se cierra un panel.
        widgetStateFile.setText(payload + "\n");
    }

    FileView {
        id: widgetStateFile
        path: Caching.runDir ? Caching.runDir + "/current_widget" : ""
        preload: false
        printErrors: false
    }

    Process {
        id: startupResetProcess
        command: ["bash", "-c", `[ -n "${Caching.runDir}" ] && echo '{"widget":"hidden","screen":""}' > "${Caching.runDir}/current_widget"`]
        running: true
    }

    Connections {
        target: (typeof Caching !== "undefined") ? Caching : null
        function onRunDirChanged() {
            masterWindow.reportWidgetState();
        }
    }

    // Vortex: SUPER+número cambia de escritorio directo en Hyprland y manda el evento
    // «custom>>serpantinum:close» (compositors/hyprland/config/keybinds.lua). Aquí se
    // cierran los popups igual que con «serpantinum msg close», sin lanzar procesos.
    Connections {
        target: Boolean(Quickshell.env("HYPRLAND_INSTANCE_SIGNATURE")) ? Hyprland : null
        function onRawEvent(event) {
            if (event.name !== "custom") return;
            if (event.data === "serpantinum:close") {
                mainIpc.handleCommand("close", "", "");
            } else if (event.data.indexOf("serpantinum:toggle ") === 0) {
                // Vortex: los atajos de los paneles (keybinds.lua) llegan aquí sin lanzar procesos
                let target = event.data.substring(19).trim();
                if (target === "" || target === "wallpaper") return;
                mainIpc.handleCommand("toggle", target, "");
                masterWindow.queueWidgetLaunchLog(target);
            }
        }
    }

    // Vortex: lo mismo que log_widget_launch de qs_manager.sh (el lanzador ordena los
    // paneles por uso), pero un momento después, para no competir con la animación.
    property var pendingLaunchLogs: []

    function queueWidgetLaunchLog(target) {
        let names = [target];
        let known = ["network", "volume", "guide", "calendar", "music", "notifications", "system", "keybinds", "celular"];
        if (known.indexOf(target) !== -1 && typeof I18n !== "undefined") {
            let shown = I18n.t("widgets." + target + ".name", "");
            if (shown && shown !== target && shown.indexOf("widgets.") !== 0) names.push(shown);
        }
        masterWindow.pendingLaunchLogs = masterWindow.pendingLaunchLogs.concat(names);
        launchLogTimer.restart();
    }

    Timer {
        id: launchLogTimer
        interval: 1500
        repeat: false
        onTriggered: {
            let names = masterWindow.pendingLaunchLogs;
            masterWindow.pendingLaunchLogs = [];
            if (!Caching.qsDir || names.length === 0) return;
            let args = ["python3", Caching.qsDir + "/launcher/app_rank.py", "--log-launch", "--name"];
            for (let i = 0; i < names.length; i++) Quickshell.execDetached(args.concat([names[i]]));
        }
    }

    IpcHandler {
        id: mainIpc
        target: "main"

        function forceReload(): void {
            Quickshell.reload(true);
        }

        function clearNotifications(): void {
            NotificationManager.clearNotifications();
        }

        function handleCommand(cmd: string, targetWidget: string, arg: string): void {
            cmd = cmd || "";
            targetWidget = targetWidget || "";
            arg = arg || "";

            // Vortex: el clic en el botón de la barra de un panel abierto primero lo cierra (clic
            // fuera, ver vortexClicFuera) y luego llega su «toggle»: sin esto se volvería a abrir.
            if (cmd === "toggle" && targetWidget !== "" && targetWidget === masterWindow.vortexCerradoPorClic
                    && Date.now() - masterWindow.vortexCerradoEn < 600) {
                masterWindow.vortexCerradoPorClic = "";
                return;
            }

            if (cmd === "clearNotifications" || cmd === "clear_notifications" || cmd === "clearNotifs") {
                NotificationManager.clearNotifications();
                return;
            }

            if (cmd === "launcher" || targetWidget === "launcher") {
                if (cmd === "close") {
                    LauncherController.hide();
                } else if (cmd === "open") {
                    ClipboardController.hide();
                    LauncherController.show();
                } else {
                    ClipboardController.hide();
                    LauncherController.toggle();
                }
                return;
            }

            if (cmd === "clipboard" || targetWidget === "clipboard" || cmd === "clip" || targetWidget === "clip") {
                if (cmd === "close") {
                    ClipboardController.hide();
                } else if (cmd === "open") {
                    LauncherController.hide();
                    ClipboardController.show();
                } else {
                    LauncherController.hide();
                    ClipboardController.toggle();
                }
                return;
            }

            if (cmd === "airplane" || targetWidget === "airplane" || cmd === "flight" || targetWidget === "flight") {
                let isAirplane = !Networking.wifiEnabled && !(Bluetooth.defaultAdapter && Bluetooth.defaultAdapter.enabled);
                if (isAirplane) {
                    Networking.wifiEnabled = true;
                    if (Bluetooth.defaultAdapter) Bluetooth.defaultAdapter.enabled = true;
                } else {
                    Networking.wifiEnabled = false;
                    if (Bluetooth.defaultAdapter) Bluetooth.defaultAdapter.enabled = false;
                }
                return;
            }

            if (cmd === "autohide" || targetWidget === "autohide") {
                let bar = Config.getSetting("bar", {});
                bar.autohide = !bar.autohide;
                Config.setSetting("bar", bar);
                return;
            }

            let effectivelyActive = masterWindow.targetActive;

            if (cmd === "close") {
                if (typeof DockAppMenuController !== "undefined") {
                    DockAppMenuController.hide();
                }
                switchWidget("hidden", "");
            } else if (cmd === "toggle" || cmd === "open") {
                if (targetWidget === effectivelyActive) {
                    let currentItem = widgetCache[targetWidget] || widgetStack.currentItem;

                    if (arg !== "" && arg !== masterWindow.activeArg) {
                        masterWindow.activeArg = arg;
                        if (currentItem && currentItem.activeMode !== undefined) {
                            currentItem.activeMode = arg;
                        }
                        if (currentItem && currentItem.gotoTab !== undefined) {
                            currentItem.gotoTab(arg);
                        }
                    } else if (cmd === "toggle") {
                        switchWidget("hidden", "");
                    }
                } else if (getLayout(targetWidget)) {
                    switchWidget(targetWidget, arg);
                }
            } else if (getLayout(cmd)) {
                let legacyArg = targetWidget;

                if (cmd === effectivelyActive) {
                    let currentItem = widgetCache[cmd] || widgetStack.currentItem;
                    if (legacyArg !== "" && currentItem && currentItem.activeMode !== undefined && currentItem.activeMode !== legacyArg) {
                        currentItem.activeMode = legacyArg;
                    } else if (legacyArg !== "" && currentItem && currentItem.gotoTab !== undefined) {
                        currentItem.gotoTab(legacyArg);
                    } else {
                        switchWidget("hidden", "");
                    }
                } else {
                    switchWidget(cmd, legacyArg);
                }
            }
        }
    }

    WlrLayershell.namespace: "qs-master"
    WlrLayershell.layer: WlrLayer.Overlay

    exclusionMode: ExclusionMode.Ignore
    focusable: true

    implicitWidth: masterWindow.screen ? masterWindow.screen.width : 0
    implicitHeight: masterWindow.screen ? masterWindow.screen.height : 0

    visible: isVisible

    mask: Region {
        item: masterWindow.isCurrentDraggable ? maskContent : topBarHole
        intersection: masterWindow.isCurrentDraggable ? Intersection.Combine : Intersection.Xor
    }

    property var rawBarSettings: (typeof Config !== "undefined" && Config.rawSettings && Config.rawSettings.bar) ? Config.rawSettings.bar : ({})
    property string barPosition: (rawBarSettings && rawBarSettings.position !== undefined) ? rawBarSettings.position : "top"
    property bool barAutohide: (rawBarSettings && rawBarSettings.autohide !== undefined) ? Boolean(rawBarSettings.autohide) : false

    readonly property bool isFullscreenActive: {
        try {
            if (typeof Hyprland !== "undefined" && Hyprland.focusedWorkspace) {
                return Boolean(Hyprland.focusedWorkspace.hasFullscreen || (Hyprland.activeToplevel && Hyprland.activeToplevel.fullscreen));
            }
        } catch (e) {}
        return false;
    }

    readonly property bool isBarEffectivelyHidden: barAutohide || isFullscreenActive
    readonly property bool screenReady: masterWindow.width >= 100 && masterWindow.height >= 100
    readonly property bool isCurrentDraggable: {
        let t = getLayout(masterWindow.currentActive);
        return Boolean(t && t.draggable) || masterWindow.currentActive === "guide";
    }

    Item {
        id: maskContent
        x: masterWindow._animX
        y: masterWindow._animY
        width: masterWindow._animW
        height: masterWindow._animH
    }

    Item {
        id: topBarHole

        property real thicknessRatio: {
            let h = (masterWindow.rawBarSettings && masterWindow.rawBarSettings.height !== undefined) ? Number(masterWindow.rawBarSettings.height) : 40;
            if (isNaN(h)) h = 40;
            return Math.max(28, Math.min(48, h)) / 40;
        }
        property int barThickness: Math.round(48 * thicknessRatio)
        property string bp: masterWindow.barPosition
        property bool activeBar: !masterWindow.isBarEffectivelyHidden

        property bool overlapTopLeft: masterWindow.currentActive !== "hidden" && masterWindow._animX < 10 && masterWindow._animY < barThickness
        property bool overlapTopRight: masterWindow.currentActive !== "hidden" && (masterWindow._animX + masterWindow._animW) > (masterWindow.width - 10) && masterWindow._animY < barThickness
        property bool overlapBottomLeft: masterWindow.currentActive !== "hidden" && masterWindow._animX < 10 && (masterWindow._animY + masterWindow._animH) > (masterWindow.height - barThickness)
        property bool overlapBottomRight: masterWindow.currentActive !== "hidden" && (masterWindow._animX + masterWindow._animW) > (masterWindow.width - 10) && (masterWindow._animY + masterWindow._animH) > (masterWindow.height - barThickness)

        x: {
            if (!activeBar) return 0;
            if (bp === "left") return 0;
            if (bp === "right") return masterWindow.width - barThickness;
            if (overlapTopLeft && bp === "top") return masterWindow._animW;
            if (overlapBottomLeft && bp === "bottom") return masterWindow._animW;
            return 0;
        }

        y: {
            if (!activeBar) return 0;
            if (bp === "top") return 0;
            if (bp === "bottom") return masterWindow.height - barThickness;
            if (overlapTopLeft && bp === "left") return masterWindow._animH;
            if (overlapTopRight && bp === "right") return masterWindow._animH;
            return 0;
        }

        width: {
            if (!activeBar) return 0;
            if (bp === "left" || bp === "right") return barThickness;
            let w = masterWindow.width;
            if (overlapTopLeft && bp === "top") w -= masterWindow._animW;
            if (overlapTopRight && bp === "top") w -= masterWindow._animW;
            if (overlapBottomLeft && bp === "bottom") w -= masterWindow._animW;
            if (overlapBottomRight && bp === "bottom") w -= masterWindow._animW;
            return Math.max(0, w);
        }

        height: {
            if (!activeBar) return 0;
            if (bp === "top" || bp === "bottom") return barThickness;
            let h = masterWindow.height;
            if (overlapTopLeft && bp === "left") h -= masterWindow._animH;
            if (overlapBottomLeft && bp === "left") h -= masterWindow._animH;
            if (overlapTopRight && bp === "right") h -= masterWindow._animH;
            if (overlapBottomRight && bp === "right") h -= masterWindow._animH;
            return Math.max(0, h);
        }
    }

    MouseArea {
        anchors.fill: parent
        enabled: masterWindow.isVisible && !masterWindow.isCurrentDraggable && !masterWindow.isClosing
        onClicked: switchWidget("hidden", "")
    }

    Item {
        id: preloaderContainer
        visible: false
    }

    property var widgetCache: ({})
    property var componentCache: ({})

    function widgetNameForItem(item) {
        for (let name in widgetCache) {
            if (widgetCache[name] === item) return name;
        }
        return null;
    }

    function ensureWidgetItem(name, t) {
        let cached = widgetCache[name];
        if (cached) return cached;

        let comp = componentCache[name];
        if (!comp) {
            comp = typeof t.comp === "string" ? Qt.createComponent(t.comp) : t.comp;
            if (comp) componentCache[name] = comp;
        }
        if (!comp) return null;

        if (comp.status === Component.Loading) return null;
        if (comp.status === Component.Error) {
            console.warn("Widget component failed to load:", name, comp.errorString());
            return null;
        }

        let item = comp.createObject(preloaderContainer);
        if (item) {
            item.visible = false;
            widgetCache[name] = item;
        }
        return item;
    }

    Component.onCompleted: {
        applyConfigSettings();
        reportWidgetState();
        // Vortex: Ajustes y el selector de fondos ya no se precargan al arrancar (quedaban
        // en memoria aunque nunca se abrieran). Se crean la primera vez que se abren.
    }

    Component.onDestruction: {
        if (typeof Caching !== "undefined" && Caching.runDir) {
            Quickshell.execDetached(["bash", "-c", "echo '{\"widget\":\"hidden\",\"screen\":\"\"}' > " + Caching.runDir + "/current_widget"]);
        }
    }

    property string targetActive: "hidden"
    property string currentActive: "hidden"
    onCurrentActiveChanged: {
        reportWidgetState();
        let prev = masterWindow._previousActive;
        masterWindow._previousActive = currentActive;
        if (masterWindow.releasableWidgets.indexOf(prev) !== -1 && prev !== currentActive) {
            masterWindow.widgetHiddenSince[prev] = Date.now();
            masterWindow.scheduleWidgetRelease();
        }
    }

    // Vortex (R2b): Ajustes y el selector de fondos son los paneles más pesados (Ajustes
    // carga ~17 pestañas para la búsqueda). Se crean al abrirlos (R2) y, si siguen
    // cerrados un minuto después, se destruyen para devolver la memoria. Al reabrirlos
    // se vuelven a crear desde el componente ya compilado (componentCache).
    // Vortex 1.7.5: todos los paneles, no solo esos dos, y a los 20 s: los demás (red, volumen,
    // reloj, música, notificaciones, sistema) se quedaban en memoria para siempre tras abrirlos
    // una vez (~10-25 MB cada uno). Reabrirlos cuesta un instante.
    readonly property var releasableWidgets: ["guide", "wallpaper", "network", "volume", "calendar", "music", "notifications", "system", "keybinds", "celular"]
    readonly property int widgetReleaseDelay: 20000
    property string _previousActive: "hidden"
    property var widgetHiddenSince: ({})

    function scheduleWidgetRelease() {
        let now = Date.now();
        let wait = -1;
        for (let i = 0; i < releasableWidgets.length; i++) {
            let name = releasableWidgets[i];
            if (!widgetCache[name] || name === currentActive || name === targetActive) continue;
            let since = widgetHiddenSince[name] !== undefined ? widgetHiddenSince[name] : now;
            let left = Math.max(1000, since + widgetReleaseDelay - now);
            if (wait < 0 || left < wait) wait = left;
        }
        if (wait < 0) {
            widgetReleaseTimer.stop();
            return;
        }
        widgetReleaseTimer.interval = wait;
        widgetReleaseTimer.restart();
    }

    // Un Process en marcha (descargar un fondo, instalar una fuente…) se mataría al
    // destruir el panel y su resultado se perdería: en ese caso se espera otra vuelta.
    function hasRunningProcess(obj) {
        if (!obj) return false;
        if (obj.running === true && obj.command !== undefined && obj.processId !== undefined) return true;
        let kids = obj.data;
        if (!kids) return false;
        for (let i = 0; i < kids.length; i++) {
            if (hasRunningProcess(kids[i])) return true;
        }
        return false;
    }

    // Vortex 1.7.5: lo pide el gestor de recursos cuando falta memoria (IPC «vortex soltarMemoria»,
    // Shell.qml): suelta ya los paneles cerrados, sin esperar sus 20 s, y recoge la basura de JS.
    function soltarMemoria() {
        let now = Date.now();
        for (let i = 0; i < releasableWidgets.length; i++) {
            if (widgetCache[releasableWidgets[i]]) widgetHiddenSince[releasableWidgets[i]] = now - widgetReleaseDelay;
        }
        releaseIdleWidgets();
        gc();
    }

    function releaseIdleWidgets() {
        let now = Date.now();
        let released = false;
        for (let i = 0; i < releasableWidgets.length; i++) {
            let name = releasableWidgets[i];
            let item = widgetCache[name];
            if (!item || name === currentActive || name === targetActive) continue;
            let since = widgetHiddenSince[name] !== undefined ? widgetHiddenSince[name] : now;
            if (now - since < widgetReleaseDelay - 500) continue;
            if (widgetStack.currentItem === item && (masterWindow.isVisible || masterWindow.isClosing)) continue;
            if (hasRunningProcess(item)) {
                widgetHiddenSince[name] = now;
                continue;
            }
            if (widgetStack.currentItem === item) widgetStack.clear(StackView.Immediate);
            delete widgetCache[name];
            delete widgetHiddenSince[name];
            item.visible = false;
            item.destroy();
            released = true;
        }
        if (released) {
            widgetGcTimer.pass = 0;
            widgetGcTimer.interval = 2000;
            widgetGcTimer.restart();
        }
        scheduleWidgetRelease();
    }

    Timer {
        id: widgetReleaseTimer
        repeat: false
        onTriggered: masterWindow.releaseIdleWidgets()
    }

    // destroy() borra los objetos al volver al bucle de eventos; luego se pide al GC de
    // JavaScript que recoja lo que dejaron, para que la memoria se devuelva de verdad.
    // Vortex: dos pasadas (a los 2 s y a los 12 s): la primera suelta los envoltorios JS
    // y eso libera objetos que solo una segunda pasada alcanza a recoger.
    Timer {
        id: widgetGcTimer
        interval: 2000
        repeat: false
        property int pass: 0
        onTriggered: {
            gc();
            if (pass === 0) {
                pass = 1;
                interval = 10000;
                restart();
            } else {
                pass = 0;
                interval = 2000;
            }
        }
    }

    onScreenChanged: {
        applyConfigSettings();
        if (currentActive !== "hidden") {
            reportWidgetState();
        }
    }

    property bool isVisible: false
    property bool isClosing: false
    property string activeArg: ""
    property bool disableMorph: true
    property int switchGeneration: 0
    property bool userMoved: false

    property int morphDuration:       300
    property int morphDurationSwitch: 300
    property int exitDuration:        180

    property real _animW: 1
    property real _animH: 1
    property real _animX: 0
    property real _animY: 0

    property real _stageW: 1
    property real _stageH: 1

    property real globalUiScale: 1.0

    Notifs.NotificationPopups {
        id: osdPopups
    }

    function applyConfigSettings() {
        let parsed = (typeof Config !== "undefined" && Config.rawSettings) ? Config.rawSettings : {};
        let sVal = undefined;

        if (parsed.general && parsed.general.uiScale !== undefined) {
            sVal = parsed.general.uiScale;
        } else if (parsed.uiScale !== undefined) {
            sVal = parsed.uiScale;
        }

        let targetScale = (sVal !== undefined && typeof sVal === "number" && !isNaN(sVal) && sVal > 0) ? sVal : 1.0;
        if (masterWindow.globalUiScale !== targetScale) {
            masterWindow.globalUiScale = targetScale;
        }

        let b = parsed.bar || {};
        masterWindow.rawBarSettings = b;
        if (b.position !== undefined) masterWindow.barPosition = b.position;
        if (b.autohide !== undefined) masterWindow.barAutohide = Boolean(b.autohide);
    }

    Connections {
        target: (typeof Config !== "undefined") ? Config : null
        function onSettingsLoaded() {
            masterWindow.applyConfigSettings();
        }
    }

    function getLayout(name) {
        let bp = masterWindow.barPosition;
        let effHidden = masterWindow.isBarEffectivelyHidden;
        let scrW = masterWindow.screen ? masterWindow.screen.width : masterWindow.width;
        let scrH = masterWindow.screen ? masterWindow.screen.height : masterWindow.height;

        let result = Registry.getLayout(name, 0, 0, scrW, scrH, masterWindow.globalUiScale, bp);
        if (!result) return null;

        let scale = masterWindow.globalUiScale || 1.0;
        let isFixed = (name === "guide" || name === "keybinds" || name === "wallpaper" || name === "notifications" || name === "system" || name === "hidden");

        if (effHidden && !isFixed) {
            let offsetAdjustment = Math.round(46 * scale);
            let adjusted = {
                w: result.w,
                h: result.h,
                rx: result.rx,
                ry: result.ry,
                comp: result.comp,
                draggable: result.draggable
            };

            if (bp === "top") {
                adjusted.ry = Math.max(Math.round(6 * scale), result.ry - offsetAdjustment);
            } else if (bp === "bottom") {
                adjusted.ry = Math.min(masterWindow.height - result.h - Math.round(6 * scale), result.ry + offsetAdjustment);
            } else if (bp === "left" && name !== "calendar") {
                adjusted.rx = Math.max(Math.round(6 * scale), result.rx - offsetAdjustment);
            } else if (bp === "right" && name !== "calendar") {
                adjusted.rx = Math.min(masterWindow.width - result.w - Math.round(6 * scale), result.rx + offsetAdjustment);
            }

            return adjusted;
        }

        return result;
    }

    function recenterX(t, dynW) {
        let originalCenterX = t.rx + Math.floor(t.w / 2);
        return Math.floor(originalCenterX - (dynW / 2));
    }

    readonly property var targetLayout: {
        if (currentActive === "hidden" || currentActive === "") return null;
        if (!screenReady) return null;

        let t = getLayout(currentActive);
        if (!t || !t.w || !t.h || t.w < 10 || t.h < 10) return null;

        if (t.rx < -t.w - 50 || t.ry < -t.h - 50 ||
            t.rx > masterWindow.width + 50 || t.ry > masterWindow.height + 50) {
            return null;
        }

        let item = widgetCache[currentActive];
        let tw = t.w, th = t.h;
        if (item) {
            if (typeof item.targetMasterWidth === "number" && !isNaN(item.targetMasterWidth) && item.targetMasterWidth >= 50) {
                tw = item.targetMasterWidth;
            }
            if (typeof item.targetMasterHeight === "number" && !isNaN(item.targetMasterHeight) && item.targetMasterHeight >= 50) {
                th = item.targetMasterHeight;
            }
        }

        let x = (tw !== t.w) ? recenterX(t, tw) : t.rx;
        return { x: x, y: t.ry, w: tw, h: th };
    }

    onTargetLayoutChanged: {
        if (!targetLayout || masterWindow.currentActive === "hidden" || !masterWindow.isVisible) return;
        if (!masterWindow.userMoved || !masterWindow.isCurrentDraggable) {
            masterWindow._animX = targetLayout.x;
            masterWindow._animY = targetLayout.y;
        }
        masterWindow._animW = targetLayout.w;
        masterWindow._animH = targetLayout.h;
        masterWindow._stageW = targetLayout.w;
        masterWindow._stageH = targetLayout.h;
    }

    onIsVisibleChanged: {
        if (isVisible) {
            widgetStack.forceActiveFocus();
        } else if (currentActive === "hidden" && !delayedClear.running) {
            for (let k in widgetCache) {
                let it = widgetCache[k];
                if (it) it.visible = false;
            }
        }
    }

    Item {
        id: animContainer
        x: masterWindow._animX
        y: masterWindow._animY
        width: masterWindow._animW
        height: masterWindow._animH
        clip: true

        DragHandler {
            id: windowDragHandler
            target: null
            acceptedButtons: Qt.LeftButton
            enabled: masterWindow.isCurrentDraggable

            property real startX: 0
            property real startY: 0

            onActiveChanged: {
                if (active) {
                    masterWindow.disableMorph = true;
                    startX = masterWindow._animX;
                    startY = masterWindow._animY;
                } else {
                    masterWindow.disableMorph = false;
                }
            }

            onTranslationChanged: {
                if (active) {
                    masterWindow.userMoved = true;
                    masterWindow._animX = startX + translation.x;
                    masterWindow._animY = startY + translation.y;
                }
            }
        }

        Item {
            id: contentStage
            width: masterWindow._stageW
            height: masterWindow._stageH
            x: (animContainer.width - width) / 2
            y: (animContainer.height - height) / 2
            transformOrigin: Item.Center

            scale: masterWindow.isVisible ? 1.0 : 0.96
            Behavior on scale {
                NumberAnimation {
                    duration: masterWindow.isVisible ? 280 : 160
                    easing.type: masterWindow.isVisible ? Easing.OutCubic : Easing.InCubic
                }
            }

            opacity: masterWindow.isVisible ? 1.0 : 0.0
            visible: masterWindow.isVisible || opacity > 0
            Behavior on opacity {
                NumberAnimation {
                    duration: masterWindow.isVisible ? 200 : 140
                    easing.type: Easing.OutCubic
                }
            }

            MouseArea { anchors.fill: parent }

            StackView {
                id: widgetStack
                anchors.fill: parent
                focus: true

                replaceEnter: null
                replaceExit: null
                pushEnter: null
                pushExit: null
                popEnter: null
                popExit: null

                Keys.onEscapePressed: (event) => {
                    switchWidget("hidden", "");
                    event.accepted = true;
                }

                onCurrentItemChanged: {
                    if (currentItem) currentItem.forceActiveFocus();
                }
            }
        }

        Behavior on x {
            enabled: !masterWindow.disableMorph
            NumberAnimation { duration: masterWindow.morphDuration; easing.type: Easing.OutCubic }
        }
        Behavior on y {
            enabled: !masterWindow.disableMorph
            NumberAnimation { duration: masterWindow.morphDuration; easing.type: Easing.OutCubic }
        }
        Behavior on width {
            enabled: !masterWindow.disableMorph
            NumberAnimation { duration: masterWindow.morphDuration; easing.type: Easing.OutCubic }
        }
        Behavior on height {
            enabled: !masterWindow.disableMorph
            NumberAnimation { duration: masterWindow.morphDuration; easing.type: Easing.OutCubic }
        }
    }

    function switchWidget(newWidget, arg) {
        masterWindow.switchGeneration++;
        let gen = masterWindow.switchGeneration;
        masterWindow.targetActive = newWidget;

        if (newWidget !== "guide") {
            masterWindow.userMoved = false;
        }

        if (delayedClear.running) {
            delayedClear.stop();
        }
        if (exitSafetyTimer.running) {
            exitSafetyTimer.stop();
        }

        if (newWidget === "hidden") {
            if (currentActive !== "hidden" && !masterWindow.isClosing) {
                let closingGen = gen;
                masterWindow.isClosing = true;
                masterWindow.targetActive = "hidden";

                let activeItem = widgetCache[currentActive] || widgetStack.currentItem;

                let finalizeClose = function() {
                    if (closingGen !== masterWindow.switchGeneration) return;
                    masterWindow.isClosing = false;
                    masterWindow.currentActive = "hidden";
                    masterWindow.morphDuration = masterWindow.exitDuration;
                    masterWindow.disableMorph = true;
                    masterWindow.isVisible = false;

                    for (let k in widgetCache) {
                        let it = widgetCache[k];
                        if (it) it.visible = false;
                    }
                    reportWidgetState();
                };

                if (activeItem && typeof activeItem.startExit === "function") {
                    exitSafetyTimer.scheduledGeneration = closingGen;
                    exitSafetyTimer.restart();
                    activeItem.startExit(finalizeClose);
                } else {
                    finalizeClose();
                }
            }
        } else {
            masterWindow.isClosing = false;
            let targetScreen = resolveTargetScreen();
            if (targetScreen && masterWindow.screen !== targetScreen) {
                masterWindow.screen = targetScreen;
            }
            executeSwitch(newWidget, arg, gen);
        }
    }

    function executeSwitch(newWidget, arg, gen) {
        if (gen !== masterWindow.switchGeneration || newWidget === "hidden") return;

        let targetScreen = resolveTargetScreen();
        if (targetScreen && masterWindow.screen !== targetScreen) {
            masterWindow.screen = targetScreen;
        }

        let t = getLayout(newWidget);
        if (!t || !t.w || !t.h || t.w < 10 || t.h < 10) {
            Qt.callLater(function() {
                if (gen === masterWindow.switchGeneration) {
                    executeSwitch(newWidget, arg, gen);
                }
            });
            return;
        }

        let cachedItem = ensureWidgetItem(newWidget, t);
        if (!cachedItem) {
            Qt.callLater(function() {
                if (gen === masterWindow.switchGeneration) {
                    executeSwitch(newWidget, arg, gen);
                }
            });
            return;
        }

        if (gen !== masterWindow.switchGeneration) return;

        if (cachedItem.targetMasterWidth !== undefined)  cachedItem.targetMasterWidth  = t.w;
        if (cachedItem.targetMasterHeight !== undefined) cachedItem.targetMasterHeight = t.h;

        let isComingFromHidden = (!masterWindow.isVisible || widgetStack.currentItem === null || masterWindow.currentActive === "hidden");

        masterWindow.currentActive = newWidget;
        masterWindow.activeArg = arg;

        let finalW = (typeof cachedItem.targetMasterWidth === "number" && cachedItem.targetMasterWidth >= 50) ? cachedItem.targetMasterWidth : t.w;
        let finalH = (typeof cachedItem.targetMasterHeight === "number" && cachedItem.targetMasterHeight >= 50) ? cachedItem.targetMasterHeight : t.h;
        let finalX = (finalW !== t.w) ? recenterX(t, finalW) : t.rx;
        let finalY = t.ry;

        if (!masterWindow.userMoved || !masterWindow.isCurrentDraggable) {
            masterWindow._animX = finalX;
            masterWindow._animY = finalY;
        }
        masterWindow._animW = finalW;
        masterWindow._animH = finalH;
        masterWindow._stageW = finalW;
        masterWindow._stageH = finalH;

        if (isComingFromHidden) {
            masterWindow.disableMorph = true;
        } else {
            masterWindow.morphDuration = masterWindow.morphDurationSwitch;
            masterWindow.disableMorph = false;
        }

        if (newWidget === "wallpaper" && cachedItem.widgetArg !== undefined) cachedItem.widgetArg = arg;
        if (newWidget === "wallpaper" && cachedItem.refreshForDisplay !== undefined) cachedItem.refreshForDisplay();
        if (arg !== "" && cachedItem.activeMode !== undefined) cachedItem.activeMode = arg;
        if (arg !== "" && cachedItem.gotoTab !== undefined) cachedItem.gotoTab(arg);

        if (widgetStack.currentItem !== cachedItem) {
            widgetStack.replace(cachedItem, {}, StackView.Immediate);
        }

        for (let k in widgetCache) {
            let it = widgetCache[k];
            if (it && it !== cachedItem) {
                it.visible = false;
            }
        }
        cachedItem.visible = true;

        masterWindow.isVisible = true;

        if (isComingFromHidden) {
            Qt.callLater(function() {
                if (gen === masterWindow.switchGeneration && masterWindow.isVisible && masterWindow.currentActive === newWidget) {
                    masterWindow.disableMorph = false;
                }
            });
        }
    }

    Timer {
        id: exitSafetyTimer
        interval: 450
        property int scheduledGeneration: -1
        onTriggered: {
            if (scheduledGeneration === masterWindow.switchGeneration && masterWindow.isClosing) {
                masterWindow.isClosing = false;
                masterWindow.currentActive = "hidden";
                masterWindow.morphDuration = masterWindow.exitDuration;
                masterWindow.disableMorph = true;
                masterWindow.isVisible = false;
                for (let k in widgetCache) {
                    let it = widgetCache[k];
                    if (it) it.visible = false;
                }
                reportWidgetState();
            }
        }
    }

    Timer {
        id: delayedClear
        interval: 200
        property int scheduledGeneration: -1
        onTriggered: {
            if (masterWindow.currentActive === "hidden" && scheduledGeneration === masterWindow.switchGeneration) {
                masterWindow.disableMorph = true;
                for (let k in widgetCache) {
                    let it = widgetCache[k];
                    if (it) it.visible = false;
                }
            }
        }
    }
}
