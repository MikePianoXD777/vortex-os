import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex (1.7.7): hoja de atajos de teclado (Super+F1, o «Atajos de teclado» en el lanzador), para
// quien llega a Linux por primera vez. La lista está en assets/atajos.json, por secciones, con el
// texto en español e inglés; pruebas/atajos.lua comprueba que cada atajo exista de verdad. Se
// escribe para buscar (en la descripción o en las teclas).
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    property var secciones: []
    property string filtro: ""
    readonly property bool espanol: (typeof I18n === "undefined") || I18n.currentLang !== "en"

    function texto(obj) {
        if (!obj) return "";
        return (espanol ? obj.es : obj.en) || obj.es || "";
    }

    // Nombre de cada tecla tal como se ve en el teclado.
    readonly property var nombresEs: ({
        "SUPER": "Super", "CTRL": "Ctrl", "ALT": "Alt", "SHIFT": "Shift", "TAB": "Tab",
        "Space": "Espacio", "Print": "Impr", "Escape": "Esc", "Delete": "Supr", "Period": ".",
        "mouse:272": "clic izquierdo", "mouse:273": "clic derecho", "mouse_wheel": "rueda"
    })
    readonly property var nombresEn: ({
        "SUPER": "Super", "CTRL": "Ctrl", "ALT": "Alt", "SHIFT": "Shift", "TAB": "Tab",
        "Space": "Space", "Print": "Print Screen", "Escape": "Esc", "Delete": "Del", "Period": ".",
        "mouse:272": "left click", "mouse:273": "right click", "mouse_wheel": "wheel"
    })

    function teclasDe(atajo) {
        let combo = atajo.ver || (atajo.teclas && atajo.teclas[0]) || "";
        let nombres = espanol ? nombresEs : nombresEn;
        return combo.split("+").map(t => t.trim()).filter(t => t !== "").map(t => nombres[t] || t);
    }

    // Sin acentos ni mayúsculas, para que «capturar» encuentre «Capturar» y «ventana» a «Ventana».
    function normalizar(t) {
        return String(t).toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
    }

    function coincide(atajo) {
        if (filtro.trim() === "") return true;
        let f = normalizar(filtro.trim());
        return normalizar(texto(atajo)).indexOf(f) !== -1 || normalizar(teclasDe(atajo).join(" ")).indexOf(f) !== -1;
    }

    function cuantos(seccion) {
        let n = 0;
        for (let i = 0; i < seccion.atajos.length; i++) if (coincide(seccion.atajos[i])) n++;
        return n;
    }

    FileView {
        path: Caching.serpantinumDir ? (Caching.serpantinumDir + "/assets/atajos.json") : ""
        onLoaded: {
            try {
                let data = JSON.parse(text());
                if (Array.isArray(data)) window.secciones = data;
            } catch (e) {}
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
            focusTimer.restart();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
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
        transform: Translate { y: window.s(20) * (1 - introMain) }

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
            spacing: window.s(16)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Text {
                    text: "󰌌"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(34)
                    color: ThemeBackend.blue
                }

                ColumnLayout {
                    spacing: window.s(2)
                    Text {
                        text: window.espanol ? "Atajos de teclado" : "Keyboard shortcuts"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        text: window.espanol ? "Super es la tecla de Windows. Esta hoja se abre con Super + F1."
                                             : "Super is the Windows key. Open this sheet with Super + F1."
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }
                }

                Item { Layout.fillWidth: true }

                Rectangle {
                    Layout.preferredWidth: window.s(300)
                    Layout.preferredHeight: window.s(40)
                    radius: height / 2
                    color: Qt.alpha(ThemeBackend.surface0, 0.7)
                    border.width: busqueda.activeFocus ? window.s(2) : 0
                    border.color: ThemeBackend.blue

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

                        Text {
                            anchors.fill: parent
                            visible: busqueda.text === ""
                            text: window.espanol ? "Buscar: captura, ventana, wifi…" : "Search: screenshot, window, Wi-Fi…"
                            font: busqueda.font
                            color: ThemeBackend.overlay1
                            elide: Text.ElideRight
                        }
                    }
                }
            }

            Flickable {
                id: lista
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentWidth: width
                contentHeight: columnas.height
                boundsBehavior: Flickable.StopAtBounds

                // Tres columnas; cada sección va a la columna más corta, como un mosaico.
                Row {
                    id: columnas
                    width: lista.width
                    spacing: window.s(14)

                    Repeater {
                        model: 3
                        delegate: Column {
                            id: columna
                            required property int index
                            width: (columnas.width - columnas.spacing * 2) / 3
                            spacing: window.s(14)

                            Repeater {
                                model: window.secciones.filter((sec, i) => i % 3 === columna.index)
                                delegate: Rectangle {
                                    id: tarjeta
                                    required property var modelData
                                    width: columna.width
                                    visible: window.cuantos(modelData) > 0
                                    height: visible ? contenido.implicitHeight + window.s(28) : 0
                                    radius: ThemeBackend.borderRadius
                                    color: Qt.alpha(ThemeBackend.surface0, 0.45)

                                    Column {
                                        id: contenido
                                        x: window.s(14)
                                        y: window.s(14)
                                        width: parent.width - window.s(28)
                                        spacing: window.s(10)

                                        Row {
                                            spacing: window.s(8)
                                            Text {
                                                text: tarjeta.modelData.icono || ""
                                                font.family: ThemeBackend.iconFont
                                                font.pixelSize: window.s(18)
                                                color: ThemeBackend.mauve
                                            }
                                            Text {
                                                text: window.texto(tarjeta.modelData)
                                                font.family: ThemeBackend.fontFamily
                                                font.pixelSize: window.s(15)
                                                font.weight: Font.Bold
                                                color: ThemeBackend.text
                                            }
                                        }

                                        Repeater {
                                            model: tarjeta.modelData.atajos
                                            delegate: RowLayout {
                                                id: fila
                                                required property var modelData
                                                width: contenido.width
                                                visible: window.coincide(modelData)
                                                spacing: window.s(10)

                                                Flow {
                                                    Layout.preferredWidth: contenido.width * 0.5
                                                    Layout.alignment: Qt.AlignTop
                                                    spacing: window.s(3)

                                                    Repeater {
                                                        model: window.teclasDe(fila.modelData)
                                                        delegate: Rectangle {
                                                            required property string modelData
                                                            width: tecla.implicitWidth + window.s(10)
                                                            height: window.s(22)
                                                            radius: window.s(6)
                                                            color: Qt.alpha(ThemeBackend.surface1, 0.9)
                                                            border.width: window.s(1)
                                                            border.color: Qt.alpha(ThemeBackend.overlay0, 0.6)

                                                            Text {
                                                                id: tecla
                                                                anchors.centerIn: parent
                                                                text: parent.modelData
                                                                font.family: ThemeBackend.fontFamily
                                                                font.pixelSize: window.s(11)
                                                                font.weight: Font.DemiBold
                                                                color: ThemeBackend.text
                                                            }
                                                        }
                                                    }
                                                }

                                                Text {
                                                    Layout.fillWidth: true
                                                    Layout.alignment: Qt.AlignVCenter
                                                    text: window.texto(fila.modelData)
                                                    wrapMode: Text.WordWrap
                                                    font.family: ThemeBackend.fontFamily
                                                    font.pixelSize: window.s(13)
                                                    color: ThemeBackend.subtext1
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {
                Layout.fillWidth: true
                visible: window.secciones.length > 0 && window.secciones.every(sec => window.cuantos(sec) === 0)
                horizontalAlignment: Text.AlignHCenter
                text: window.espanol ? "Ningún atajo con «" + window.filtro + "»" : "No shortcut matches “" + window.filtro + "”"
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(14)
                color: ThemeBackend.subtext0
            }
        }
    }
}
