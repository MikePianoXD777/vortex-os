import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.10: panel «OneDrive» (lanzador → «OneDrive»). Sin cuenta: qué es y un botón que conecta
// (vortex-onedrive conectar: instala rclone si falta, abre el navegador para iniciar sesión y monta
// ~/OneDrive). Conectado: el espacio, abrir la carpeta o la web, cómo ver lo compartido y desconectar.
// Mientras está abierto lee «vortex-onedrive estado» cada 2 s.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("onedrive." + clave, porDefecto);
    }

    property var e: ({})
    property bool cargado: false
    property bool pedido: false

    readonly property bool conectado: Boolean(e.conectado)
    readonly property bool montado: Boolean(e.montado)
    readonly property bool conectando: Boolean(e.conectando) || pedido

    function refrescar() {
        if (!estadoProc.running) estadoProc.running = true;
    }

    function correr(accion) {
        Quickshell.execDetached(["vortex-onedrive", accion]);
        refrescoPronto.restart();
    }

    function gb(bytes) {
        return (bytes / 1073741824).toFixed(bytes >= 107374182400 ? 0 : 1) + " GB";
    }

    Process {
        id: estadoProc
        command: ["vortex-onedrive", "estado"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    window.e = JSON.parse(this.text);
                    if (window.e.conectando || window.e.conectado || window.e.error) window.pedido = false;
                } catch (err) {}
                window.cargado = true;
            }
        }
    }

    Timer {
        interval: 2000
        repeat: true
        running: window.visible
        onTriggered: window.refrescar()
    }
    Timer {
        id: refrescoPronto
        interval: 900
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

    component Boton: Rectangle {
        id: boton
        property string icono: ""
        property string texto: ""
        property color tinte: ThemeBackend.blue
        property bool principal: false
        signal pulsado()
        implicitHeight: window.s(36)
        implicitWidth: fila.implicitWidth + window.s(28)
        radius: height / 2
        color: principal ? (zona.containsMouse ? Qt.lighter(tinte, 1.1) : tinte)
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

    component Parrafo: Text {
        Layout.fillWidth: true
        wrapMode: Text.WordWrap
        font.family: ThemeBackend.fontFamily
        font.pixelSize: window.s(13)
        color: ThemeBackend.subtext1
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
            spacing: window.s(14)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(12)
                Text {
                    text: "󰏊"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(32)
                    color: ThemeBackend.blue
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: "OneDrive"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(20)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: window.conectando ? window.t("connecting", "Connecting…")
                            : !window.conectado ? window.t("not_connected", "Not connected")
                            : !window.montado ? window.t("not_mounted", "Connected, but the folder isn't available")
                            : window.e.tipo === "personal" ? window.t("connected_personal", "Connected · personal account")
                            : window.e.tipo ? window.t("connected_work", "Connected · work or school account")
                            : window.t("connected", "Connected")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: window.conectado && window.montado ? ThemeBackend.green : ThemeBackend.subtext0
                    }
                }
            }

            // Error de la última vez.
            Rectangle {
                visible: Boolean(window.e.error) && !window.conectando
                Layout.fillWidth: true
                implicitHeight: textoError.implicitHeight + window.s(20)
                radius: window.s(10)
                color: Qt.alpha(ThemeBackend.red, 0.12)
                border.width: 1
                border.color: Qt.alpha(ThemeBackend.red, 0.4)
                Text {
                    id: textoError
                    anchors.fill: parent
                    anchors.margins: window.s(10)
                    wrapMode: Text.WordWrap
                    text: window.e.error || ""
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(12)
                    color: ThemeBackend.text
                }
            }

            // Conectando: qué hacer en el navegador.
            ColumnLayout {
                visible: window.conectando
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)
                Parrafo {
                    text: window.t("browser", "A browser window opened: sign in with your Microsoft account and press «Accept». It works with personal, work and school accounts.")
                }
                Parrafo {
                    text: window.t("browser_install", "If it asks for your password first, it's to install what OneDrive needs (only the first time).")
                    color: ThemeBackend.subtext0
                    font.pixelSize: window.s(12)
                }
                Item { Layout.fillHeight: true }
            }

            // Sin conectar.
            ColumnLayout {
                visible: !window.conectando && !window.conectado && window.cargado
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)
                Parrafo {
                    text: window.t("intro", "Your OneDrive files as a normal folder, «OneDrive» in your home: open them, edit them and save; what you save goes up by itself. Nothing is downloaded until you open it.")
                }
                Item { Layout.fillHeight: true }
                Boton {
                    Layout.alignment: Qt.AlignHCenter
                    principal: true
                    icono: "󰌹"
                    texto: window.t("connect", "Connect OneDrive")
                    onPulsado: {
                        window.pedido = true;
                        window.correr("conectar");
                    }
                }
            }

            // Conectado.
            ColumnLayout {
                visible: !window.conectando && window.conectado
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(14)

                ColumnLayout {
                    visible: window.e.total > 0
                    Layout.fillWidth: true
                    spacing: window.s(6)
                    Text {
                        text: window.t("space", "{u} of {t} used").replace("{u}", window.gb(window.e.usado || 0)).replace("{t}", window.gb(window.e.total || 1))
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext1
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        implicitHeight: window.s(8)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface1, 0.8)
                        Rectangle {
                            height: parent.height
                            radius: parent.radius
                            width: parent.width * Math.min(1, Math.max(0, (window.e.usado || 0) / Math.max(1, window.e.total || 1)))
                            color: (window.e.usado || 0) / Math.max(1, window.e.total || 1) > 0.9 ? ThemeBackend.red : ThemeBackend.blue
                        }
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: window.s(8)
                    Boton {
                        visible: window.montado
                        principal: true
                        icono: "󰉋"
                        texto: window.t("open_folder", "Open folder")
                        onPulsado: window.correr("abrir")
                    }
                    Boton {
                        visible: !window.montado
                        principal: true
                        icono: "󰑐"
                        texto: window.t("retry", "Try again")
                        onPulsado: window.correr("montar")
                    }
                    Boton {
                        icono: "󰖟"
                        texto: window.t("open_web", "Open on the web")
                        onPulsado: Quickshell.execDetached(["xdg-open", "https://onedrive.live.com/"])
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: compartido.implicitHeight + window.s(24)
                    radius: window.s(12)
                    color: Qt.alpha(ThemeBackend.surface0, 0.5)
                    ColumnLayout {
                        id: compartido
                        x: window.s(12)
                        y: window.s(12)
                        width: parent.width - window.s(24)
                        spacing: window.s(4)
                        Text {
                            text: "󰉌  " + window.t("shared_title", "Did someone share something with you?")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            font.weight: Font.Bold
                            color: ThemeBackend.text
                        }
                        Parrafo {
                            font.pixelSize: window.s(12)
                            text: window.t("shared_desc", "Shared files don't show up in the folder by themselves. On the web, go to «Shared», right-click it and choose «Add shortcut to My files»: from then on it's in your OneDrive folder.")
                        }
                    }
                }

                Item { Layout.fillHeight: true }

                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: window.t("disconnect", "Disconnect OneDrive from this computer")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(12)
                    font.underline: zonaDesc.containsMouse
                    color: ThemeBackend.overlay1
                    MouseArea {
                        id: zonaDesc
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: window.correr("desconectar")
                    }
                }
            }
        }
    }
}
