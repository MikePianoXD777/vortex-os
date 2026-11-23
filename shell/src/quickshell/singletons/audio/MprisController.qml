pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Mpris
import "../../"
import "ElegirReproductor.js" as Elegir

Singleton {
    id: root

    readonly property var _playerWatch: {
        let players = Mpris.players.values;
        let states = [];
        for (let i = 0; i < players.length; i++) {
            states.push(players[i].isPlaying);
            states.push(players[i].canControl);
            states.push(players[i].busName);
        }
        return states;
    }

    property string lastPlayingBus: ""

    // Vortex 4.1: desde cuándo suena cada reproductor (ElegirReproductor.js). Una vista previa de YouTube ya no
    // le quita la barra a Vortex Player (ni su portada).
    property var _desde: ({})
    property int _reelegir: 0
    readonly property var vigiaReproductores: root._playerWatch
    onVigiaReproductoresChanged: root._marcarDesde()
    Component.onCompleted: root._marcarDesde()
    function _marcarDesde() {
        let falta = Elegir.marcar(root._lista(), root._desde, Date.now());
        root._reelegir++;
        if (falta > 0) {
            reelegirTimer.interval = falta + 50;
            reelegirTimer.restart();
        }
    }
    function _lista() {
        return Mpris.players.values.filter(p => p).sort((a, b) => (a.busName || "").localeCompare(b.busName || "")).map(p => ({
            player: p,
            busName: p.busName || "",
            isPlaying: p.isPlaying === true,
            stopped: p.playbackState === MprisPlaybackState.Stopped,
            title: p.trackTitle || "",
            canControl: p.canControl === true
        }));
    }
    Timer {
        id: reelegirTimer
        repeat: false
        onTriggered: root._marcarDesde()
    }

    readonly property MprisPlayer activePlayer: {
        let _ = _playerWatch;
        let __ = _reelegir;
        try {
            let lista = root._lista();
            let i = Elegir.elegir(lista, root.lastPlayingBus, root._desde, Date.now());
            return i >= 0 ? lista[i].player : null;
        } catch (e) {
            return null;
        }
    }

    readonly property string activePlayerId: activePlayer ? (activePlayer.busName || activePlayer.identity || "") : ""
    readonly property bool hasActivePlayer: activePlayer !== null
    readonly property bool isPlaying: {
        try {
            return activePlayer ? activePlayer.isPlaying === true : false;
        } catch (e) {
            return false;
        }
    }
    readonly property string trackTitle: {
        try {
            return activePlayer ? String(activePlayer.trackTitle || "") : "";
        } catch (e) {
            return "";
        }
    }
    readonly property string trackArtist: {
        try {
            return activePlayer ? String(activePlayer.trackArtist || "") : "";
        } catch (e) {
            return "";
        }
    }
    readonly property string currentArtUrl: {
        if (!activePlayer) return "";
        if (activePlayer.trackArtUrl && activePlayer.trackArtUrl !== "") return activePlayer.trackArtUrl;
        if (activePlayer.metadata) {
            let m = activePlayer.metadata;
            if (m["mpris:artUrl"]) return m["mpris:artUrl"];
            if (m["artUrl"]) return m["artUrl"];
            if (m["xesam:url"] && typeof m["xesam:url"] === "string" && (m["xesam:url"].startsWith("http") || m["xesam:url"].startsWith("file://"))) {
                if (m["xesam:url"].match(/\.(jpg|jpeg|png|webp)$/i)) return m["xesam:url"];
            }
        }
        return "";
    }
    property real livePosition: activePlayer ? activePlayer.position : 0

    property string artUrl: ""
    property int artWidth: 0
    property string artTrack: ""
    property string artRawUrl: ""
    property string grad: ""
    property string textColor: "#cdd6f4"
    property string deviceIcon: "󰓃"
    property string deviceName: "Speaker"
    property bool fetchPending: false

    onActivePlayerChanged: {
        if (root.activePlayer && root.activePlayer.isPlaying) {
            root.lastPlayingBus = root.activePlayer.busName || "";
        }
        if (root.activePlayer) {
            root.livePosition = root.activePlayer.position;
        }
        root.queueFetch();
    }

    Timer {
        interval: 1000
        repeat: true
        running: root.hasActivePlayer && root.isPlaying
        onTriggered: {
            if (root.activePlayer) {
                root.activePlayer.positionChanged();
                root.livePosition = root.activePlayer.position;
            }
        }
    }

    Timer {
        id: fetchDebounceTimer
        interval: 60
        repeat: false
        onTriggered: root.fetchArt()
    }

    function queueFetch() {
        fetchDebounceTimer.restart();
    }

    Connections {
        target: root.activePlayer
        function onPositionChanged() {
            if (root.activePlayer) root.livePosition = root.activePlayer.position;
        }
        function onPostTrackChanged() {
            if (root.activePlayer) root.livePosition = root.activePlayer.position;
            root.queueFetch();
        }
        function onTrackArtUrlChanged() {
            root.queueFetch();
        }
        function onTrackTitleChanged() {
            root.queueFetch();
        }
        function onMetadataChanged() {
            root.queueFetch();
        }
        function onPlaybackStateChanged() {
            // El que se enseña y se pone a sonar queda como «el que sonaba» (así una vista previa no se lo lleva).
            if (root.activePlayer && root.activePlayer.isPlaying) root.lastPlayingBus = root.activePlayer.busName || "";
            root.queueFetch();
        }
    }

    Process {
        id: artFetchProc
        command: [
            "bash",
            Caching.qsDir + "/media/art_fetch.sh",
            root.activePlayerId,
            root.currentArtUrl,
            root.trackTitle,
            root.trackArtist
        ]
        onExited: {
            if (root.fetchPending) {
                root.fetchPending = false;
                artFetchProc.running = true;
            }
        }
        stdout: StdioCollector {
            onStreamFinished: {
                let txt = this.text.trim();
                if (txt !== "") {
                    try {
                        let d = JSON.parse(txt);
                        root.deviceIcon = d.deviceIcon || "󰓃";
                        root.deviceName = d.deviceName || "Speaker";

                        if (d.playerId !== root.activePlayerId || d.title !== root.trackTitle || d.artist !== root.trackArtist) {
                            return;
                        }

                        let track = d.playerId + "\n" + d.title + "\n" + d.artist;
                        let sameTrack = !!(d.title || d.artist) && track === root.artTrack;

                        if (d.isPlaceholder === false && d.artUrl) {
                            if (sameTrack && d.artUrl !== root.artUrl && d.artWidth < root.artWidth) return;
                            let carriedOver = track !== root.artTrack && d.rawUrl === root.artRawUrl;
                            root.artTrack = track;
                            root.artRawUrl = d.rawUrl;
                            root.artWidth = carriedOver ? 0 : d.artWidth;
                            root.artUrl = d.artUrl;
                            root.grad = d.grad || "";
                            root.textColor = d.textColor || "#cdd6f4";
                        } else {
                            if (sameTrack && root.artUrl && root.artWidth > 0) return;
                            root.artTrack = "";
                            root.artRawUrl = "";
                            root.artWidth = 0;
                            root.artUrl = "";
                            root.grad = d.grad || "";
                            root.textColor = d.textColor || "#cdd6f4";
                        }
                    } catch(e) {}
                }
            }
        }
    }

    function fetchArt() {
        if (!root.hasActivePlayer) {
            root.artTrack = "";
            root.artRawUrl = "";
            root.artWidth = 0;
            root.artUrl = "";
            root.grad = "";
            root.textColor = "#cdd6f4";
            return;
        }
        if (artFetchProc.running) {
            root.fetchPending = true;
            return;
        }
        root.fetchPending = false;
        artFetchProc.running = true;
    }

    function forceArtRefresh() {
        root.fetchArt();
    }
}
