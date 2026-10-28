import QtQuick
import QtQuick.Window
import Quickshell.Io
import "../"

MusicPlayerState {
    id: root

    property var eqData: ({
        "bandas": root.bandasBase([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        "preset": "Flat", "pending": false, "balance": 0,
        "mono": false, "swap": false, "dynamics": "off"
    })

    property var savedEqData: ({
        "bandas": root.bandasBase([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        "preset": "Flat", "pending": false, "balance": 0,
        "mono": false, "swap": false, "dynamics": "off"
    })

    property bool audioTestRunning: false

    Timer {
        id: audioTestTimer
        interval: 3200
        repeat: false
        onTriggered: root.audioTestRunning = false
    }

    // Vortex 1.10 (idea de THYRVAREN): las bandas son una lista de {f: Hz, g: dB} que se edita desde el
    // panel: cambiar la frecuencia de cada una y agregar o quitar (de 2 a 16). Los presets vuelven a
    // las 10 de siempre. equalizer.sh guarda la lista con «set_bands f:g,f:g,…».
    readonly property var frecuenciasBase: [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
    readonly property int minBandas: 2
    readonly property int maxBandas: 16

    function bandasBase(ganancias) {
        return frecuenciasBase.map((f, i) => ({ "f": f, "g": ganancias[i] }));
    }

    function bandasDe(datos) {
        if (datos && Array.isArray(datos.bandas) && datos.bandas.length >= minBandas) return datos.bandas;
        // Estado de antes de la 1.10 (b1…b10), mientras equalizer.sh lo pasa a la lista
        return bandasBase(frecuenciasBase.map((f, i) => Number(datos && datos["b" + (i + 1)]) || 0));
    }

    readonly property var eqBandas: bandasDe(eqData)
    property string eqAyuda: ""

    function textoBandas(bandas) {
        return bandas.map(b => Math.round(b.f) + ":" + Math.round(b.g)).join(",");
    }

    function formatoFrecuencia(f) {
        if (f < 1000) return String(Math.round(f));
        let k = f / 1000;
        return (k >= 10 || Math.abs(k - Math.round(k)) < 0.05 ? String(Math.round(k)) : k.toFixed(1)) + "k";
    }

    // «1.5k», «1,5 kHz», «800», «800hz» → Hz entre 20 y 20 000 (NaN si no se entiende)
    function leerFrecuencia(texto) {
        let m = String(texto).trim().toLowerCase().replace(",", ".").match(/^([0-9]+(?:\.[0-9]+)?)\s*(k)?(?:hz)?$/);
        if (!m) return NaN;
        let f = parseFloat(m[1]) * (m[2] ? 1000 : 1);
        return Math.max(20, Math.min(20000, Math.round(f)));
    }

    function cambiarBandas(bandas) {
        let lista = bandas.map(b => ({ "f": Math.max(20, Math.min(20000, Math.round(b.f))),
                                       "g": Math.max(-12, Math.min(12, Math.round(b.g))) }));
        lista.sort((a, b) => a.f - b.f);
        let temp = Object.assign({}, root.eqData);
        temp.bandas = lista;
        temp.preset = "Custom";
        temp.pending = true;
        root.eqData = temp;
        root.lastEqUpdate = Date.now();
        enviarBandasTimer.restart();
    }

    // La rueda del mouse cambia la frecuencia muchas veces seguidas: se manda una sola vez al final
    Timer {
        id: enviarBandasTimer
        interval: 250
        onTriggered: {
            root.lastEqUpdate = Date.now();
            root.ecualizador(["set_bands", root.textoBandas(root.eqBandas)]);
        }
    }

    function ponerGanancia(i, g) {
        let b = eqBandas.slice();
        b[i] = { "f": b[i].f, "g": g };
        cambiarBandas(b);
    }

    function ponerFrecuencia(i, f) {
        if (isNaN(f)) return;
        let b = eqBandas.slice();
        // Dos bandas en la misma frecuencia se suman: la movida se corre un poquito
        while (b.some((x, j) => j !== i && Math.round(x.f) === Math.round(f))) f = f * 1.03;
        b[i] = { "f": f, "g": b[i].g };
        cambiarBandas(b);
    }

    // La nueva va en el hueco más grande (en octavas, contando de 20 Hz a 20 kHz), con la ganancia
    // de sus vecinas para no cambiar cómo suena
    function agregarBanda() {
        let b = eqBandas;
        if (b.length >= maxBandas) return;
        let mejor = 0, hueco = -1;
        for (let i = 0; i <= b.length; i++) {
            let izq = i === 0 ? 20 : b[i - 1].f, der = i === b.length ? 20000 : b[i].f;
            let octavas = Math.log2(der / izq) * (i === 0 || i === b.length ? 0.5 : 1);
            if (octavas > hueco) { hueco = octavas; mejor = i; }
        }
        let izq = mejor === 0 ? 20 : b[mejor - 1].f, der = mejor === b.length ? 20000 : b[mejor].f;
        let gIzq = mejor === 0 ? b[0].g : b[mejor - 1].g, gDer = mejor === b.length ? b[b.length - 1].g : b[mejor].g;
        let nueva = b.slice();
        nueva.splice(mejor, 0, { "f": Math.sqrt(izq * der), "g": (Number(gIzq) + Number(gDer)) / 2 });
        cambiarBandas(nueva);
    }

    function quitarBanda(i) {
        if (eqBandas.length <= minBandas) return;
        let b = eqBandas.slice();
        b.splice(i, 1);
        cambiarBandas(b);
    }

    function restoreLastSavedEqualizer() {
        if (!root.savedEqData) return;
        enviarBandasTimer.stop();
        var wasPending = root.eqData && root.eqData.pending;
        var needsRestore = wasPending ||
            JSON.stringify(bandasDe(root.eqData)) !== JSON.stringify(bandasDe(root.savedEqData));
        var restored = Object.assign({}, root.savedEqData);
        restored.pending = false;
        root.eqData = restored;
        if (needsRestore) {
            root.lastEqUpdate = Date.now();
            if (root.savedEqData.preset && root.savedEqData.preset !== "Custom") {
                root.ecualizador(["preset", String(root.savedEqData.preset)]);
            } else {
                root.ecualizador(["set_bands", textoBandas(bandasDe(root.savedEqData)), "guardado"]);
            }
        }
    }

    property color eqAccentColor: ThemeBackend.mauve || "#cba6f7"
    property string accumulatedEqOut: ""
    property real lastEqUpdate: 0

    property real eqLightningProgress: 0.0
    property real eqLightningFade: 1.0

    SequentialAnimation {
        id: eqLightningAnim
        running: false
        ScriptAction { script: { root.eqLightningFade = 0.0; root.eqLightningProgress = 0.0; } }
        NumberAnimation { 
            target: root; property: "eqLightningProgress"; 
            from: 0.0; to: 10.0; 
            duration: 650; 
            easing.type: Easing.OutSine 
        }
        PauseAnimation { duration: 150 }
        NumberAnimation { 
            target: root; property: "eqLightningFade"; 
            from: 0.0; to: 1.0; 
            duration: 800; 
            easing.type: Easing.OutQuad 
        }
        ScriptAction { script: { root.eqLightningProgress = 0.0; } }
    }

    function triggerEqLightning() {
        Sounds.playSfx("musicpopup/swoosh.wav", 0.75);
        eqLightningAnim.restart();
    }

    function execCmd(cmdStr) {
        var safeCmd = cmdStr.replace(/`/g, "\\`");
        var p = Qt.createQmlObject(`
            import Quickshell.Io
            Process {
                command: ["bash", "-c", \`${safeCmd}\`]
                running: true
                onExited: (exitCode) => destroy()
            }
        `, root);
    }

    // Vortex: equalizer.sh con sus argumentos en lista (execCmd pasa por «bash -c» y se rompe si la
    // carpeta del shell tiene espacios, como cuando corre desde el repo)
    Component {
        id: ecualizadorProc
        Process {
            onExited: destroy()
        }
    }

    function ecualizador(args) {
        let p = ecualizadorProc.createObject(root, { "command": ["bash", Caching.qsDir + "/media/equalizer.sh"].concat(args) });
        p.running = true;
    }

    function applyPresetOptimistically(presetName) {
        var presets = {
            "Flat": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            "Bass": [5, 7, 5, 2, 1, 0, 0, 0, 1, 2],
            "Treble": [-2, -1, 0, 1, 2, 3, 4, 5, 6, 6],
            "Vocal": [-2, -1, 1, 3, 5, 5, 4, 2, 1, 0],
            "Pop": [2, 4, 2, 0, 1, 2, 4, 2, 1, 2],
            "Rock": [5, 4, 2, -1, -2, -1, 2, 4, 5, 6],
            "Jazz": [3, 3, 1, 1, 1, 1, 2, 1, 2, 3],
            "Classic": [0, 1, 2, 2, 2, 2, 1, 2, 3, 4]
        };
        if (presets[presetName]) {
            enviarBandasTimer.stop();
            var temp = Object.assign({}, root.eqData);
            temp.bandas = root.bandasBase(presets[presetName]);
            temp.preset = presetName;
            temp.pending = false;
            root.eqData = temp;
            root.savedEqData = Object.assign({}, temp);
            
            root.lastEqUpdate = Date.now();
            
            root.triggerEqLightning();
            ecualizador(["preset", String(presetName)]);
        }
    }

    function setBalance(value) {
        var clamped = Math.max(-1.0, Math.min(1.0, Number(value)));
        if (isNaN(clamped)) return;

        var temp = Object.assign({}, root.eqData);
        temp.balance = Math.round(clamped * 100) / 100;
        root.eqData = temp;
        root.savedEqData = Object.assign({}, temp);
        root.lastEqUpdate = Date.now();
        root.ecualizador(["set_balance", String(temp.balance)]);
    }

    function setAudioFlag(name, value) {
        if (name !== "mono" && name !== "swap") return;
        var temp = Object.assign({}, root.eqData);
        temp[name] = value === true;
        root.eqData = temp;
        root.savedEqData = Object.assign({}, temp);
        root.lastEqUpdate = Date.now();
        root.ecualizador(["set_" + name, String(temp[name])]);
    }

    function setDynamics(value) {
        if (["off", "balanced", "night"].indexOf(value) < 0) return;
        var temp = Object.assign({}, root.eqData);
        temp.dynamics = value;
        root.eqData = temp;
        root.savedEqData = Object.assign({}, temp);
        root.lastEqUpdate = Date.now();
        root.ecualizador(["set_dynamics", String(value)]);
    }

    function resetAudio() {
        var temp = Object.assign({}, root.eqData);
        temp.balance = 0;
        temp.mono = false;
        temp.swap = false;
        temp.dynamics = "off";
        root.eqData = temp;
        root.savedEqData = Object.assign({}, temp);
        root.audioTestRunning = false;
        audioTestTimer.stop();
        root.lastEqUpdate = Date.now();
        root.ecualizador(["reset_audio"]);
    }

    function toggleAudioTest() {
        if (root.audioTestRunning) {
            root.ecualizador(["stop_test"]);
            root.audioTestRunning = false;
            audioTestTimer.stop();
        } else {
            root.ecualizador(["test"]);
            root.audioTestRunning = true;
            audioTestTimer.restart();
        }
    }

    property bool eqInitDone: false

    function applyEqState(text) {
        var outStr = text ? text.trim() : "";
        if (outStr.length === 0) return;

        // Our own writes land here too; skip them for a moment and re-read once they settle.
        var sinceLocal = Date.now() - root.lastEqUpdate;
        if (sinceLocal < 2000) {
            eqResyncTimer.interval = 2000 - sinceLocal;
            eqResyncTimer.restart();
            return;
        }

        try {
            var parsed = JSON.parse(outStr);
            root.eqData = parsed;
            if (!parsed.pending) {
                root.savedEqData = Object.assign({}, parsed);
            }
        } catch(e) {}
    }

    // equalizer.sh keeps its state in this file; watch it instead of polling the script.
    FileView {
        id: eqStateFile
        path: Caching.stateDir + "/music/eq_state.json"
        watchChanges: true
        printErrors: false
        onFileChanged: reload()
        onLoaded: root.applyEqState(text())
        // The script creates the file with defaults on first use.
        onLoadFailed: {
            if (!root.eqInitDone && !eqInitProc.running) {
                root.eqInitDone = true;
                eqInitProc.running = true;
            }
        }
    }

    Timer {
        id: eqResyncTimer
        repeat: false
        onTriggered: eqStateFile.reload()
    }

    Process {
        id: eqInitProc
        running: false
        command: ["bash", Caching.qsDir + "/media/equalizer.sh", "get"] // Vortex: rutas con espacios
        onExited: eqStateFile.reload()
    }
}
