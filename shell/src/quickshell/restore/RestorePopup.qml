import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.10: Vortexine Restore (lanzador → «Vortexine Restore», o «vortex restore»). La lista de
// puntos de restauración de la configuración (vortex-restore lista --json); «Crear punto» guarda uno
// con tu nota y «Volver» regresa todo o solo una parte (Hyprland, tema, widgets…). Antes de volver se
// guarda lo de ahora, así que un «Volver» se deshace volviendo al punto «Antes de volver…».
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("restore." + clave, porDefecto);
    }

    property var puntos: []
    property bool cargado: false
    property string abierto: ""      // el punto que muestra sus opciones de «Volver»
    property bool trabajando: false
    property string mensaje: ""
    property bool mensajeError: false

    readonly property var partes: ["hyprland", "tema", "widgets", "terminal", "apariencia", "vortex"]

    function cargar() {
        if (listaProc.running) return;
        listaProc.running = true;
    }

    function correr(args, aviso) {
        if (accionProc.running) return;
        window.trabajando = true;
        window.mensaje = "";
        window.avisoAlTerminar = aviso;
        accionProc.command = ["vortex-restore"].concat(args);
        accionProc.running = true;
    }
    property string avisoAlTerminar: ""

    function crear() {
        let nota = notaInput.text.trim();
        correr(["crear", nota !== "" ? nota : window.t("manual_note", "Manual point")], window.t("created", "Point saved."));
        notaInput.text = "";
    }

    function volver(id, parte) {
        window.abierto = "";
        correr(parte === "todo" ? ["volver", id] : ["volver", id, parte], window.t("restored", "Done: it's back to that point."));
    }

    Process {
        id: listaProc
        command: ["vortex-restore", "lista", "--json"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.puntos = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.puntos = [];
                }
                window.cargado = true;
            }
        }
    }

    Process {
        id: accionProc
        stderr: StdioCollector { id: errores }
        onExited: function(exitCode, exitStatus) {
            window.trabajando = false;
            window.mensajeError = exitCode !== 0;
            window.mensaje = exitCode === 0 ? window.avisoAlTerminar : (errores.text.trim() || window.t("error", "It didn't work."));
            window.cargar();
        }
    }

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
            mensaje = "";
            abierto = "";
            cargar();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        cargar();
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

    function nombreParte(id) {
        let porDefecto = { hyprland: "Hyprland", tema: "Bar and theme", widgets: "Widgets", terminal: "Terminal",
                           apariencia: "App look", vortex: "Vortexine's", todo: "Everything" };
        return window.t("parts." + id, porDefecto[id] || id);
    }

    function nombreTipo(tipo) {
        let porDefecto = { manual: "Yours", auto: "Automatic", inicio: "Login", actualizar: "Before updating",
                           antes: "Before going back", volver: "Went back" };
        return window.t("types." + tipo, porDefecto[tipo] || tipo);
    }

    function colorTipo(tipo) {
        return tipo === "manual" ? ThemeBackend.mauve : tipo === "inicio" ? ThemeBackend.green
             : tipo === "antes" || tipo === "volver" ? ThemeBackend.peach : ThemeBackend.blue;
    }

    function fecha(segundos) {
        let d = new Date(segundos * 1000);
        let hoy = new Date();
        let ayer = new Date(hoy.getTime() - 86400000);
        let hora = Qt.formatTime(d, "HH:mm");
        if (d.toDateString() === hoy.toDateString()) return window.t("today", "Today") + " " + hora;
        if (d.toDateString() === ayer.toDateString()) return window.t("yesterday", "Yesterday") + " " + hora;
        return Qt.formatDate(d, "dd/MM") + " " + hora;
    }

    component Boton: Rectangle {
        id: boton
        property string texto: ""
        property color fondo: ThemeBackend.surface1
        property color letra: ThemeBackend.text
        signal pulsado()
        implicitHeight: window.s(32)
        implicitWidth: etiquetaBoton.implicitWidth + window.s(26)
        radius: height / 2
        opacity: enabled ? 1 : 0.5
        color: zonaBoton.containsMouse ? Qt.lighter(fondo, 1.15) : fondo
        Text {
            id: etiquetaBoton
            anchors.centerIn: parent
            text: boton.texto
            font.family: ThemeBackend.fontFamily
            font.pixelSize: window.s(12)
            font.weight: Font.Bold
            color: boton.letra
        }
        MouseArea {
            id: zonaBoton
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: boton.pulsado()
        }
    }

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

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Rectangle {
                    Layout.preferredWidth: window.s(52)
                    Layout.preferredHeight: window.s(52)
                    radius: width / 2
                    color: Qt.alpha(ThemeBackend.mauve, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: window.trabajando || listaProc.running ? "󰔟" : "󰁯"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(26)
                        color: ThemeBackend.mauve
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: "Vortexine Restore"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: window.t("intro", "Restore points of your desktop: if something breaks, go back to an earlier one, all of it or just one part.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(10)

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: window.s(38)
                    radius: height / 2
                    color: Qt.alpha(ThemeBackend.surface0, 0.6)
                    border.width: notaInput.activeFocus ? 1 : 0
                    border.color: ThemeBackend.mauve

                    TextInput {
                        id: notaInput
                        anchors.fill: parent
                        anchors.leftMargin: window.s(16)
                        anchors.rightMargin: window.s(16)
                        verticalAlignment: TextInput.AlignVCenter
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.text
                        clip: true
                        maximumLength: 80
                        onAccepted: window.crear()
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: notaInput.text === ""
                            text: window.t("note_placeholder", "Note (optional), e.g. «before changing the bar»")
                            font: notaInput.font
                            color: ThemeBackend.overlay1
                        }
                    }
                }
                Boton {
                    Layout.preferredHeight: window.s(38)
                    texto: "󰆓  " + window.t("create", "Create point")
                    fondo: ThemeBackend.mauve
                    letra: ThemeBackend.crust
                    enabled: !window.trabajando
                    onPulsado: window.crear()
                }
            }

            Flickable {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentHeight: lista.height
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: lista
                    width: parent.width
                    spacing: window.s(8)

                    Text {
                        visible: window.cargado && window.puntos.length === 0
                        width: lista.width
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                        topPadding: window.s(30)
                        text: window.t("empty", "There are no points yet. Create the first one; from now on one is also saved by itself when something changes.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }

                    Repeater {
                        model: window.puntos
                        delegate: Rectangle {
                            id: fila
                            required property var modelData
                            required property int index
                            readonly property var p: modelData
                            readonly property bool abierta: window.abierto === p.id
                            width: lista.width
                            height: columna.implicitHeight + window.s(24)
                            radius: window.s(12)
                            color: abierta ? Qt.alpha(ThemeBackend.surface1, 0.6) : Qt.alpha(ThemeBackend.surface0, 0.4)
                            border.width: abierta ? 1 : 0
                            border.color: Qt.alpha(ThemeBackend.mauve, 0.5)

                            ColumnLayout {
                                id: columna
                                x: window.s(14)
                                y: window.s(12)
                                width: parent.width - window.s(28)
                                spacing: window.s(8)

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: window.s(10)

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: window.s(4)
                                        RowLayout {
                                            spacing: window.s(8)
                                            Text {
                                                text: window.fecha(fila.p.fecha)
                                                font.family: ThemeBackend.fontFamily
                                                font.pixelSize: window.s(12)
                                                font.weight: Font.Bold
                                                color: ThemeBackend.subtext1
                                            }
                                            Rectangle {
                                                implicitHeight: window.s(20)
                                                implicitWidth: tipoTxt.implicitWidth + window.s(14)
                                                radius: height / 2
                                                color: Qt.alpha(window.colorTipo(fila.p.tipo), 0.18)
                                                Text {
                                                    id: tipoTxt
                                                    anchors.centerIn: parent
                                                    text: window.nombreTipo(fila.p.tipo)
                                                    font.family: ThemeBackend.fontFamily
                                                    font.pixelSize: window.s(10)
                                                    font.weight: Font.Bold
                                                    color: window.colorTipo(fila.p.tipo)
                                                }
                                            }
                                            Text {
                                                visible: fila.index === 0
                                                text: window.t("latest", "· the newest")
                                                font.family: ThemeBackend.fontFamily
                                                font.pixelSize: window.s(11)
                                                color: ThemeBackend.overlay1
                                            }
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            text: fila.p.nota
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(14)
                                            font.weight: Font.DemiBold
                                            color: ThemeBackend.text
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            visible: fila.p.partes.length > 0
                                            wrapMode: Text.WordWrap
                                            text: window.t("changed", "Changed:") + " " + fila.p.partes.map(x => window.nombreParte(x)).join(", ")
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(11)
                                            color: ThemeBackend.subtext0
                                        }
                                    }
                                    Boton {
                                        texto: fila.abierta ? window.t("cancel", "Cancel") : "󰁯  " + window.t("go_back", "Go back")
                                        enabled: !window.trabajando
                                        onPulsado: window.abierto = fila.abierta ? "" : fila.p.id
                                    }
                                }

                                Text {
                                    visible: fila.abierta
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    text: window.t("what", "What goes back to how it was at this point? What's here now is saved first, so you can undo it.")
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(12)
                                    color: ThemeBackend.subtext1
                                }
                                Flow {
                                    visible: fila.abierta
                                    Layout.fillWidth: true
                                    spacing: window.s(8)
                                    Repeater {
                                        model: ["todo"].concat(window.partes)
                                        delegate: Boton {
                                            required property string modelData
                                            texto: window.nombreParte(modelData)
                                            fondo: modelData === "todo" ? ThemeBackend.peach : ThemeBackend.surface2
                                            letra: modelData === "todo" ? ThemeBackend.crust : ThemeBackend.text
                                            enabled: !window.trabajando
                                            onPulsado: window.volver(fila.p.id, modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {
                visible: window.mensaje !== ""
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                text: window.mensaje
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                color: window.mensajeError ? ThemeBackend.red : ThemeBackend.green
            }

            Text {
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: window.t("hint", "If the desktop doesn't start: press Ctrl+Alt+F3, log in with your user and type «vortex emergencia».")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(11)
                color: ThemeBackend.overlay1
            }
        }
    }
}
