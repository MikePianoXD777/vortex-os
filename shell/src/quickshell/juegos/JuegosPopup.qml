import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.8: biblioteca de juegos (Super+Shift+G, o «Tus juegos» en el lanzador). Junta en un solo
// lugar los juegos de Steam y Heroic (juegos.sh, con sus portadas; lo último que jugaste primero) y
// las apps marcadas como juego (Categories=Game: Minecraft, Roblox con Sober, Flatpaks…). Un clic lo
// abre y cierra el panel; el modo juego entra solo (vortex-gestor). Se escribe para buscar y Enter
// abre el primero.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    property var tiendas: []
    property string filtro: ""
    property bool cargado: false

    function t(clave, porDefecto) {
        return I18n.t("juegos." + clave, porDefecto);
    }

    // Las apps con Categories=Game, menos los accesos directos que Steam y Heroic crean para sus
    // juegos (esos ya salen con portada).
    function appsDeJuego() {
        let arr = [];
        if (typeof DesktopEntries === "undefined" || !DesktopEntries.applications || !DesktopEntries.applications.values) return arr;
        let entries = DesktopEntries.applications.values;
        for (let i = 0; i < entries.length; i++) {
            let e = entries[i];
            if (e.noDisplay || !e.categories || e.categories.indexOf("Game") < 0) continue;
            let exec = e.execString || "";
            if (exec.indexOf("steam://rungameid") >= 0 || exec.indexOf("heroic://launch") >= 0) continue;
            arr.push({ id: "app:" + e.id, nombre: e.name || e.id, origen: "app", portada: "", vertical: false,
                       icono: e.icon || "", entrada: e, ultimo: 0 });
        }
        arr.sort((a, b) => a.nombre.toLowerCase().localeCompare(b.nombre.toLowerCase()));
        return arr;
    }

    property int revApps: 0
    Connections {
        target: (typeof DesktopEntries !== "undefined" && DesktopEntries.applications) ? DesktopEntries.applications : null
        function onValuesChanged() { window.revApps++; }
    }

    readonly property var todos: {
        let dummy = window.revApps;
        return window.tiendas.concat(window.appsDeJuego());
    }

    readonly property var visibles: {
        let f = window.filtro.trim().toLowerCase();
        if (f === "") return window.todos;
        return window.todos.filter(j => j.nombre.toLowerCase().indexOf(f) >= 0);
    }

    function abrir(j) {
        if (!j) return;
        if (j.origen === "app") {
            if (j.entrada) j.entrada.execute();
        } else {
            Quickshell.execDetached(["xdg-open", j.lanzar]);
        }
        Quickshell.execDetached(["hyprctl", "dispatch", "hl.dsp.event(\"serpantinum:close\")"]);
    }

    Process {
        id: lista
        command: ["bash", Caching.qsDir + "/juegos/juegos.sh"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.tiendas = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.tiendas = [];
                }
                window.cargado = true;
            }
        }
    }

    function recargar() {
        if (!lista.running) lista.running = true;
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

    Timer {
        id: focusTimer
        interval: 50
        repeat: false
        onTriggered: busqueda.forceActiveFocus()
    }

    onVisibleChanged: {
        if (visible) {
            filtro = "";
            busqueda.text = "";
            recargar();
            focusTimer.restart();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        recargar();
        if (visible) {
            focusTimer.restart();
            resetAndPlayIntro();
        }
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
            spacing: window.s(18)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Text {
                    text: "󰊴"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(32)
                    color: ThemeBackend.mauve
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Your games")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        text: window.cargado
                            ? (window.todos.length === 1 ? window.t("count_one", "1 game") : window.t("count", "{n} games").replace("{n}", window.todos.length))
                            : window.t("loading", "Looking for your games…")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                    }
                }

                Rectangle {
                    Layout.preferredWidth: window.s(300)
                    Layout.preferredHeight: window.s(40)
                    radius: height / 2
                    color: Qt.alpha(ThemeBackend.surface0, 0.7)
                    border.width: busqueda.activeFocus ? window.s(2) : 0
                    border.color: ThemeBackend.mauve

                    Text {
                        id: lupa
                        anchors.left: parent.left
                        anchors.leftMargin: window.s(14)
                        anchors.verticalCenter: parent.verticalCenter
                        text: "󰍉"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(16)
                        color: ThemeBackend.subtext0
                    }

                    TextInput {
                        id: busqueda
                        anchors.left: lupa.right
                        anchors.right: parent.right
                        anchors.leftMargin: window.s(10)
                        anchors.rightMargin: window.s(14)
                        anchors.verticalCenter: parent.verticalCenter
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(14)
                        color: ThemeBackend.text
                        clip: true
                        onTextChanged: window.filtro = text
                        Keys.onReturnPressed: window.abrir(window.visibles[0])
                        Keys.onEnterPressed: window.abrir(window.visibles[0])

                        Text {
                            anchors.fill: parent
                            visible: busqueda.text === ""
                            text: window.t("search", "Search your games…")
                            font: busqueda.font
                            color: ThemeBackend.overlay1
                            elide: Text.ElideRight
                        }
                    }
                }
            }

            Text {
                visible: window.cargado && window.visibles.length === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: window.todos.length === 0
                    ? window.t("empty", "No games yet. Install them with Steam or Heroic (or a Flatpak from the Shop) and they will show up here.")
                    : window.t("no_match", "No game matches your search.")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(14)
                color: ThemeBackend.subtext0
            }

            GridView {
                id: rejilla
                visible: window.visibles.length > 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                model: window.visibles

                readonly property int columnas: Math.max(1, Math.floor(width / window.s(170)))
                cellWidth: Math.floor(width / columnas)
                cellHeight: cellWidth * 1.5 + window.s(34)

                delegate: Item {
                    id: celda
                    required property var modelData
                    required property int index
                    readonly property var j: modelData
                    width: rejilla.cellWidth
                    height: rejilla.cellHeight

                    Item {
                        id: tarjeta
                        anchors.horizontalCenter: parent.horizontalCenter
                        y: window.s(4)
                        width: celda.width - window.s(16)
                        height: width * 1.5
                        scale: zona.containsMouse ? 1.04 : 1.0
                        Behavior on scale { NumberAnimation { duration: 140; easing.type: Easing.OutQuad } }

                        Rectangle {
                            id: fondo
                            anchors.fill: parent
                            radius: window.s(12)
                            color: Qt.alpha(ThemeBackend.surface0, 0.8)
                            border.width: zona.containsMouse || (celda.index === 0 && window.filtro !== "") ? window.s(2) : 0
                            border.color: ThemeBackend.mauve
                            clip: true

                            Image {
                                id: portada
                                anchors.fill: parent
                                anchors.margins: fondo.border.width
                                visible: celda.j.portada !== "" && status === Image.Ready
                                source: celda.j.portada === "" ? ""
                                    : (celda.j.portada.indexOf("http") === 0 ? celda.j.portada : "file://" + celda.j.portada)
                                sourceSize.width: window.s(300)
                                fillMode: Image.PreserveAspectCrop
                                asynchronous: true
                                cache: true
                            }

                            // Sin portada (apps): el ícono grande con el nombre.
                            Column {
                                visible: !portada.visible
                                anchors.centerIn: parent
                                width: parent.width - window.s(20)
                                spacing: window.s(12)
                                Image {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    width: window.s(72)
                                    height: width
                                    sourceSize.width: width
                                    sourceSize.height: height
                                    source: celda.j.icono ? (Quickshell.iconPath(celda.j.icono, true) || "") : ""
                                    visible: status === Image.Ready
                                    asynchronous: true
                                }
                                Text {
                                    width: parent.width
                                    horizontalAlignment: Text.AlignHCenter
                                    wrapMode: Text.WordWrap
                                    maximumLineCount: 3
                                    elide: Text.ElideRight
                                    text: celda.j.nombre
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(14)
                                    font.weight: Font.DemiBold
                                    color: ThemeBackend.text
                                }
                            }

                            // De dónde viene.
                            Rectangle {
                                visible: celda.j.origen !== "app"
                                anchors.top: parent.top
                                anchors.left: parent.left
                                anchors.margins: window.s(8)
                                width: chip.implicitWidth + window.s(14)
                                height: window.s(20)
                                radius: height / 2
                                color: Qt.alpha(ThemeBackend.crust, 0.75)
                                Text {
                                    id: chip
                                    anchors.centerIn: parent
                                    text: celda.j.origen === "steam" ? "Steam" : "Heroic"
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(10)
                                    font.weight: Font.DemiBold
                                    color: ThemeBackend.text
                                }
                            }

                            // Al pasar el mouse: el botón de jugar.
                            Rectangle {
                                anchors.fill: parent
                                radius: parent.radius
                                color: Qt.alpha(ThemeBackend.crust, 0.35)
                                opacity: zona.containsMouse ? 1 : 0
                                Behavior on opacity { NumberAnimation { duration: 140 } }
                                Rectangle {
                                    anchors.centerIn: parent
                                    width: window.s(56)
                                    height: width
                                    radius: width / 2
                                    color: ThemeBackend.mauve
                                    Text {
                                        anchors.centerIn: parent
                                        anchors.horizontalCenterOffset: window.s(2)
                                        text: "󰐊"
                                        font.family: ThemeBackend.iconFont
                                        font.pixelSize: window.s(30)
                                        color: ThemeBackend.crust
                                    }
                                }
                            }
                        }

                        MouseArea {
                            id: zona
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: window.abrir(celda.j)
                        }
                    }

                    Text {
                        anchors.top: tarjeta.bottom
                        anchors.topMargin: window.s(8)
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: tarjeta.width
                        horizontalAlignment: Text.AlignHCenter
                        elide: Text.ElideRight
                        text: celda.j.nombre
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: zona.containsMouse ? ThemeBackend.text : ThemeBackend.subtext1
                    }
                }
            }
        }
    }
}
