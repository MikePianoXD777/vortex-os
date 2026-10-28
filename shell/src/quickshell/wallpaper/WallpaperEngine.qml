import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Wayland
import Quickshell.Io
import Quickshell.Hyprland
import Quickshell.Services.UPower
import QtMultimedia
import "../"

ShellRoot {
    id: globalRoot

    Variants {
        id: root
        model: Quickshell.screens

        delegate: Component {
            PanelWindow {
                id: barWindow

                required property var modelData
                screen: modelData

                WlrLayershell.namespace: "wallpaper-bg"
                WlrLayershell.layer: WlrLayer.Background

                focusable: false
                exclusionMode: ExclusionMode.Ignore
                color: "#0a0a0f"

                anchors { top: true; bottom: true; left: true; right: true }

                readonly property string wpCacheDir: Caching.getCacheDir("wallpaper")
                readonly property string wpStatePath: wpCacheDir + "/current_" + barWindow.screen.name
                readonly property string wpCopyDir: wpCacheDir + "/copy_" + barWindow.screen.name
                readonly property string wpSnapshotPath: wpCacheDir + "/current_wallpaper.png"
                readonly property string wpMonitorSnapshotPath: wpCacheDir + "/current_wallpaper_" + barWindow.screen.name + ".png"

                property string currentWallpaperPath: ""
                property string originalFileName: ""
                property int activeLayer: 0
                property string pathA: ""
                property bool isVideoA: false
                property string pathB: ""
                property bool isVideoB: false
                property bool playbackPaused: false

                // Vortex: fondos animados (Ajustes > Tema > Fondos animados; «animatedWallpapers» en
                // settings.json). Se pausan solos (se queda el cuadro) si están apagados, si hay ventanas
                // en el escritorio de este monitor, con la laptop a batería o con el equipo inactivo; y se reproduce una copia
                // optimizada (scripts/wallpaper/optimizar_video.sh) en cuanto está lista.
                property int animRev: 0
                readonly property var animCfg: {
                    let dummy = barWindow.animRev;
                    let a = (typeof Config !== "undefined") ? Config.getSetting("animatedWallpapers", {}) : {};
                    return (a && typeof a === "object") ? a : {};
                }
                readonly property bool animEnabled: animCfg.enabled !== false
                readonly property bool animPauseCovered: animCfg.pauseCovered !== false
                readonly property bool animPauseBattery: animCfg.pauseOnBattery !== false
                readonly property bool animOptimize: animCfg.optimize !== false
                readonly property var hyprMonitor: Hyprland.monitorFor(barWindow.screen)
                readonly property bool covered: {
                    let ws = barWindow.hyprMonitor ? barWindow.hyprMonitor.activeWorkspace : null;
                    return !!(ws && ws.toplevels && ws.toplevels.values && ws.toplevels.values.length > 0);
                }
                readonly property bool onBattery: UPower.onBattery
                // También tras 3 min sin tocar el equipo (pantalla bloqueada o apagada): nadie lo ve.
                IdleMonitor {
                    id: animIdle
                    timeout: 180
                    respectInhibitors: true
                }
                readonly property bool autoPaused: !animEnabled || (animPauseCovered && covered) || (animPauseBattery && onBattery) || animIdle.isIdle
                readonly property bool effectivePaused: playbackPaused || autoPaused
                property var optimizedReady: ({})       // video original -> copia optimizada (ya hecha)
                property var optimizedApplied: ({})     // las que ya se reproducen (se cambian sin que se vea)
                readonly property string videoSrcA: (isVideoA && pathA) ? (optimizedApplied[pathA] || pathA) : ""
                readonly property string videoSrcB: (isVideoB && pathB) ? (optimizedApplied[pathB] || pathB) : ""

                Connections {
                    target: (typeof Config !== "undefined") ? Config : null
                    function onSettingsLoaded() { barWindow.animRev++; }
                }

                onEffectivePausedChanged: {
                    if (effectivePaused) {
                        barWindow.applyOptimized();
                        barWindow.pauseA();
                        barWindow.pauseB();
                    } else {
                        if (barWindow.activeLayer === 0 && barWindow.isVideoA) barWindow.playA();
                        if (barWindow.activeLayer === 1 && barWindow.isVideoB) barWindow.playB();
                    }
                }

                // La copia optimizada entra cuando no se ve el cambio: con el fondo pausado o en una capa que
                // no es la activa. Si no, espera a la próxima pausa.
                function applyOptimized() {
                    let ready = barWindow.optimizedReady;
                    let applied = Object.assign({}, barWindow.optimizedApplied);
                    let changed = false;
                    for (let orig in ready) {
                        if (applied[orig] === ready[orig]) continue;
                        let visible = (barWindow.activeLayer === 0 && barWindow.pathA === orig) ||
                                      (barWindow.activeLayer === 1 && barWindow.pathB === orig);
                        if (visible && !barWindow.effectivePaused) continue;
                        applied[orig] = ready[orig];
                        changed = true;
                    }
                    if (changed) barWindow.optimizedApplied = applied;
                }

                function requestOptimized(path) {
                    if (!barWindow.animOptimize || !path || barWindow.optimizedReady[path] !== undefined) return;
                    if (videoOptimizer.running) {
                        videoOptimizer.queued = path;
                        return;
                    }
                    videoOptimizer.targetPath = path;
                    videoOptimizer.running = true;
                }

                Process {
                    id: videoOptimizer
                    running: false
                    property string targetPath: ""
                    property string queued: ""
                    readonly property real dpr: (barWindow.screen && barWindow.screen.devicePixelRatio) ? barWindow.screen.devicePixelRatio : 1
                    command: [
                        "bash", Caching.serpantinumDir + "/scripts/wallpaper/optimizar_video.sh", targetPath,
                        String(Math.round((barWindow.screen ? barWindow.screen.width : 1920) * dpr)),
                        String(Math.round((barWindow.screen ? barWindow.screen.height : 1080) * dpr)),
                        barWindow.wpCacheDir + "/animados"
                    ]
                    stdout: StdioCollector {
                        onStreamFinished: {
                            let lines = this.text.trim().split("\n");
                            let out = lines.length > 0 ? lines[lines.length - 1].trim() : "";
                            let orig = videoOptimizer.targetPath;
                            if (orig !== "") {
                                let ready = Object.assign({}, barWindow.optimizedReady);
                                ready[orig] = (out !== "" && out !== orig) ? out : orig;
                                barWindow.optimizedReady = ready;
                                barWindow.applyOptimized();
                            }
                        }
                    }
                    onExited: {
                        if (queued !== "") {
                            let next = queued;
                            queued = "";
                            barWindow.requestOptimized(next);
                        }
                    }
                }

                onPathAChanged: if (isVideoA && pathA) requestOptimized(pathA)
                onPathBChanged: if (isVideoB && pathB) requestOptimized(pathB)
                onIsVideoAChanged: if (isVideoA && pathA) requestOptimized(pathA)
                onIsVideoBChanged: if (isVideoB && pathB) requestOptimized(pathB)
                property bool snapReadyA: false
                property bool snapReadyB: false
                property int snapRevA: 0
                property int snapRevB: 0
                readonly property string layerSnapshotPathA: wpCacheDir + "/layer_" + barWindow.screen.name + "_a.png"
                readonly property string layerSnapshotPathB: wpCacheDir + "/layer_" + barWindow.screen.name + "_b.png"

                readonly property int fadeDuration: 1000
                readonly property int maskDuration: 1600
                property real transitionProgress: 1.0
                property bool isPreloading: false
                property int activeTransitionType: 0
                property bool wipeIsVertical: false
                property int swipeDirection: 0
                property real transitionOriginX: 0.5
                property real transitionOriginY: 0.5
                property bool isInitialLoad: true

                Component.onCompleted: {
                    restorePoller.running = true;
                    Wallpaper.registerEngine(barWindow.screen ? barWindow.screen.name : "", barWindow);
                }

                Component.onDestruction: Wallpaper.unregisterEngine(barWindow.screen ? barWindow.screen.name : "", barWindow)

                onCurrentWallpaperPathChanged: {
                    if (barWindow.screen && barWindow.screen.name) {
                        let map = Object.assign({}, Wallpaper.screenWallpaperPaths);
                        map[barWindow.screen.name] = currentWallpaperPath;
                        Wallpaper.screenWallpaperPaths = map;
                    }
                }

                onOriginalFileNameChanged: {
                    if (barWindow.screen && barWindow.screen.name) {
                        let map = Object.assign({}, Wallpaper.screenWallpapers);
                        map[barWindow.screen.name] = originalFileName;
                        Wallpaper.screenWallpapers = map;
                    }
                }

                Connections {
                    target: Wallpaper

                    function onWallpaperChanged(screenName, path, transition) {
                        if (screenName === "all" || screenName === barWindow.screen.name)
                            barWindow.changeWallpaper(path, transition);
                    }

                    function onPlaybackChanged(screenName, state) {
                        if (screenName === "all" || screenName === barWindow.screen.name) {
                            if (state === "pause") {
                                barWindow.playbackPaused = true;
                                barWindow.stopA();
                                barWindow.stopB();
                            } else if (state === "play") {
                                barWindow.playbackPaused = false;
                                if (barWindow.activeLayer === 0 && barWindow.isVideoA) barWindow.playA();
                                if (barWindow.activeLayer === 1 && barWindow.isVideoB) barWindow.playB();
                            }
                        }
                    }

                    function onWallpaperCleared(screenName) {
                        if (screenName === "all" || screenName === barWindow.screen.name) {
                            barWindow.currentWallpaperPath = "";
                            barWindow.pathA = "";
                            barWindow.pathB = "";
                            barWindow.isVideoA = false;
                            barWindow.isVideoB = false;
                            barWindow.originalFileName = "";
                            barWindow.stopA();
                            barWindow.stopB();
                            Quickshell.execDetached(["bash", "-c", "rm -f '" + barWindow.wpStatePath + "' '" + barWindow.wpStatePath + "_name'"]);
                        }
                    }
                }

                Process {
                    id: restorePoller
                    running: false
                    command: [
                        "bash", "-c",
                        "F1='" + barWindow.wpStatePath + "'; F2='" + barWindow.wpStatePath + "_name'; [ -f \"$F1\" ] && cat \"$F1\" || true; echo '---SPLIT---'; [ -f \"$F2\" ] && cat \"$F2\" || true"
                    ]
                    stdout: StdioCollector {
                        onStreamFinished: {
                            let parts = this.text.split("---SPLIT---");
                            let savedPath = parts[0] ? parts[0].trim() : "";
                            let savedName = parts[1] ? parts[1].trim() : "";
                            if (savedPath !== "") {
                                barWindow.originalFileName = savedName;
                                barWindow._loadNew(savedPath, false);
                                if (barWindow.isVideo(savedPath)) {
                                    videoSnapshotProcess.targetPath = savedPath;
                                    videoSnapshotProcess.running = false;
                                    videoSnapshotProcess.running = true;
                                }
                                if (savedName !== "") {
                                    let histFile = barWindow.wpCacheDir + "/history.txt";
                                    Quickshell.execDetached(["bash", "-c",
                                        "HIST='" + histFile + "'; if [ -f \"$HIST\" ]; then if [ \"$(head -n 1 \"$HIST\" 2>/dev/null)\" != '" + savedName + "' ]; then grep -v -F -x '" + savedName + "' \"$HIST\" > \"$HIST.tmp\" 2>/dev/null || true; printf '%s\n' '" + savedName + "' | cat - \"$HIST.tmp\" > \"$HIST\"; rm -f \"$HIST.tmp\"; fi; else printf '%s\n' '" + savedName + "' > \"$HIST\"; fi"
                                    ]);
                                }
                            }
                        }
                    }
                }

                Timer {
                    id: videoWarmUpTimer
                    interval: 250
                    repeat: false
                    onTriggered: barWindow.triggerTransition()
                }

                Process {
                    id: layerSnapshotA
                    running: false
                    property string targetPath: ""
                    command: [
                        "bash", "-c",
                        "ffmpeg -y -hide_banner -loglevel error -ss 00:00:01 -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null || ffmpeg -y -hide_banner -loglevel error -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null",
                        "_",
                        targetPath,
                        barWindow.layerSnapshotPathA
                    ]
                    onExited: exitCode => {
                        if (exitCode === 0) {
                            barWindow.snapReadyA = true;
                            barWindow.snapRevA++;
                        }
                    }
                }

                Process {
                    id: layerSnapshotB
                    running: false
                    property string targetPath: ""
                    command: [
                        "bash", "-c",
                        "ffmpeg -y -hide_banner -loglevel error -ss 00:00:01 -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null || ffmpeg -y -hide_banner -loglevel error -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null",
                        "_",
                        targetPath,
                        barWindow.layerSnapshotPathB
                    ]
                    onExited: exitCode => {
                        if (exitCode === 0) {
                            barWindow.snapReadyB = true;
                            barWindow.snapRevB++;
                        }
                    }
                }

                Process {
                    id: videoSnapshotProcess
                    running: false
                    property string targetPath: ""
                    command: [
                        "bash", "-c",
                        "ffmpeg -y -hide_banner -loglevel error -ss 00:00:01 -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null || ffmpeg -y -hide_banner -loglevel error -i \"$1\" -frames:v 1 -q:v 2 \"$2\" 2>/dev/null; cp -f \"$2\" \"$3\" 2>/dev/null || true",
                        "_",
                        targetPath,
                        barWindow.wpSnapshotPath,
                        barWindow.wpMonitorSnapshotPath
                    ]
                    onExited: exitCode => {
                        if (exitCode === 0 && typeof Matugen !== "undefined" && typeof Matugen.generate === "function") {
                            Matugen.generate(barWindow.wpSnapshotPath);
                        }
                    }
                }

                function snapshotLayer(slot, path) {
                    let proc = slot === 0 ? layerSnapshotA : layerSnapshotB;
                    if (slot === 0) barWindow.snapReadyA = false;
                    else barWindow.snapReadyB = false;
                    proc.targetPath = path;
                    proc.running = false;
                    proc.running = true;
                }

                function isVideo(p) {
                    let lp = p.toLowerCase();
                    return /\.(mp4|mkv|mov|webm|m4v|avi|wmv|flv|ogv|mpg|mpeg|3gp)$/.test(lp);
                }

                function isSupportedMedia(p) {
                    if (!p) return false;
                    let lp = p.toLowerCase();
                    return lp.endsWith(".jpg") || lp.endsWith(".jpeg") || lp.endsWith(".png") ||
                           lp.endsWith(".webp") || lp.endsWith(".gif") || lp.endsWith(".bmp") ||
                           lp.endsWith(".avif") || barWindow.isVideo(p);
                }

                function playA() {
                    if (videoLoaderA.item && typeof videoLoaderA.item.play === "function") {
                        // Pausado: carga el video y se queda en el primer cuadro (no en negro).
                        if (barWindow.effectivePaused) videoLoaderA.item.pause();
                        else videoLoaderA.item.play();
                    }
                }

                function pauseA() {
                    if (videoLoaderA.item && typeof videoLoaderA.item.pause === "function") {
                        videoLoaderA.item.pause();
                    }
                }

                function stopA() {
                    if (videoLoaderA.item && typeof videoLoaderA.item.stop === "function") {
                        videoLoaderA.item.stop();
                    }
                }

                function playB() {
                    if (videoLoaderB.item && typeof videoLoaderB.item.play === "function") {
                        if (barWindow.effectivePaused) videoLoaderB.item.pause();
                        else videoLoaderB.item.play();
                    }
                }

                function pauseB() {
                    if (videoLoaderB.item && typeof videoLoaderB.item.pause === "function") {
                        videoLoaderB.item.pause();
                    }
                }

                function stopB() {
                    if (videoLoaderB.item && typeof videoLoaderB.item.stop === "function") {
                        videoLoaderB.item.stop();
                    }
                }

                function triggerTransition() {
                    videoWarmUpTimer.stop();
                    barWindow.isPreloading = false;
                    transitionAnim.restart();
                }

                function _loadNew(path, force) {
                    if (!path) return;
                    if (!force && path === barWindow.currentWallpaperPath) return;

                    let cleanPath = String(path).trim();
                    let vid = barWindow.isVideo(cleanPath);
                    if (vid) barWindow.snapshotLayer(barWindow.activeLayer === 1 ? 0 : 1, cleanPath);

                    let slash = cleanPath.lastIndexOf("/");
                    let filename = cleanPath.substring(slash + 1);
                    if (!filename.startsWith("wallpaper.")) {
                        barWindow.originalFileName = filename;
                    }

                    if (barWindow.isInitialLoad) {
                        barWindow.isInitialLoad = false;
                        barWindow.transitionProgress = 1.0;
                        barWindow.isPreloading = false;
                        if (barWindow.activeLayer === 1) {
                            barWindow.pathA = cleanPath;
                            barWindow.isVideoA = vid;
                            barWindow.activeLayer = 0;
                            if (vid) barWindow.playA();
                        } else {
                            barWindow.pathB = cleanPath;
                            barWindow.isVideoB = vid;
                            barWindow.activeLayer = 1;
                            if (vid) barWindow.playB();
                        }
                        barWindow.currentWallpaperPath = cleanPath;
                        return;
                    }

                    transitionAnim.stop();
                    videoWarmUpTimer.stop();
                    barWindow.transitionProgress = 0.0;
                    barWindow.isPreloading = true;

                    if (barWindow.activeLayer === 1) {
                        barWindow.pathA = cleanPath;
                        barWindow.isVideoA = vid;
                        barWindow.activeLayer = 0;
                        if (vid) {
                            barWindow.playA();
                            videoWarmUpTimer.restart();
                        } else {
                            barWindow.triggerTransition();
                        }
                    } else {
                        barWindow.pathB = cleanPath;
                        barWindow.isVideoB = vid;
                        barWindow.activeLayer = 1;
                        if (vid) {
                            barWindow.playB();
                            videoWarmUpTimer.restart();
                        } else {
                            barWindow.triggerTransition();
                        }
                    }

                    barWindow.currentWallpaperPath = cleanPath;
                }

                function changeWallpaper(path, ttype) {
                    if (!path) return;

                    let isInterrupted = transitionAnim.running && barWindow.transitionProgress > 0.1 && barWindow.transitionProgress < 0.85;

                    barWindow.isInitialLoad = false;

                    let fromMenu = false;
                    let customOriginX = -1;
                    let customOriginY = -1;
                    let chosenType = -1;

                    if (typeof ttype === "object" && ttype !== null) {
                        if (ttype.type !== undefined) chosenType = Number(ttype.type);
                        if (ttype.originX !== undefined) customOriginX = Number(ttype.originX);
                        if (ttype.originY !== undefined) customOriginY = Number(ttype.originY);
                        fromMenu = true;
                    } else if (typeof ttype === "string" && ttype.indexOf("circle") !== -1) {
                        chosenType = 2;
                        let parts = ttype.split(":");
                        if (parts.length >= 3) {
                            customOriginX = parseFloat(parts[1]);
                            customOriginY = parseFloat(parts[2]);
                        }
                        fromMenu = true;
                    } else if (typeof ttype === "number" && ttype >= 0) {
                        chosenType = ttype;
                    }

                    if (typeof DesktopMenuController !== "undefined" && DesktopMenuController.isMenuShuffle) {
                        let isTargetScreen = !DesktopMenuController.screen || !DesktopMenuController.screen.name || (barWindow.screen && DesktopMenuController.screen.name === barWindow.screen.name);
                        if (isTargetScreen) {
                            fromMenu = true;
                            chosenType = 2;
                            customOriginX = DesktopMenuController.menuOriginX;
                            customOriginY = DesktopMenuController.menuOriginY;
                        }
                    }

                    if (isInterrupted) {
                        barWindow.activeTransitionType = 0;
                    } else if (chosenType >= 0) {
                        barWindow.activeTransitionType = chosenType;
                    } else {
                        barWindow.activeTransitionType = Math.floor(Math.random() * 4);
                    }

                    barWindow.wipeIsVertical = Math.random() < 0.5;
                    barWindow.swipeDirection = Math.floor(Math.random() * 8);

                    if (fromMenu && customOriginX >= 0 && customOriginY >= 0) {
                        barWindow.transitionOriginX = customOriginX;
                        barWindow.transitionOriginY = customOriginY;
                    } else {
                        barWindow.transitionOriginX = 0.15 + Math.random() * 0.70;
                        barWindow.transitionOriginY = 0.15 + Math.random() * 0.70;
                    }

                    if (typeof DesktopMenuController !== "undefined") {
                        DesktopMenuController.isMenuShuffle = false;
                    }

                    let cleanPath = String(path).trim();
                    let slash = cleanPath.lastIndexOf("/");
                    let origName = cleanPath.substring(slash + 1);
                    let dot = cleanPath.lastIndexOf(".");
                    let ext = (dot !== -1 && dot > slash) ? cleanPath.substring(dot) : "";
                    let dest = wpCopyDir + "/wallpaper" + ext;
                    let histFile = wpCacheDir + "/history.txt";
                    let vid = barWindow.isVideo(cleanPath);
                    let snapshotPath = barWindow.wpSnapshotPath;
                    let monSnapshotPath = barWindow.wpMonitorSnapshotPath;
                    Quickshell.execDetached(["bash", "-c",
                        "mkdir -p '" + wpCopyDir + "'" +
                        " && printf '%s' '" + cleanPath + "' > '" + wpStatePath + "'" +
                        " && printf '%s' '" + origName + "' > '" + wpStatePath + "_name'" +
                        " && cp -f '" + cleanPath + "' '" + dest + "'" +
                        (vid ? "" : " && cp -f '" + cleanPath + "' '" + snapshotPath + "' && cp -f '" + cleanPath + "' '" + monSnapshotPath + "'") +
                        " && ( HIST='" + histFile + "'; if [ -f \"$HIST\" ]; then grep -v -F -x '" + origName + "' \"$HIST\" > \"$HIST.tmp\" 2>/dev/null || true; printf '%s\n' '" + origName + "' | cat - \"$HIST.tmp\" > \"$HIST\"; rm -f \"$HIST.tmp\"; else printf '%s\n' '" + origName + "' > \"$HIST\"; fi )"
                    ]);

                    if (vid) {
                        videoSnapshotProcess.targetPath = cleanPath;
                        videoSnapshotProcess.running = false;
                        videoSnapshotProcess.running = true;
                    } else {
                        if (typeof Matugen !== "undefined" && typeof Matugen.generate === "function") {
                            Matugen.generate(cleanPath);
                        }
                    }

                    barWindow._loadNew(cleanPath, true);
                }

                function handleDrop(drop) {
                    let raw = "";
                    if (drop.hasUrls && drop.urls.length > 0) {
                        raw = drop.urls[0].toString();
                    } else if (drop.hasText && drop.text.length > 0) {
                        let lines = drop.text.trim().split("\n");
                        raw = lines[0].trim();
                    }
                    if (!raw) return;

                    let cleanPath = decodeURIComponent(raw.replace(/^file:\/\//, "")).trim();
                    if (!barWindow.isSupportedMedia(cleanPath)) return;

                    let scrW = barWindow.width > 0 ? barWindow.width : (barWindow.screen && barWindow.screen.geometry ? barWindow.screen.geometry.width : 1920);
                    let scrH = barWindow.height > 0 ? barWindow.height : (barWindow.screen && barWindow.screen.geometry ? barWindow.screen.geometry.height : 1080);
                    let normX = scrW > 0 ? Math.max(0.0, Math.min(1.0, drop.x / scrW)) : 0.5;
                    let normY = scrH > 0 ? Math.max(0.0, Math.min(1.0, drop.y / scrH)) : 0.5;

                    barWindow.changeWallpaper(cleanPath, {
                        type: 2,
                        originX: normX,
                        originY: normY
                    });
                }

                NumberAnimation {
                    id: transitionAnim
                    target: barWindow
                    property: "transitionProgress"
                    from: 0.0
                    to: 1.0
                    duration: barWindow.activeTransitionType === 0 ? barWindow.fadeDuration : barWindow.maskDuration
                    easing.type: Easing.InOutCubic

                    onFinished: {
                        if (barWindow.activeLayer === 0) {
                            barWindow.stopB();
                            barWindow.pathB = "";
                            barWindow.isVideoB = false;
                        } else {
                            barWindow.stopA();
                            barWindow.pathA = "";
                            barWindow.isVideoA = false;
                        }
                    }
                }

                Component {
                    id: videoLayerCompA
                    Item {
                        anchors.fill: parent
                        function play() { playerA.play(); }
                        function pause() { playerA.pause(); }
                        function stop() { playerA.stop(); }

                        MediaPlayer {
                            id: playerA
                            source: barWindow.videoSrcA ? "file://" + barWindow.videoSrcA : ""
                            videoOutput: videoOutputA
                            loops: MediaPlayer.Infinite
                            onMediaStatusChanged: {
                                if ((mediaStatus === MediaPlayer.LoadedMedia || mediaStatus === MediaPlayer.BufferedMedia) && barWindow.activeLayer === 0 && barWindow.isVideoA) {
                                    if (barWindow.effectivePaused) playerA.pause();
                                    else playerA.play();
                                }
                            }
                        }

                        VideoOutput {
                            id: videoOutputA
                            anchors.fill: parent
                            fillMode: VideoOutput.PreserveAspectCrop
                        }
                    }
                }

                Component {
                    id: videoLayerCompB
                    Item {
                        anchors.fill: parent
                        function play() { playerB.play(); }
                        function pause() { playerB.pause(); }
                        function stop() { playerB.stop(); }

                        MediaPlayer {
                            id: playerB
                            source: barWindow.videoSrcB ? "file://" + barWindow.videoSrcB : ""
                            videoOutput: videoOutputB
                            loops: MediaPlayer.Infinite
                            onMediaStatusChanged: {
                                if ((mediaStatus === MediaPlayer.LoadedMedia || mediaStatus === MediaPlayer.BufferedMedia) && barWindow.activeLayer === 1 && barWindow.isVideoB) {
                                    if (barWindow.effectivePaused) playerB.pause();
                                    else playerB.play();
                                }
                            }
                        }

                        VideoOutput {
                            id: videoOutputB
                            anchors.fill: parent
                            fillMode: VideoOutput.PreserveAspectCrop
                        }
                    }
                }

                Item {
                    id: scene
                    anchors.fill: parent
                    clip: true

                    ShaderEffect {
                        id: transitionShaderA
                        anchors.fill: parent
                        visible: false
                        layer.enabled: layerA.layer.enabled

                        property vector2d itemSize: Qt.vector2d(width, height)
                        property real progress: barWindow.transitionProgress
                        property real transitionType: barWindow.activeTransitionType
                        property vector2d origin: Qt.vector2d(barWindow.transitionOriginX, barWindow.transitionOriginY)
                        property vector4d params: Qt.vector4d(barWindow.wipeIsVertical ? 1.0 : 0.0, barWindow.swipeDirection, 0.0, 0.0)

                        fragmentShader: "file://" + Caching.serpantinumDir + "/assets/shaders/transitions/wallpaper_transition.frag.qsb"
                    }

                    Item {
                        id: layerA
                        width: parent.width
                        height: parent.height

                        readonly property bool isIncoming: barWindow.activeLayer === 0
                        readonly property real p: barWindow.transitionProgress

                        z: isIncoming ? 2 : 1
                        visible: isIncoming || p < 1.0

                        opacity: {
                            if (barWindow.isPreloading && isIncoming) return 0.0;
                            if (barWindow.activeTransitionType !== 0) return 1.0;
                            return isIncoming ? p : 1.0 - p;
                        }

                        layer.enabled: barWindow.activeTransitionType !== 0 && isIncoming && !barWindow.isPreloading && p < 1.0
                        layer.effect: MultiEffect {
                            maskEnabled: true
                            maskSource: transitionShaderA
                            maskThresholdMin: 0.0
                            maskSpreadAtMin: 0.0
                        }

                        Image {
                            id: imgA
                            anchors.fill: parent
                            // Vortex 1.7.5: sin tamaño todavía (al arrancar) se decodificaba la foto completa
                            // (una 4K son ~33 MB) antes de ajustarla a la pantalla; ahora espera al tamaño. Y
                            // sin caché: cada fondo que pasaba por aquí se quedaba guardado.
                            source: parent.width > 0 && !barWindow.isVideoA && barWindow.pathA ? "file://" + barWindow.pathA : ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            visible: !barWindow.isVideoA && barWindow.pathA !== ""
                            cache: false
                            sourceSize.width: parent.width > 0 ? Math.ceil(parent.width * (Screen.devicePixelRatio || 1)) : 0
                            sourceSize.height: parent.height > 0 ? Math.ceil(parent.height * (Screen.devicePixelRatio || 1)) : 0
                        }

                        Loader {
                            id: videoLoaderA
                            anchors.fill: parent
                            active: barWindow.isVideoA && barWindow.pathA !== ""
                            asynchronous: false
                            sourceComponent: videoLayerCompA
                            visible: barWindow.isVideoA
                            onLoaded: {
                                if (item && barWindow.activeLayer === 0 && barWindow.isVideoA) {
                                    if (barWindow.effectivePaused) item.pause();
                                    else item.play();
                                }
                            }
                        }
                    }

                    ShaderEffect {
                        id: transitionShaderB
                        anchors.fill: parent
                        visible: false
                        layer.enabled: layerB.layer.enabled

                        property vector2d itemSize: Qt.vector2d(width, height)
                        property real progress: barWindow.transitionProgress
                        property real transitionType: barWindow.activeTransitionType
                        property vector2d origin: Qt.vector2d(barWindow.transitionOriginX, barWindow.transitionOriginY)
                        property vector4d params: Qt.vector4d(barWindow.wipeIsVertical ? 1.0 : 0.0, barWindow.swipeDirection, 0.0, 0.0)

                        fragmentShader: "file://" + Caching.serpantinumDir + "/assets/shaders/transitions/wallpaper_transition.frag.qsb"
                    }

                    Item {
                        id: layerB
                        width: parent.width
                        height: parent.height

                        readonly property bool isIncoming: barWindow.activeLayer === 1
                        readonly property real p: barWindow.transitionProgress

                        z: isIncoming ? 2 : 1
                        visible: isIncoming || p < 1.0

                        opacity: {
                            if (barWindow.isPreloading && isIncoming) return 0.0;
                            if (barWindow.activeTransitionType !== 0) return 1.0;
                            return isIncoming ? p : 1.0 - p;
                        }

                        layer.enabled: barWindow.activeTransitionType !== 0 && isIncoming && !barWindow.isPreloading && p < 1.0
                        layer.effect: MultiEffect {
                            maskEnabled: true
                            maskSource: transitionShaderB
                            maskThresholdMin: 0.0
                            maskSpreadAtMin: 0.0
                        }

                        Image {
                            id: imgB
                            anchors.fill: parent
                            // Vortex 1.7.5: sin tamaño todavía (al arrancar) se decodificaba la foto completa
                            // (una 4K son ~33 MB) antes de ajustarla a la pantalla; ahora espera al tamaño. Y
                            // sin caché: cada fondo que pasaba por aquí se quedaba guardado.
                            source: parent.width > 0 && !barWindow.isVideoB && barWindow.pathB ? "file://" + barWindow.pathB : ""
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                            visible: !barWindow.isVideoB && barWindow.pathB !== ""
                            cache: false
                            sourceSize.width: parent.width > 0 ? Math.ceil(parent.width * (Screen.devicePixelRatio || 1)) : 0
                            sourceSize.height: parent.height > 0 ? Math.ceil(parent.height * (Screen.devicePixelRatio || 1)) : 0
                        }

                        Loader {
                            id: videoLoaderB
                            anchors.fill: parent
                            active: barWindow.isVideoB && barWindow.pathB !== ""
                            asynchronous: false
                            sourceComponent: videoLayerCompB
                            visible: barWindow.isVideoB
                            onLoaded: {
                                if (item && barWindow.activeLayer === 1 && barWindow.isVideoB) {
                                    if (barWindow.effectivePaused) item.pause();
                                    else item.play();
                                }
                            }
                        }
                    }
                }

                DropArea {
                    anchors.fill: parent

                    onEntered: drag => {
                        if (drag.hasUrls || drag.hasText) {
                            if (typeof drag.acceptProposedAction === "function") {
                                drag.acceptProposedAction();
                            } else if (typeof drag.accept === "function") {
                                drag.accept();
                            }
                            drag.accepted = true;
                        }
                    }

                    onDropped: drop => {
                        barWindow.handleDrop(drop);
                        if (typeof drop.acceptProposedAction === "function") {
                            drop.acceptProposedAction();
                        } else if (typeof drop.accept === "function") {
                            drop.accept();
                        }
                        drop.accepted = true;
                    }

                    MouseArea {
                        anchors.fill: parent
                        acceptedButtons: Qt.RightButton | Qt.LeftButton
                        onClicked: mouse => {
                            if (mouse.button === Qt.RightButton) {
                                DesktopMenuController.toggle(barWindow.screen, mouse.x, mouse.y, "desktop");
                            } else {
                                DesktopMenuController.hide();
                            }
                        }
                    }
                }
            }
        }
    }
}
