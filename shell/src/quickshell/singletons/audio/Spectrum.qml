pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "../../"

Item {
    id: root

    property int barCount: 64
    readonly property var emptyLevels: {
        let arr = [];
        for (let i = 0; i < barCount; i++) arr.push(0.0);
        return Object.freeze(arr);
    }
    property var barLevels: emptyLevels
    property int activeConsumers: 0
    property bool isPlaying: MprisController.isPlaying || Audio.hasActiveStream
    property bool isDecaying: false
    property bool isRestarting: false
    property int consecutiveCrashes: 0
    property bool processEnabled: activeConsumers > 0 && (isPlaying || isDecaying) && !isRestarting

    function registerConsumer() {
        activeConsumers++;
    }

    function unregisterConsumer() {
        activeConsumers = Math.max(0, activeConsumers - 1);
    }

    function resetBars() {
        root.barLevels = root.emptyLevels;
    }

    function restartVisualizer() {
        if (!processEnabled || consecutiveCrashes >= 5) return;
        consecutiveCrashes++;
        isRestarting = true;
        restartTimer.restart();
    }

    onActiveConsumersChanged: {
        if (activeConsumers === 0) {
            root.isDecaying = false;
            decayTimer.stop();
        }
    }

    onIsPlayingChanged: {
        if (isPlaying) {
            root.isDecaying = false;
            decayTimer.stop();
        } else if (activeConsumers > 0) {
            root.isDecaying = true;
            decayTimer.restart();
        } else {
            root.isDecaying = false;
            decayTimer.stop();
            resetBars();
        }
    }

    onProcessEnabledChanged: {
        if (!processEnabled) {
            if (!root.isRestarting) root.consecutiveCrashes = 0;
            resetBars();
        }
    }

    Timer {
        id: decayTimer
        interval: 1200
        repeat: false
        onTriggered: root.isDecaying = false
    }

    Timer {
        id: restartTimer
        interval: 500
        repeat: false
        onTriggered: root.isRestarting = false
    }

    Process {
        id: visualizerProcess
        running: root.processEnabled
        onExited: root.restartVisualizer()
        // Vortex (2.5.1): el visualizador sigue siendo cava (no se adopta serpantinum-audio). Va a
        // 30 cuadros por segundo (antes 60). Los visualizadores suavizan los niveles en cada
        // cuadro (Visualizer.qml), así que la mitad de muestras basta y cava, el parseo y los
        // bindings gastan la mitad.
        command: [
            "bash", "-c",
            "exec cava -p <(printf '[general]\\nbars = %d\\nframerate = 30\\nsensitivity = 150\\n[output]\\nmethod = raw\\nraw_target = /dev/stdout\\ndata_format = ascii\\nascii_max_range = 1000\\nbar_delimiter = 59\\n' " + root.barCount + ")"
        ]
        stdout: SplitParser {
            onRead: data => {
                if (!root.processEnabled) return;
                // Vortex: cava saca los niveles como «n;n;n;…» (0 a 1000), no como JSON
                let str = data.trim();
                if (str.length === 0) return;
                root.consecutiveCrashes = 0;
                if (!/[1-9]/.test(str)) {
                    if (root.barLevels !== root.emptyLevels) root.resetBars();
                    return;
                }
                let parts = str.split(";");
                let levels = new Array(root.barCount);
                for (let i = 0; i < root.barCount; i++) {
                    let val = i < parts.length ? (parseInt(parts[i]) || 0) : 0;
                    levels[i] = Math.max(0.0, Math.min(1.0, val / 1000.0));
                }
                root.barLevels = levels;
            }
        }
    }
}
