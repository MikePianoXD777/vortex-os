import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.10: panel «Metrónomo y afinador» (lanzador → «Metrónomo»). El metrónomo es un proceso
// aparte (musica.py metronomo): sigue sonando con el panel cerrado y el panel solo le cambia el tempo
// y lee el pulso de metronomo.json. El afinador escucha el micrófono solo mientras su pestaña está a la
// vista y da la nota en do-re-mi con los cents.
// Teclas: Espacio suena/para, ↑↓ ±1, ←→ ±5, T marca el tempo, F cambia la figura, Tab cambia de pestaña.
// Mientras suena o escucha, un clic afuera no lo cierra (Main.qml lee mantenerAbierto): se cierra con
// Esc o con la ✕.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("musica." + clave, porDefecto);
    }

    readonly property string script: Caching.qsDir + "/musica/musica.py"
    readonly property string carpeta: Caching.getRunDir("musica")

    property int pestana: 0 // 0 metrónomo, 1 afinador
    readonly property bool mantenerAbierto: sonando || pestana === 1

    // ------------------------------------------------------------ metrónomo
    property int bpm: 90
    property int compas: 4
    property string figura: "negras"
    property var est: ({})
    property bool leido: false
    readonly property bool sonando: Boolean(est.sonando) && (Date.now() / 1000 - (est.t || 0)) < Math.max(3, 120 / Math.max(20, est.bpm || bpm))
    property int pulso: -1
    property var taps: []

    readonly property var compases: [
        { n: 1, txt: "—" }, { n: 2, txt: "2/4" }, { n: 3, txt: "3/4" }, { n: 4, txt: "4/4" }, { n: 5, txt: "5/4" }, { n: 6, txt: "6/8" }
    ]

    readonly property var figuras: [
        { id: "negras", txt: t("fig_quarter", "Quarters") },
        { id: "octavos", txt: t("fig_eighth", "Eighths") },
        { id: "tresillos", txt: t("fig_triplet", "Triplets") },
        { id: "dieciseisavos", txt: t("fig_sixteenth", "Sixteenths") },
        { id: "sincopa", txt: t("fig_syncopation", "Syncopation") }
    ]

    function ponerFigura(f) {
        figura = f;
        if (sonando) envio.restart();
    }

    function nombreTempo(b) {
        if (b < 40) return "Grave";
        if (b < 60) return "Largo";
        if (b < 66) return "Larghetto";
        if (b < 76) return "Adagio";
        if (b < 108) return "Andante";
        if (b < 120) return "Moderato";
        if (b < 168) return "Allegro";
        if (b < 200) return "Presto";
        return "Prestissimo";
    }

    function ponerBpm(b) {
        bpm = Math.max(20, Math.min(300, Math.round(b)));
        if (sonando) envio.restart();
    }

    function ponerCompas(c) {
        compas = c;
        if (sonando) envio.restart();
    }

    function sonar() {
        Quickshell.execDetached(["python3", script, carpeta, "metronomo", String(bpm), String(compas), figura]);
    }

    function alternar() {
        if (sonando) {
            Quickshell.execDetached(["python3", script, carpeta, "parar"]);
            est = Object.assign({}, est, { sonando: false });
        } else {
            sonar();
        }
    }

    function marcar() {
        let ahora = Date.now();
        let lista = taps.filter(x => ahora - x < 2500);
        lista.push(ahora);
        if (lista.length > 5) lista.shift();
        taps = lista;
        if (lista.length >= 2) ponerBpm(60000 * (lista.length - 1) / (lista[lista.length - 1] - lista[0]));
    }

    Timer {
        id: envio
        interval: 120
        onTriggered: window.sonar()
    }

    FileView {
        id: estadoArchivo
        printErrors: false
        path: window.carpeta ? window.carpeta + "/metronomo.json" : ""
        onLoaded: {
            try {
                let e = JSON.parse(text());
                let antes = window.est.n;
                window.est = e;
                if (!window.leido) {
                    // Al abrir, el panel toma el tempo con el que quedó (o con el que sigue sonando).
                    window.leido = true;
                    if (e.bpm) window.bpm = e.bpm;
                    if (e.compas) window.compas = e.compas;
                    if (e.figura) window.figura = e.figura;
                }
                if (e.n !== antes && e.pulso >= 0) {
                    window.pulso = e.pulso;
                    destello.restart();
                }
            } catch (err) {}
        }
    }

    Timer {
        interval: 40
        repeat: true
        running: window.visible
        onTriggered: estadoArchivo.reload()
    }

    // ------------------------------------------------------------ afinador
    property var lectura: ({})
    property real centsSuaves: 0
    Behavior on centsSuaves { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
    readonly property bool hayNota: (lectura.hz || 0) > 0
    readonly property var nombresNotas: t("note_names", "C,C♯,D,D♯,E,F,F♯,G,G♯,A,A♯,B").split(",")

    Process {
        id: afinadorProc
        running: window.visible && window.pestana === 1
        command: ["python3", window.script, "afinador"]
        stdout: SplitParser {
            onRead: data => {
                try {
                    let l = JSON.parse(data);
                    window.lectura = l;
                    if (l.hz > 0) window.centsSuaves = l.cents;
                } catch (err) {}
            }
        }
        onRunningChanged: if (!running) window.lectura = ({})
    }

    // ------------------------------------------------------------ teclas
    Keys.onPressed: event => {
        if (event.key === Qt.Key_Tab || event.key === Qt.Key_Backtab) {
            pestana = 1 - pestana;
        } else if (pestana === 0 && event.key === Qt.Key_Space) {
            alternar();
        } else if (pestana === 0 && event.key === Qt.Key_Up) {
            ponerBpm(bpm + 1);
        } else if (pestana === 0 && event.key === Qt.Key_Down) {
            ponerBpm(bpm - 1);
        } else if (pestana === 0 && event.key === Qt.Key_Right) {
            ponerBpm(bpm + 5);
        } else if (pestana === 0 && event.key === Qt.Key_Left) {
            ponerBpm(bpm - 5);
        } else if (pestana === 0 && event.key === Qt.Key_T) {
            marcar();
        } else if (pestana === 0 && event.key === Qt.Key_F) {
            let i = figuras.findIndex(f => f.id === figura);
            ponerFigura(figuras[(i + 1) % figuras.length].id);
        } else {
            return;
        }
        event.accepted = true;
    }

    // ------------------------------------------------------------ animación de entrada
    property var exitCallback: null

    function startExit(onFinished) {
        introAnim.stop();
        exitCallback = onFinished;
        exitAnim.restart();
    }

    function resetAndPlayIntro() {
        exitAnim.stop();
        introMain = 0;
        introAnim.restart();
    }

    onVisibleChanged: {
        if (visible) {
            forceActiveFocus();
            estadoArchivo.reload();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        estadoArchivo.reload();
        if (visible) resetAndPlayIntro();
    }

    property real introMain: 0

    NumberAnimation {
        id: introAnim
        target: window; property: "introMain"; from: 0; to: 1.0; duration: 700; easing.type: Easing.OutExpo
    }

    SequentialAnimation {
        id: exitAnim
        NumberAnimation { target: window; property: "introMain"; to: 0.0; duration: 180; easing.type: Easing.InQuart }
        ScriptAction {
            script: {
                if (window.exitCallback) {
                    let cb = window.exitCallback;
                    window.exitCallback = null;
                    cb();
                }
            }
        }
    }

    // ------------------------------------------------------------ piezas
    component Boton: Rectangle {
        id: boton
        property string icono: ""
        property string texto: ""
        property color tinte: ThemeBackend.blue
        property bool principal: false
        property bool elegido: false
        signal pulsado()
        implicitHeight: window.s(36)
        implicitWidth: Math.max(implicitHeight, fila.implicitWidth + window.s(28))
        radius: height / 2
        color: principal || elegido ? (zona.containsMouse ? Qt.lighter(tinte, 1.1) : tinte)
                                    : (zona.containsMouse ? Qt.alpha(tinte, 0.25) : Qt.alpha(ThemeBackend.surface1, 0.8))

        Row {
            id: fila
            anchors.centerIn: parent
            spacing: window.s(7)
            Text {
                visible: boton.icono !== ""
                text: boton.icono
                font.family: ThemeBackend.iconFont
                font.pixelSize: window.s(15)
                color: boton.principal || boton.elegido ? ThemeBackend.crust : boton.tinte
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                visible: boton.texto !== ""
                text: boton.texto
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                font.weight: Font.DemiBold
                color: boton.principal || boton.elegido ? ThemeBackend.crust : ThemeBackend.text
                anchors.verticalCenter: parent.verticalCenter
            }
        }

        MouseArea {
            id: zona
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: boton.pulsado()
        }
    }

    component Parrafo: Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        font.family: ThemeBackend.fontFamily
        font.pixelSize: window.s(12)
        color: ThemeBackend.subtext0
    }

    Item {
        anchors.fill: parent
        scale: 0.95 + (0.05 * introMain)
        opacity: introMain
        transform: Translate { y: window.s(-20) * (1 - introMain) }

        Rectangle {
            anchors.fill: parent
            radius: ThemeBackend.borderRadius
            color: Qt.alpha(ThemeBackend.base, ThemeBackend.elementOpacity)
            clip: true

            WallpaperBlur {
                z: -1
                anchors.fill: parent
                topLeftRadius: parent.topLeftRadius
                topRightRadius: parent.topRightRadius
                bottomLeftRadius: parent.bottomLeftRadius
                bottomRightRadius: parent.bottomRightRadius
            }
        }

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: window.s(22)
            spacing: window.s(16)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(12)
                Text {
                    text: "󰝚"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(30)
                    color: ThemeBackend.mauve
                }
                Text {
                    Layout.fillWidth: true
                    text: window.t("title", "Metronome and tuner")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(18)
                    font.weight: Font.Bold
                    color: ThemeBackend.text
                    elide: Text.ElideRight
                }
                Row {
                    spacing: window.s(6)
                    Boton {
                        texto: window.t("metronome", "Metronome")
                        tinte: ThemeBackend.mauve
                        elegido: window.pestana === 0
                        onPulsado: window.pestana = 0
                    }
                    Boton {
                        texto: window.t("tuner", "Tuner")
                        tinte: ThemeBackend.mauve
                        elegido: window.pestana === 1
                        onPulsado: window.pestana = 1
                    }
                    Boton {
                        icono: "󰅖"
                        tinte: ThemeBackend.overlay1
                        onPulsado: Quickshell.execDetached(["hyprctl", "dispatch", "hl.dsp.event(\"serpantinum:close\")"])
                    }
                }
            }

            // ---------------------------------------------------- metrónomo
            ColumnLayout {
                visible: window.pestana === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(16)

                // Los pulsos del compás: el que suena se enciende.
                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: window.s(12)
                    Repeater {
                        model: window.compas
                        Rectangle {
                            required property int index
                            readonly property bool encendido: window.sonando && window.pulso === index
                            width: window.s(index === 0 && window.compas > 1 ? 26 : 20)
                            height: width
                            radius: width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            color: encendido ? (index === 0 && window.compas > 1 ? ThemeBackend.peach : ThemeBackend.mauve)
                                             : Qt.alpha(ThemeBackend.surface1, 0.9)
                            scale: encendido ? 1.0 + 0.25 * destelloValor.v : 1.0
                        }
                    }
                }

                QtObject {
                    id: destelloValor
                    property real v: 0
                }
                NumberAnimation {
                    id: destello
                    target: destelloValor; property: "v"; from: 1; to: 0; duration: 220; easing.type: Easing.OutQuad
                }

                ColumnLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: 0
                    RowLayout {
                        Layout.alignment: Qt.AlignHCenter
                        spacing: window.s(18)
                        Boton {
                            icono: "󰍴"
                            onPulsado: window.ponerBpm(window.bpm - 1)
                        }
                        Text {
                            Layout.minimumWidth: window.s(130)
                            horizontalAlignment: Text.AlignHCenter
                            text: window.bpm
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(64)
                            font.weight: Font.Bold
                            color: ThemeBackend.text
                        }
                        Boton {
                            icono: "󰐕"
                            onPulsado: window.ponerBpm(window.bpm + 1)
                        }
                    }
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: window.t("bpm", "beats per minute") + " · " + window.nombreTempo(window.bpm)
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext1
                    }
                }

                // Deslizador del tempo (20 a 240; con las flechas llega a 300).
                Item {
                    id: barra
                    Layout.fillWidth: true
                    implicitHeight: window.s(24)
                    readonly property real fraccion: Math.min(1, (window.bpm - 20) / 220)
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        height: window.s(6)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface1, 0.8)
                        Rectangle {
                            width: parent.width * barra.fraccion
                            height: parent.height
                            radius: parent.radius
                            color: ThemeBackend.mauve
                        }
                    }
                    Rectangle {
                        x: (barra.width - width) * barra.fraccion
                        anchors.verticalCenter: parent.verticalCenter
                        width: window.s(18)
                        height: width
                        radius: width / 2
                        color: ThemeBackend.text
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        function mover(mx) { window.ponerBpm(20 + 220 * Math.max(0, Math.min(1, mx / width))); }
                        onPressed: mouse => mover(mouse.x)
                        onPositionChanged: mouse => { if (pressed) mover(mouse.x); }
                        onWheel: wheel => window.ponerBpm(window.bpm + (wheel.angleDelta.y > 0 ? 1 : -1))
                    }
                }

                Flow {
                    Layout.alignment: Qt.AlignHCenter
                    Layout.maximumWidth: parent.width
                    spacing: window.s(6)
                    Repeater {
                        model: window.compases
                        Boton {
                            required property var modelData
                            texto: modelData.txt
                            tinte: ThemeBackend.mauve
                            elegido: window.compas === modelData.n
                            onPulsado: window.ponerCompas(modelData.n)
                        }
                    }
                }

                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: window.s(6)
                    Repeater {
                        model: window.figuras
                        Boton {
                            required property var modelData
                            texto: modelData.txt
                            tinte: ThemeBackend.peach
                            elegido: window.figura === modelData.id
                            onPulsado: window.ponerFigura(modelData.id)
                        }
                    }
                }

                Item { Layout.fillHeight: true }

                RowLayout {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: window.s(10)
                    Boton {
                        principal: true
                        tinte: window.sonando ? ThemeBackend.red : ThemeBackend.mauve
                        icono: window.sonando ? "󰓛" : "󰐊"
                        texto: window.sonando ? window.t("stop", "Stop") : window.t("start", "Start")
                        implicitWidth: window.s(140)
                        implicitHeight: window.s(44)
                        onPulsado: window.alternar()
                    }
                    Boton {
                        icono: "󰳽"
                        texto: window.t("tap", "Tap")
                        tinte: ThemeBackend.mauve
                        implicitHeight: window.s(44)
                        onPulsado: window.marcar()
                    }
                }

                Parrafo {
                    text: window.sonando ? window.t("keeps_playing", "It keeps playing if you close this. Space starts and stops it.")
                                         : window.t("keys", "Space starts and stops · ↑↓ ←→ tempo · T: tap the beat · F: next figure")
                }
            }

            // ---------------------------------------------------- afinador
            ColumnLayout {
                visible: window.pestana === 1
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(14)

                Item { Layout.fillHeight: true }

                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: window.s(4)
                    Text {
                        text: window.hayNota ? window.nombresNotas[((window.lectura.midi % 12) + 12) % 12] : "—"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(84)
                        font.weight: Font.Bold
                        color: !window.hayNota ? ThemeBackend.overlay1
                             : Math.abs(window.lectura.cents) <= 5 ? ThemeBackend.green
                             : Math.abs(window.lectura.cents) <= 15 ? ThemeBackend.yellow : ThemeBackend.text
                    }
                    Text {
                        visible: window.hayNota
                        anchors.bottom: parent.bottom
                        anchors.bottomMargin: window.s(14)
                        text: Math.floor(window.lectura.midi / 12) - 1
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(26)
                        color: ThemeBackend.subtext0
                    }
                }

                // Aguja de -50 a +50 cents; el centro es afinado.
                Item {
                    id: medidor
                    Layout.fillWidth: true
                    implicitHeight: window.s(46)
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width
                        height: window.s(8)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface1, 0.8)
                    }
                    Rectangle {
                        anchors.centerIn: parent
                        width: parent.width * 0.1
                        height: window.s(8)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.green, 0.45)
                    }
                    Repeater {
                        model: 11
                        Rectangle {
                            required property int index
                            x: medidor.width * index / 10 - width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            width: window.s(2)
                            height: window.s(index === 5 ? 30 : 16)
                            color: Qt.alpha(ThemeBackend.overlay1, 0.6)
                        }
                    }
                    Rectangle {
                        visible: window.hayNota
                        x: medidor.width * (0.5 + Math.max(-50, Math.min(50, window.centsSuaves)) / 100) - width / 2
                        anchors.verticalCenter: parent.verticalCenter
                        width: window.s(5)
                        height: window.s(42)
                        radius: width / 2
                        color: Math.abs(window.centsSuaves) <= 5 ? ThemeBackend.green
                             : Math.abs(window.centsSuaves) <= 15 ? ThemeBackend.yellow : ThemeBackend.red
                    }
                }

                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "♭ " + window.t("flat", "low")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: !window.hayNota ? ""
                            : Math.abs(window.lectura.cents) <= 5 ? window.t("in_tune", "In tune")
                            : window.lectura.cents < 0 ? window.t("go_up", "A little higher") + " (" + window.lectura.cents + " ¢)"
                            : window.t("go_down", "A little lower") + " (+" + window.lectura.cents + " ¢)"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(15)
                        font.weight: Font.DemiBold
                        color: ThemeBackend.text
                    }
                    Text {
                        text: window.t("sharp", "high") + " ♯"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                    }
                }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: window.hayNota ? window.lectura.hz.toFixed(1) + " Hz · La = 440 Hz" : "La = 440 Hz"
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(12)
                    color: ThemeBackend.overlay1
                }

                Item { Layout.fillHeight: true }

                // Nivel del micrófono: para ver que sí está escuchando.
                RowLayout {
                    Layout.fillWidth: true
                    spacing: window.s(10)
                    Text {
                        text: "󰍬"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(16)
                        color: window.lectura.error ? ThemeBackend.red : ThemeBackend.subtext0
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: window.s(6)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface1, 0.8)
                        Rectangle {
                            height: parent.height
                            radius: parent.radius
                            width: parent.width * Math.min(1, window.lectura.nivel || 0)
                            color: ThemeBackend.mauve
                            Behavior on width { NumberAnimation { duration: 90 } }
                        }
                    }
                }

                Parrafo {
                    text: window.lectura.error ? window.t("no_mic", "Can't hear the microphone. Check it in the volume panel.")
                                               : window.t("sing", "Sing or play a note near the microphone.")
                }
            }
        }
    }
}
