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
    // 4.0: dos listas fijas que se turnan en vez de una nueva por cada lectura de cava (30 por segundo, más el
    // «split» con 64 textos): esa basura hacía crecer la memoria del shell con la música y el visualizador.
    // Se turnan para que cada lectura sea otra lista y los visualizadores se enteren del cambio.
    readonly property var _listaA: new Array(barCount).fill(0.0)
    readonly property var _listaB: new Array(barCount).fill(0.0)
    property bool _usarA: true
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
                // Vortex: cava saca los niveles como «n;n;n;…» (0 a 1000), no como JSON. 4.0: se leen cifra
                // por cifra, sin crear textos ni listas nuevas.
                let n = root.barCount;
                let out = root._usarA ? root._listaA : root._listaB;
                let i = 0;
                let val = 0;
                let cifras = false;
                let algo = false;
                for (let k = 0; k < data.length && i < n; k++) {
                    let c = data.charCodeAt(k);
                    if (c >= 48 && c <= 57) {
                        val = val * 10 + (c - 48);
                        cifras = true;
                    } else if (c === 59 && cifras) {
                        out[i++] = val >= 1000 ? 1.0 : val / 1000.0;
                        if (val > 0) algo = true;
                        val = 0;
                        cifras = false;
                    }
                }
                if (cifras && i < n) {
                    out[i++] = val >= 1000 ? 1.0 : val / 1000.0;
                    if (val > 0) algo = true;
                }
                if (i === 0) return;
                root.consecutiveCrashes = 0;
                if (!algo) {
                    if (root.barLevels !== root.emptyLevels) root.resetBars();
                    return;
                }
                for (; i < n; i++) out[i] = 0.0;
                root._usarA = !root._usarA;
                root.barLevels = out;
            }
        }
    }
}
