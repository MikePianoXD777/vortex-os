pragma Singleton
import QtQuick
import QtQml
import Quickshell
import Quickshell.Services.Pipewire
import Quickshell.Services.Mpris
import "../../"

import "AudioCache.js" as AudioCache

Singleton {
    id: root

    function pipewireNodes() {
        try {
            const model = Pipewire.nodes;
            const list = model ? model.values : null;
            return list ? list : [];
        } catch (e) {
            return [];
        }
    }

    property var trackedNodes: []

    function refreshTrackedNodes() {
        const list = root.pipewireNodes();
        // Vortex: solo se enlazan los nodos de audio. Todo lo que lee este archivo
        // (salidas, entradas, apps) filtra por n.audio, y n.audio se sabe sin enlazar
        // el nodo; enlazar también cámaras, vídeo y MIDI solo gastaba memoria y eventos.
        root.trackedNodes = list ? list.filter(n => n && n.audio) : [];
    }

    Connections {
        target: Pipewire.nodes ?? null
        ignoreUnknownSignals: true
        function onValuesChanged() {
            root.refreshTrackedNodes();
        }
    }

    Component.onCompleted: {
        root.refreshTrackedNodes();
        root.refreshActiveStream();
    }

    PwObjectTracker {
        objects: root.trackedNodes
    }

    function isSerpantinumStream(node) {
        if (!node || !node.properties) return false;
        let p = node.properties;
        let appId = p["application.id"] || "";
        let appName = p["application.name"] || "";
        if (appId === "serpantinum-sfx" || appId === "serpantinum" || appId === "org.serpantinum.sfx") return true;
        if (appName === "serpantinum-sfx" || appName === "serpantinum") return true;
        if (p["node.group"] === "serpantinum-audio" || p["node.link-group"] === "serpantinum-audio") return true;
        let mediaFile = p["media.filename"] || p["media.name"] || "";
        if (mediaFile.indexOf("assets/sounds/") !== -1) return true;
        return false;
    }

    readonly property var outputs: {
        let arr = [];
        for (const n of root.pipewireNodes()) {
            if (!n.isStream && n.isSink && n.audio) arr.push(n);
        }
        return AudioCache.updateOutputs(arr);
    }

    readonly property var inputs: {
        let arr = [];
        for (const n of root.pipewireNodes()) {
            if (!n.isStream && !n.isSink && n.audio
                && n.properties?.["device.class"] !== "monitor"
                && !n.name?.endsWith(".monitor")) {
                arr.push(n);
            }
        }
        return AudioCache.updateInputs(arr);
    }

    readonly property var apps: {
        let arr = [];
        for (const n of root.pipewireNodes()) {
            if (n.isStream && n.audio
                && n.properties?.["application.id"] !== "org.PulseAudio.pavucontrol"
                && !isSerpantinumStream(n)) {
                arr.push(n);
            }
        }
        return AudioCache.updateApps(arr);
    }

    function computeHasActiveStream(list) {
        try {
            for (const n of list ?? []) {
                if (!n || !n.audio) continue;
                let p = n.properties;
                if (!p || p["media.class"] !== "Stream/Output/Audio") continue;
                let corked = p["pulse.corked"];
                if (corked === "true" || corked === true) continue;
                if (n.audio.muted || (n.audio.volume !== undefined && n.audio.volume <= 0.001)) continue;
                return true;
            }
        } catch (e) {
        }
        return false;
    }

    property bool hasActiveStream: false

    function refreshActiveStream() {
        const active = root.computeHasActiveStream(root.apps) === true;
        if (root.hasActiveStream !== active) root.hasActiveStream = active;
    }

    onAppsChanged: root.refreshActiveStream()

    Instantiator {
        model: root.apps

        delegate: QtObject {
            id: streamWatch
            required property var modelData

            property Connections nodeConnections: Connections {
                target: streamWatch.modelData
                ignoreUnknownSignals: true
                function onPropertiesChanged() { root.refreshActiveStream(); }
            }

            property Connections audioConnections: Connections {
                target: streamWatch.modelData ? streamWatch.modelData.audio : null
                ignoreUnknownSignals: true
                function onMutedChanged() { root.refreshActiveStream(); }
                function onVolumesChanged() { root.refreshActiveStream(); }
            }
        }
    }

    readonly property PwNode defaultSink: Pipewire.defaultAudioSink

    readonly property string activeOutputName: root.defaultSink && root.defaultSink.name ? root.defaultSink.name : ""
    readonly property PwNode defaultSource: Pipewire.defaultAudioSource

    function setDefaultOutput(node) {
        if (node) Pipewire.preferredDefaultAudioSink = node;
    }

    function setDefaultInput(node) {
        if (node) Pipewire.preferredDefaultAudioSource = node;
    }

    function toggleMute(node) {
        if (node && node.audio) node.audio.muted = !node.audio.muted;
    }

    function setVolume(node, pct) {
        if (node && node.audio) node.audio.volume = Math.max(0, Math.min(1.5, pct / 100.0));
    }

    function getNodeName(node) {
        if (!node) return "";
        return node.properties?.["device.description"] || node.description || node.name || "Unknown Device";
    }

    function getNodeSubDesc(node) {
        if (!node) return "";
        if (node.isStream) {
            return node.properties?.["media.name"] || node.properties?.["window.title"] || node.properties?.["media.role"] || "Audio Stream";
        }
        return node.name || "Unknown";
    }

    function getNodeAppName(node) {
        if (!node) return "";
        return node.properties?.["application.name"] || node.properties?.["application.process.binary"] || node.description || "Unknown App";
    }

    // ================================================================== el audio te sigue (4.4)
    // Conectas audífonos (Bluetooth o USB) y el sonido se pasa solo a ellos. Los desconectas (o se les
    // acaba la pila) mientras sonaban y la música se pausa, en vez de seguir por las bocinas.
    // Se apaga en Ajustes con «audio.seguir»: false.

    property bool seguirActivo: {
        let a = (typeof Config !== "undefined") ? Config.getSetting("audio", {}) : {};
        return !(a && a.seguir === false);
    }
    Connections {
        target: (typeof Config !== "undefined") ? Config : null
        ignoreUnknownSignals: true
        function onSettingsLoaded() {
            let a = Config.getSetting("audio", {});
            root.seguirActivo = !(a && a.seguir === false);
        }
    }

    function esAudifono(n) {
        if (!n || n.isStream || !n.isSink || !n.audio) return false;
        let nombre = n.name || "";
        if (nombre.indexOf("bluez_output.") === 0) return true;
        let p = n.properties || {};
        let forma = String(p["device.form_factor"] || "").toLowerCase();
        if (forma === "headphone" || forma === "headset" || forma === "hands-free" || forma === "handset") return true;
        // Audífonos USB que no dicen su forma: por el nombre del aparato.
        if (String(p["device.bus"] || "") !== "usb") return false;
        let d = String(n.description || n.nickname || "").toLowerCase();
        return /headset|headphone|audífono|audifono|auricular|earbud/.test(d);
    }

    // Las salidas que ya había: al arrancar el shell (o PipeWire) no se cambia nada.
    property var salidasVistas: ({})
    property bool sigueListo: false
    // Las dos últimas salidas por defecto: cuando se va una, PipeWire pone otra antes de que se revise.
    property string ultimaSalida: ""
    property string previaSalida: ""

    Timer {
        id: sigueArranque
        interval: 6000
        running: Pipewire.ready
        onTriggered: {
            let vistas = {};
            for (const n of root.outputs) if (n && n.name) vistas[n.name] = root.esAudifono(n);
            root.salidasVistas = vistas;
            root.sigueListo = true;
        }
    }

    // Una salida nueva tarda un momento en traer sus propiedades: se revisa un poco después.
    Timer {
        id: sigueRevisar
        interval: 1500
        onTriggered: root.revisarSalidas()
    }
    onOutputsChanged: if (sigueListo) sigueRevisar.restart()

    onDefaultSinkChanged: {
        let n = root.defaultSink;
        if (n && n.name && n.name !== root.ultimaSalida) {
            root.previaSalida = root.ultimaSalida;
            root.ultimaSalida = n.name;
        }
    }

    function revisarSalidas() {
        let actuales = {};
        let nuevoAudifono = null;
        for (const n of root.outputs) {
            if (!n || !n.name) continue;
            let aud = root.esAudifono(n);
            actuales[n.name] = aud;
            if (!(n.name in root.salidasVistas) && aud) nuevoAudifono = n;
        }
        // Se fueron los audífonos por los que sonaba: pausa lo que se estaba oyendo.
        for (const nombre in root.salidasVistas) {
            if (nombre in actuales || !root.salidasVistas[nombre]) continue;
            if (root.seguirActivo && (nombre === root.ultimaSalida || nombre === root.previaSalida)) {
                root.pausarTodo();
                break;
            }
        }
        if (root.seguirActivo && nuevoAudifono && root.defaultSink !== nuevoAudifono) {
            root.setDefaultOutput(nuevoAudifono);
        }
        root.salidasVistas = actuales;
    }

    function pausarTodo() {
        let ps = Mpris.players ? Mpris.players.values : [];
        for (let i = 0; i < ps.length; i++) {
            let p = ps[i];
            if (p && p.isPlaying && p.canPause) p.pause();
        }
    }
}
