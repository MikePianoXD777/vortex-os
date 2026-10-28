import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"
import "Minas.js" as Minas

// Vortex 1.10: Buscaminas tryhard (idea de THYRVAREN). Fácil, Medio, Experto y Tryhard (30×20 con 145
// minas), con modificadores que se juntan: sin banderas, contrarreloj (1 s por clic del 3BV), niebla
// (los números se borran a los 2.5 s) y muerte súbita (una bandera mal puesta pierde). Mide lo que
// miden los que juegan en serio: tiempo en centésimas, 3BV, 3BV/s y eficiencia, con récord por cada
// combinación. La lógica está en Minas.js (se prueba con pruebas/buscaminas.sh).
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("buscaminas." + clave, porDefecto);
    }

    readonly property string bomba: "󰚑"
    readonly property string banderaIcono: "󰈻"

    // --- partida ----------------------------------------------------------------------------------

    property string dificultad: "experto"
    property var mods: ({ "sin-banderas": false, "contrarreloj": false, "niebla": false, "muerte-subita": false })
    readonly property var dificultades: ["facil", "medio", "experto", "tryhard"]
    readonly property var listaMods: ["sin-banderas", "contrarreloj", "niebla", "muerte-subita"]

    property var tab: null
    property int version: 0           // sube con cada jugada para que las casillas se vuelvan a pintar
    property real inicio: 0
    property real fin: 0
    property real ahora: 0
    property real pausaEn: 0
    property int clics: 0
    property int bv: 0
    property int limiteSeg: 0
    property var abiertoEn: []        // cuándo se abrió cada casilla (para la niebla)
    property int bajoMouse: -1
    property bool porTiempo: false
    property bool nuevoRecord: false
    property string aviso: ""
    property string ayuda: ""

    readonly property bool jugando: { version; return tab !== null && tab.puestas && !tab.perdio && !tab.gano; }
    readonly property bool termino: { version; return tab !== null && (tab.perdio || tab.gano); }
    readonly property string claveActual: Minas.clave(dificultad, mods)

    function segundos() {
        if (!tab || inicio <= 0) return 0;
        return Math.max(0, ((fin > 0 ? fin : ahora) - inicio) / 1000);
    }

    function nuevo() {
        let d = Minas.DIFICULTADES[dificultad];
        tab = Minas.crear(d.w, d.h, d.minas);
        clics = 0;
        bv = 0;
        limiteSeg = 0;
        inicio = 0;
        fin = 0;
        ahora = 0;
        pausaEn = 0;
        abiertoEn = [];
        porTiempo = false;
        nuevoRecord = false;
        aviso = "";
        version++;
    }

    function elegirDificultad(d) {
        dificultad = d;
        nuevo();
    }

    function alternarMod(m) {
        let copia = Object.assign({}, mods);
        copia[m] = !copia[m];
        mods = copia;
        nuevo();
    }

    function tras(abiertas) {
        let n = Date.now();
        for (let k = 0; k < abiertas.length; k++) abiertoEn[abiertas[k]] = n;
        if (tab.gano || tab.perdio) terminar();
        version++;
    }

    function clicIzq(i) {
        if (!tab || termino) return;
        if (tab.estado[i] === Minas.ABIERTA) {
            acorde(i);
            return;
        }
        if (tab.estado[i] !== Minas.OCULTA) return;
        let primera = !tab.puestas;
        let abiertas = Minas.abrir(tab, i);
        if (primera) {
            inicio = Date.now();
            ahora = inicio;
            bv = Minas.tresBV(tab);
            limiteSeg = Minas.limite(dificultad, bv);
        }
        clics++;
        tras(abiertas);
    }

    function acorde(i) {
        if (!tab || termino || tab.estado[i] !== Minas.ABIERTA || tab.num[i] === 0) return;
        clics++;
        tras(Minas.acorde(tab, i));
    }

    function clicDer(i) {
        if (!tab || termino) return;
        if (tab.estado[i] === Minas.ABIERTA) {
            acorde(i);
            return;
        }
        if (mods["sin-banderas"]) {
            aviso = t("no_flags_hint", "No flags in this mode: open everything else");
            avisoTimer.restart();
            return;
        }
        if (!tab.puestas) return;
        if (Minas.bandera(tab, i, mods["muerte-subita"])) {
            clics++;
            tras([]);
        }
    }

    function terminar() {
        fin = Date.now();
        ahora = fin;
        if (!tab.puestas) return;
        let k = claveActual;
        let r = Object.assign({ "jugadas": 0, "ganadas": 0 }, records[k] || {});
        r.jugadas++;
        if (tab.gano) {
            r.ganadas++;
            let seg = Math.round(segundos() * 100) / 100;
            let bvs = seg > 0 ? Math.round(bv / seg * 100) / 100 : 0;
            if (!r.tiempo || seg < r.tiempo) {
                r.tiempo = seg;
                r.fecha = new Date().toISOString().slice(0, 10);
                nuevoRecord = true;
            }
            r.bvs = Math.max(r.bvs || 0, bvs);
            r.eficiencia = Math.max(r.eficiencia || 0, eficiencia());
        }
        let todos = Object.assign({}, records);
        todos[k] = r;
        records = todos;
        recordsArchivo.setText(JSON.stringify(records, null, 2) + "\n");
    }

    function eficiencia() {
        return clics > 0 ? Math.round(bv / clics * 100) : 0;
    }

    function formato(seg) {
        return seg.toFixed(2);
    }

    Timer {
        interval: 41
        repeat: true
        running: window.visible && window.jugando
        onTriggered: {
            window.ahora = Date.now();
            if (window.mods["contrarreloj"] && window.segundos() >= window.limiteSeg) {
                window.tab.perdio = true;
                window.tab.exploto = -1;
                window.porTiempo = true;
                window.terminar();
                window.version++;
            }
        }
    }

    Timer {
        id: avisoTimer
        interval: 1800
        onTriggered: window.aviso = ""
    }

    // --- récords ----------------------------------------------------------------------------------

    property var records: ({})
    readonly property var recordActual: records[claveActual] || null

    FileView {
        id: recordsArchivo
        printErrors: false
        path: Caching.getStateDir("buscaminas") + "/records.json"
        onLoaded: {
            try {
                let d = JSON.parse(text());
                window.records = (d && typeof d === "object") ? d : {};
            } catch (e) {
                window.records = {};
            }
        }
    }

    // --- teclado ----------------------------------------------------------------------------------

    Keys.onPressed: (event) => {
        if (event.key === Qt.Key_R || event.key === Qt.Key_F2) {
            nuevo();
            event.accepted = true;
        } else if (event.key === Qt.Key_Space) {
            if (bajoMouse >= 0) clicDer(bajoMouse);
            event.accepted = true;
        } else if (event.key >= Qt.Key_1 && event.key <= Qt.Key_4) {
            elegirDificultad(dificultades[event.key - Qt.Key_1]);
            event.accepted = true;
        }
    }

    // --- abrir y cerrar ---------------------------------------------------------------------------

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

    // Cerrar el panel a media partida la pausa: el reloj sigue donde iba al volver
    onVisibleChanged: {
        if (visible) {
            if (!tab) nuevo();
            if (pausaEn > 0 && jugando) {
                let pausa = Date.now() - pausaEn;
                inicio += pausa;
                for (let k in abiertoEn) abiertoEn[k] += pausa;
                ahora = Date.now();
            }
            pausaEn = 0;
            forceActiveFocus();
            resetAndPlayIntro();
        } else {
            if (jugando) pausaEn = Date.now();
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        nuevo();
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

    // --- piezas -----------------------------------------------------------------------------------

    component Chip: Rectangle {
        id: chip
        property string texto: ""
        property string descripcion: ""
        property bool activo: false
        property color acento: ThemeBackend.mauve
        signal pulsado()
        implicitHeight: window.s(32)
        implicitWidth: textoChip.implicitWidth + window.s(26)
        radius: height / 2
        color: activo ? Qt.alpha(acento, 0.25) : (zonaChip.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.9) : Qt.alpha(ThemeBackend.surface0, 0.7))
        border.width: activo ? 1 : 0
        border.color: acento
        Behavior on color { ColorAnimation { duration: 120 } }
        Text {
            id: textoChip
            anchors.centerIn: parent
            text: chip.texto
            font.family: ThemeBackend.fontFamily
            font.pixelSize: window.s(12)
            font.weight: Font.Bold
            color: chip.activo ? chip.acento : ThemeBackend.text
        }
        MouseArea {
            id: zonaChip
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onContainsMouseChanged: window.ayuda = containsMouse ? chip.descripcion : ""
            onClicked: chip.pulsado()
        }
    }

    component Dato: ColumnLayout {
        property string titulo: ""
        property string valor: ""
        property color tinta: ThemeBackend.text
        spacing: 0
        Text {
            text: parent.titulo
            font.family: ThemeBackend.fontFamily
            font.pixelSize: window.s(10)
            font.weight: Font.Bold
            color: ThemeBackend.subtext0
        }
        Text {
            text: parent.valor
            font.family: ThemeBackend.fontFamily
            font.pixelSize: window.s(20)
            font.weight: Font.Bold
            color: parent.tinta
        }
    }

    // Colores fijos para los números, la bandera y las minas: en el buscaminas el color es lo que deja
    // leer rápido, y con un tema en blanco y negro saldría todo igual. Con un tema claro, más oscuros.
    readonly property bool temaClaro: ThemeBackend.base.hslLightness > 0.5
    function tono(c) {
        return temaClaro ? Qt.darker(c, 1.9) : c;
    }
    readonly property var coloresNumero: ["#cdd6f4", "#89b4fa", "#a6e3a1", "#f38ba8", "#cba6f7", "#fab387", "#94e2d5", "#cdd6f4", "#9399b2"]
    readonly property color colorBandera: tono("#fab387")
    readonly property color colorMina: tono("#f38ba8")

    function colorNumero(n) {
        return tono(coloresNumero[n] || coloresNumero[0]);
    }

    // --- panel ------------------------------------------------------------------------------------

    Item {
        anchors.fill: parent
        scale: 0.95 + (0.05 * introMain)
        opacity: introMain

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
            anchors.margins: window.s(24)
            spacing: window.s(14)

            // Título y dificultad
            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Rectangle {
                    Layout.preferredWidth: window.s(52)
                    Layout.preferredHeight: window.s(52)
                    radius: width / 2
                    color: Qt.alpha(window.termino && window.tab.perdio ? ThemeBackend.red : ThemeBackend.mauve, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: window.bomba
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(26)
                        color: window.termino && window.tab.perdio ? ThemeBackend.red : ThemeBackend.mauve
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Tryhard Minesweeper")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: window.ayuda !== "" ? window.ayuda : window.t("intro", "The first click is always safe. After that, it's on you.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: window.ayuda !== "" ? ThemeBackend.text : ThemeBackend.subtext0
                    }
                }

                Repeater {
                    model: window.dificultades
                    delegate: Chip {
                        required property string modelData
                        required property int index
                        Layout.preferredHeight: window.s(36)
                        texto: window.t("diff." + modelData, modelData)
                        descripcion: {
                            let d = Minas.DIFICULTADES[modelData];
                            return d.w + "×" + d.h + " · " + d.minas + " " + window.t("mines_word", "mines") + " · " + (index + 1);
                        }
                        activo: window.dificultad === modelData
                        acento: modelData === "tryhard" ? ThemeBackend.red : ThemeBackend.mauve
                        onPulsado: window.elegirDificultad(modelData)
                    }
                }
            }

            // Modificadores (se juntan)
            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(8)

                Text {
                    text: window.t("modifiers", "Modifiers:")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(13)
                    font.weight: Font.Bold
                    color: ThemeBackend.subtext0
                }

                Repeater {
                    model: window.listaMods
                    delegate: Chip {
                        required property string modelData
                        readonly property string llave: modelData.replace("-", "_")
                        texto: window.t("mod." + llave, modelData)
                        descripcion: window.t("mod_desc." + llave, "")
                        activo: window.mods[modelData] === true
                        acento: ThemeBackend.peach
                        onPulsado: window.alternarMod(modelData)
                    }
                }

                Item { Layout.fillWidth: true }

                Chip {
                    Layout.preferredHeight: window.s(36)
                    texto: window.t("again", "New game (R)")
                    acento: ThemeBackend.green
                    activo: window.termino
                    onPulsado: window.nuevo()
                }
            }

            // Marcador
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: window.s(64)
                radius: ThemeBackend.borderRadius
                color: Qt.alpha(ThemeBackend.surface0, 0.55)

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: window.s(20)
                    anchors.rightMargin: window.s(20)
                    spacing: window.s(28)

                    Dato {
                        titulo: window.t("stat.mines", "Mines left")
                        valor: { window.version; return window.tab ? String(window.tab.minas - (window.mods["sin-banderas"] ? 0 : window.tab.banderas)) : "0"; }
                    }
                    Dato {
                        titulo: window.mods["contrarreloj"] && window.limiteSeg > 0 ? window.t("stat.left", "Time left") : window.t("stat.time", "Time")
                        valor: {
                            window.ahora; window.version;
                            if (window.mods["contrarreloj"] && window.limiteSeg > 0)
                                return Math.max(0, window.limiteSeg - window.segundos()).toFixed(2);
                            return window.formato(window.segundos());
                        }
                        tinta: window.mods["contrarreloj"] && window.limiteSeg > 0 && window.limiteSeg - window.segundos() < 10 ? ThemeBackend.red : ThemeBackend.text
                    }
                    Dato {
                        titulo: "3BV"
                        valor: window.bv > 0 ? String(window.bv) : "—"
                    }
                    Dato {
                        titulo: "3BV/s"
                        valor: { window.version; return window.termino && window.tab.gano && window.segundos() > 0 ? (window.bv / window.segundos()).toFixed(2) : "—"; }
                    }
                    Dato {
                        titulo: window.t("stat.clicks", "Clicks")
                        valor: String(window.clics)
                    }
                    Dato {
                        titulo: window.t("stat.efficiency", "Efficiency")
                        valor: { window.version; return window.termino && window.tab.gano ? window.eficiencia() + "%" : "—"; }
                    }

                    Item { Layout.fillWidth: true }

                    Dato {
                        titulo: window.t("stat.best", "Best")
                        valor: window.recordActual && window.recordActual.tiempo ? window.formato(window.recordActual.tiempo) + " s" : "—"
                        tinta: ThemeBackend.yellow
                    }
                    Dato {
                        titulo: window.t("stat.won", "Won")
                        valor: window.recordActual ? window.recordActual.ganadas + "/" + window.recordActual.jugadas : "0/0"
                    }
                }
            }

            // Tablero
            Item {
                id: area
                Layout.fillWidth: true
                Layout.fillHeight: true

                readonly property int celda: window.tab
                    ? Math.max(window.s(12), Math.floor(Math.min(width / window.tab.w, height / window.tab.h, window.s(44))))
                    : window.s(30)

                Item {
                    id: tablero
                    anchors.centerIn: parent
                    width: window.tab ? window.tab.w * area.celda : 0
                    height: window.tab ? window.tab.h * area.celda : 0

                    Repeater {
                        model: window.tab ? window.tab.w * window.tab.h : 0

                        delegate: Rectangle {
                            id: casilla
                            required property int index
                            readonly property int est: { window.version; return window.tab ? window.tab.estado[index] : 0; }
                            readonly property bool mina: { window.version; return window.tab ? window.tab.mina[index] : false; }
                            readonly property int num: { window.version; return window.tab ? window.tab.num[index] : 0; }
                            readonly property bool perdio: window.termino && window.tab.perdio
                            readonly property bool exploto: { window.version; return window.tab && window.tab.exploto === index; }
                            readonly property bool enNiebla: window.mods["niebla"] && window.jugando && est === Minas.ABIERTA && num > 0
                                && window.ahora - (window.abiertoEn[index] || 0) > 2500
                            readonly property bool encima: window.bajoMouse === index && !window.termino

                            x: (index % window.tab.w) * area.celda
                            y: Math.floor(index / window.tab.w) * area.celda
                            width: area.celda - Math.max(1, Math.round(area.celda * 0.08))
                            height: width
                            radius: Math.max(2, area.celda * 0.16)
                            color: {
                                if (exploto) return window.colorMina;
                                if (est === Minas.ABIERTA) return Qt.alpha(ThemeBackend.surface0, 0.45);
                                if (encima) return Qt.alpha(ThemeBackend.surface2, 0.95);
                                return Qt.alpha(ThemeBackend.surface1, 0.9);
                            }

                            Text {
                                anchors.centerIn: parent
                                readonly property bool esIcono: casilla.est === Minas.BANDERA || (casilla.mina && (casilla.est === Minas.ABIERTA || casilla.perdio))
                                text: {
                                    if (casilla.est === Minas.BANDERA) return window.banderaIcono;
                                    if (casilla.mina && (casilla.est === Minas.ABIERTA || casilla.perdio)) return window.bomba;
                                    if (casilla.est === Minas.ABIERTA && casilla.num > 0) return String(casilla.num);
                                    return "";
                                }
                                font.family: esIcono ? ThemeBackend.iconFont : ThemeBackend.fontFamily
                                font.pixelSize: area.celda * (esIcono ? 0.58 : 0.6)
                                font.weight: Font.Black
                                color: {
                                    if (casilla.exploto) return "#1e1e2e";
                                    if (casilla.est === Minas.BANDERA) return casilla.perdio && !casilla.mina ? ThemeBackend.overlay1 : window.colorBandera;
                                    if (casilla.mina) return window.colorMina;
                                    return window.colorNumero(casilla.num);
                                }
                                opacity: casilla.enNiebla ? 0 : 1
                                Behavior on opacity { NumberAnimation { duration: 600; easing.type: Easing.InQuad } }
                            }

                            // Bandera mal puesta, al final
                            Text {
                                anchors.centerIn: parent
                                visible: casilla.perdio && casilla.est === Minas.BANDERA && !casilla.mina
                                text: "✕"
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: area.celda * 0.7
                                font.weight: Font.Bold
                                color: window.colorMina
                            }
                        }
                    }

                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                        cursorShape: window.termino ? Qt.ArrowCursor : Qt.PointingHandCursor

                        function casillaEn(mx, my) {
                            if (!window.tab) return -1;
                            let x = Math.floor(mx / area.celda), y = Math.floor(my / area.celda);
                            if (x < 0 || y < 0 || x >= window.tab.w || y >= window.tab.h) return -1;
                            return y * window.tab.w + x;
                        }

                        onPositionChanged: (mouse) => window.bajoMouse = casillaEn(mouse.x, mouse.y)
                        onExited: window.bajoMouse = -1
                        onPressed: (mouse) => {
                            window.forceActiveFocus();
                            let i = casillaEn(mouse.x, mouse.y);
                            if (i < 0) return;
                            if (mouse.button === Qt.RightButton) window.clicDer(i);
                            else if (mouse.button === Qt.MiddleButton) window.acorde(i);
                            else window.clicIzq(i);
                        }
                    }
                }
            }

            // Cómo va / cómo se juega
            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(12)

                Text {
                    text: {
                        if (window.aviso !== "") return window.aviso;
                        if (window.termino && window.tab.gano)
                            return (window.nuevoRecord ? window.t("new_record", "New record!") + " " : "")
                                + window.t("won", "You won in %1 s").replace("%1", window.formato(window.segundos()));
                        if (window.porTiempo) return window.t("timeout", "Time's up. R for another");
                        if (window.termino) return window.t("lost", "Boom. R for another");
                        if (!window.jugando) return window.t("ready", "Click anywhere to start");
                        return "";
                    }
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(15)
                    font.weight: Font.Bold
                    color: window.aviso !== "" ? ThemeBackend.peach
                         : window.termino && window.tab.gano ? (window.nuevoRecord ? ThemeBackend.yellow : ThemeBackend.green)
                         : window.termino ? ThemeBackend.red : ThemeBackend.subtext0
                }

                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignRight
                    wrapMode: Text.WordWrap
                    maximumLineCount: 2
                    elide: Text.ElideRight
                    text: window.t("controls", "Left: open · Right: flag · Click a number or middle: chord · Space: flag/chord · R: new · 1-4: difficulty")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: ThemeBackend.overlay1
                }
            }
        }
    }
}
