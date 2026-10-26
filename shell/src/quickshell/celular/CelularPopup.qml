import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.8: panel «Tu celular» (Super+Shift+P), con KDE Connect. Muestra los celulares (batería,
// conectado o no) y sus acciones: vincular, hacerlo sonar, mandarle archivos o el portapapeles. Las
// notificaciones del celular llegan solas al centro de notificaciones (las manda kdeconnectd). KDE
// Connect viene apagado (~50 MB): si lo está, el panel explica qué es y lo enciende con un botón
// («vortex celular on»). Los datos y las acciones van por celular.sh.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    readonly property string script: Caching.qsDir + "/celular/celular.sh"
    property bool activo: false
    property var dispositivos: []
    property bool cargado: false
    property bool encendiendo: false

    function t(clave, porDefecto) {
        return I18n.t("celular." + clave, porDefecto);
    }

    function refrescar() {
        if (!estadoProc.running) estadoProc.running = true;
    }

    function accion(nombre, id) {
        Quickshell.execDetached(["bash", window.script, nombre, id]);
        refrescoPronto.restart();
    }

    Process {
        id: estadoProc
        command: ["bash", window.script, "estado"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.activo = Boolean(d.activo);
                    window.dispositivos = Array.isArray(d.dispositivos) ? d.dispositivos : [];
                    if (window.activo) window.encendiendo = false;
                } catch (e) {}
                window.cargado = true;
            }
        }
    }

    // Mientras está abierto, cada 3 s (batería, si se conectó o desconectó).
    Timer {
        interval: 3000
        repeat: true
        running: window.visible
        onTriggered: window.refrescar()
    }
    Timer {
        id: refrescoPronto
        interval: 1200
        onTriggered: window.refrescar()
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
            refrescar();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        refrescar();
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

    // Botón chico con ícono y texto.
    component Boton: Rectangle {
        id: boton
        property string icono: ""
        property string texto: ""
        property color tinte: ThemeBackend.blue
        property bool principal: false
        signal pulsado()
        implicitHeight: window.s(34)
        implicitWidth: fila.implicitWidth + window.s(24)
        radius: height / 2
        color: principal ? tinte : (zona.containsMouse ? Qt.alpha(tinte, 0.25) : Qt.alpha(ThemeBackend.surface1, 0.8))

        Row {
            id: fila
            anchors.centerIn: parent
            spacing: window.s(6)
            Text {
                visible: boton.icono !== ""
                text: boton.icono
                font.family: ThemeBackend.iconFont
                font.pixelSize: window.s(15)
                color: boton.principal ? ThemeBackend.crust : boton.tinte
                anchors.verticalCenter: parent.verticalCenter
            }
            Text {
                text: boton.texto
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                font.weight: Font.DemiBold
                color: boton.principal ? ThemeBackend.crust : ThemeBackend.text
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
            anchors.margins: window.s(20)
            spacing: window.s(14)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(12)

                Text {
                    text: "󰄜"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(30)
                    color: ThemeBackend.green
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Your phone")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(20)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        text: window.t("subtitle", "With KDE Connect, over your Wi-Fi")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                        elide: Text.ElideRight
                    }
                }
                Boton {
                    visible: window.activo
                    icono: "󰑐"
                    texto: window.t("search", "Search")
                    onPulsado: window.accion("buscar", "")
                }
            }

            // Apagado: qué es y cómo encenderlo.
            ColumnLayout {
                visible: window.cargado && !window.activo
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)

                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: window.t("intro", "See your phone's notifications here, send it files or what you copied, and make it ring when you can't find it.")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(13)
                    color: ThemeBackend.subtext1
                }
                Repeater {
                    model: [
                        window.t("step1", "Install KDE Connect on your phone (Play Store or F-Droid)."),
                        window.t("step2", "Connect it to the same Wi-Fi as this PC."),
                        window.t("step3", "Turn it on here and accept the pairing on your phone.")
                    ]
                    delegate: RowLayout {
                        required property string modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: window.s(10)
                        Rectangle {
                            Layout.preferredWidth: window.s(24)
                            Layout.preferredHeight: window.s(24)
                            radius: width / 2
                            color: Qt.alpha(ThemeBackend.green, 0.2)
                            Text {
                                anchors.centerIn: parent
                                text: String(index + 1)
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(12)
                                font.weight: Font.Bold
                                color: ThemeBackend.green
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            text: modelData
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            color: ThemeBackend.text
                        }
                    }
                }
                Item { Layout.fillHeight: true }
                Boton {
                    Layout.alignment: Qt.AlignHCenter
                    principal: true
                    tinte: ThemeBackend.green
                    icono: window.encendiendo ? "󰔟" : "󰐥"
                    texto: window.encendiendo ? window.t("turning_on", "Turning on…") : window.t("turn_on", "Turn on")
                    onPulsado: {
                        if (window.encendiendo) return;
                        window.encendiendo = true;
                        Quickshell.execDetached(["vortex", "celular", "on"]);
                        refrescoPronto.restart();
                    }
                }
            }

            // Encendido sin celulares todavía.
            Text {
                visible: window.activo && window.dispositivos.length === 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                wrapMode: Text.WordWrap
                verticalAlignment: Text.AlignVCenter
                horizontalAlignment: Text.AlignHCenter
                text: window.t("empty", "No phones yet. Open KDE Connect on your phone, on the same Wi-Fi, and it will show up here.")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(13)
                color: ThemeBackend.subtext0
            }

            Flickable {
                visible: window.activo && window.dispositivos.length > 0
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                contentHeight: lista.height
                boundsBehavior: Flickable.StopAtBounds

                Column {
                    id: lista
                    width: parent.width
                    spacing: window.s(10)

                    Repeater {
                        model: window.dispositivos
                        delegate: Rectangle {
                            id: tarjeta
                            required property var modelData
                            readonly property var d: modelData
                            width: lista.width
                            height: contenido.implicitHeight + window.s(24)
                            radius: ThemeBackend.borderRadius
                            color: Qt.alpha(ThemeBackend.surface0, 0.5)

                            ColumnLayout {
                                id: contenido
                                x: window.s(12)
                                y: window.s(12)
                                width: parent.width - window.s(24)
                                spacing: window.s(10)

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: window.s(10)
                                    Text {
                                        text: tarjeta.d.tipo === "tablet" ? "󰓶" : "󰄜"
                                        font.family: ThemeBackend.iconFont
                                        font.pixelSize: window.s(24)
                                        color: tarjeta.d.alcanzable ? ThemeBackend.green : ThemeBackend.overlay1
                                    }
                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: 0
                                        Text {
                                            Layout.fillWidth: true
                                            text: tarjeta.d.nombre
                                            elide: Text.ElideRight
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(14)
                                            font.weight: Font.Bold
                                            color: ThemeBackend.text
                                        }
                                        Text {
                                            text: {
                                                if (tarjeta.d.pide) return window.t("wants_pair", "Wants to pair");
                                                if (!tarjeta.d.vinculado) return window.t("not_paired", "Not paired");
                                                if (!tarjeta.d.alcanzable) return window.t("offline", "Not connected");
                                                return window.t("connected", "Connected");
                                            }
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(12)
                                            color: tarjeta.d.pide ? ThemeBackend.peach : ThemeBackend.subtext0
                                        }
                                    }
                                    Text {
                                        visible: tarjeta.d.vinculado && tarjeta.d.bateria >= 0
                                        text: (tarjeta.d.cargando ? "󰂄 " : "󰁹 ") + tarjeta.d.bateria + "%"
                                        font.family: ThemeBackend.fontFamily
                                        font.pixelSize: window.s(13)
                                        color: tarjeta.d.bateria <= 20 && !tarjeta.d.cargando ? ThemeBackend.red : ThemeBackend.text
                                    }
                                }

                                Flow {
                                    Layout.fillWidth: true
                                    spacing: window.s(6)

                                    Boton {
                                        visible: !tarjeta.d.vinculado
                                        principal: true
                                        tinte: tarjeta.d.pide ? ThemeBackend.peach : ThemeBackend.green
                                        icono: "󰌹"
                                        texto: tarjeta.d.pide ? window.t("accept", "Accept") : window.t("pair", "Pair")
                                        onPulsado: window.accion("vincular", tarjeta.d.id)
                                    }
                                    Boton {
                                        visible: tarjeta.d.vinculado && tarjeta.d.alcanzable
                                        icono: "󰂞"
                                        texto: window.t("ring", "Ring")
                                        onPulsado: window.accion("sonar", tarjeta.d.id)
                                    }
                                    Boton {
                                        visible: tarjeta.d.vinculado && tarjeta.d.alcanzable
                                        icono: "󰈔"
                                        texto: window.t("send_file", "Send files")
                                        onPulsado: window.accion("archivo", tarjeta.d.id)
                                    }
                                    Boton {
                                        visible: tarjeta.d.vinculado && tarjeta.d.alcanzable
                                        icono: "󰅍"
                                        texto: window.t("send_clipboard", "Send clipboard")
                                        onPulsado: window.accion("portapapeles", tarjeta.d.id)
                                    }
                                    Boton {
                                        visible: tarjeta.d.vinculado
                                        tinte: ThemeBackend.red
                                        icono: "󰌺"
                                        texto: window.t("unpair", "Unpair")
                                        onPulsado: window.accion("desvincular", tarjeta.d.id)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Text {
                visible: window.activo
                Layout.alignment: Qt.AlignHCenter
                text: window.t("turn_off", "Turn off KDE Connect")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                font.underline: apagar.containsMouse
                color: ThemeBackend.overlay1
                MouseArea {
                    id: apagar
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        Quickshell.execDetached(["vortex", "celular", "off"]);
                        refrescoPronto.restart();
                    }
                }
            }
        }
    }
}
