import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "../"

// 4.0: la bienvenida al prender. Al desbloquear la primera vez (el login es el bloqueo del shell), una
// tarjeta arriba al centro: el saludo, la fecha y el clima, lo que tienes para hoy (tus tareas) y
// «¿seguir con tu música?» con la canción donde te quedaste en Vortex Player. Se va sola a los 7 s (se
// queda mientras el ratón esté encima) y Shell.qml la descarga: no se queda nada en la memoria.
// Se apaga con «general.bienvenida»: false (Ajustes → General).
PanelWindow {
    id: ventana

    signal terminar()

    function s(v) { return Scaler.s(v); }
    function t(clave, porDefecto) { return I18n.t("bienvenida." + clave, porDefecto); }

    WlrLayershell.namespace: "vortex-bienvenida"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    anchors.top: true
    margins.top: s(64)
    implicitWidth: s(440)
    implicitHeight: tarjeta.implicitHeight + s(8)
    mask: Region { item: tarjeta }

    property string nombre: ""
    property string cancion: ""
    readonly property var paraHoy: Tareas.proximas.slice(0, 2)

    readonly property string saludo: {
        let h = new Date().getHours();
        let base = h >= 5 && h < 12 ? t("morning", "Good morning")
                 : h >= 12 && h < 19 ? t("afternoon", "Good afternoon")
                 : t("evening", "Good evening");
        return nombre !== "" ? base + ", " + nombre : base;
    }
    readonly property string fecha: {
        let txt = Qt.locale(I18n.currentLang || "es").toString(new Date(), "dddd d 'de' MMMM");
        return txt.charAt(0).toUpperCase() + txt.slice(1);
    }
    readonly property bool hayClima: Weather.isReady && Weather.currentTempFormatted !== "--°"

    // El nombre de la cuenta (el de verdad, no el usuario) y la canción donde te quedaste.
    Process {
        running: true
        command: ["sh", "-c", "getent passwd \"$(id -un)\" | cut -d: -f5 | cut -d, -f1; echo; id -un"]
        stdout: StdioCollector {
            onStreamFinished: {
                let l = this.text.split("\n").map(x => x.trim()).filter(x => x !== "");
                let n = (l.length > 1 ? l[0] : (l[0] || "")).split(" ")[0];
                ventana.nombre = n !== "" ? n.charAt(0).toUpperCase() + n.slice(1) : "";
            }
        }
    }
    FileView {
        path: Quickshell.env("HOME") + "/.config/vortex-player/sesion.json"
        printErrors: false
        onLoaded: {
            try {
                let c = JSON.parse(text()).cola || {};
                let rutas = c.rutas || [];
                let orden = c.orden || [];
                let i = c.i || 0;
                let r = rutas[orden.length === rutas.length && orden[i] !== undefined ? orden[i] : i] || "";
                let nombre = r.split("/").pop().replace(/\.[^.]+$/, "").replace(/\s*\[[A-Za-z0-9_-]{11}\]$/, "");
                ventana.cancion = nombre;
            } catch (e) {
                ventana.cancion = "";
            }
        }
    }

    property real entrada: 0
    NumberAnimation on entrada { id: animEntrada; from: 0; to: 1; duration: 520; easing.type: Easing.OutExpo; running: false }
    SequentialAnimation {
        id: animSalida
        NumberAnimation { target: ventana; property: "entrada"; to: 0; duration: 220; easing.type: Easing.InQuart }
        ScriptAction { script: ventana.terminar() }
    }
    function cerrar() { if (!animSalida.running) animSalida.start(); }

    Timer {
        id: reloj
        interval: 7000
        running: !zona.containsMouse && !zonaMusica.containsMouse && !animSalida.running
        onTriggered: ventana.cerrar()
    }

    Component.onCompleted: {
        Weather.refresh(false);
        animEntrada.start();
    }

    Rectangle {
        id: tarjeta
        width: parent.width - ventana.s(8)
        anchors.horizontalCenter: parent.horizontalCenter
        implicitHeight: columna.implicitHeight + ventana.s(40)
        radius: ThemeBackend.borderRadius * 2
        color: ThemeBackend.base
        border.width: 1
        border.color: ThemeBackend.surface0
        opacity: ventana.entrada
        transform: Translate { y: (1 - ventana.entrada) * -ventana.s(24) }

        MouseArea {
            id: zona
            anchors.fill: parent
            hoverEnabled: true
            onClicked: ventana.cerrar()
        }

        ColumnLayout {
            id: columna
            anchors.fill: parent
            anchors.margins: ventana.s(20)
            spacing: ventana.s(14)

            ColumnLayout {
                Layout.fillWidth: true
                spacing: ventana.s(2)
                Text {
                    Layout.fillWidth: true
                    text: ventana.saludo
                    elide: Text.ElideRight
                    font.family: ThemeBackend.titleFont
                    font.pixelSize: ventana.s(22)
                    font.weight: ThemeBackend.pesoTitulo(Font.Bold)
                    color: ThemeBackend.text
                }
                RowLayout {
                    spacing: ventana.s(8)
                    Text {
                        text: ventana.fecha
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: ventana.s(13)
                        color: ThemeBackend.subtext0
                    }
                    Text {
                        visible: ventana.hayClima
                        text: "·  " + Weather.currentIcon + "  " + Weather.currentTempFormatted
                        font.family: ThemeBackend.iconFont + ", " + ThemeBackend.fontFamily
                        font.pixelSize: ventana.s(13)
                        color: ThemeBackend.subtext0
                    }
                }
            }

            // Lo de hoy: tus tareas con fecha de hoy o vencidas (las mismas de la barra).
            ColumnLayout {
                visible: ventana.paraHoy.length > 0
                Layout.fillWidth: true
                spacing: ventana.s(6)
                Repeater {
                    model: ventana.paraHoy
                    delegate: RowLayout {
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: ventana.s(10)
                        Text {
                            text: "󰄱"
                            font.family: ThemeBackend.iconFont
                            font.pixelSize: ventana.s(15)
                            color: ThemeBackend.subtext0
                        }
                        Text {
                            Layout.fillWidth: true
                            text: modelData.texto
                            elide: Text.ElideRight
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: ventana.s(14)
                            color: ThemeBackend.text
                        }
                        Text {
                            text: Tareas.cuando(modelData)
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: ventana.s(12)
                            color: modelData.dias < 0 ? ThemeBackend.red : ThemeBackend.subtext0
                        }
                    }
                }
            }

            // «¿Seguir con tu música?»: Vortex Player abre y suena donde te quedaste.
            Rectangle {
                visible: ventana.cancion !== ""
                Layout.fillWidth: true
                implicitHeight: filaMusica.implicitHeight + ventana.s(20)
                radius: ThemeBackend.borderRadius * 1.5
                color: zonaMusica.containsMouse ? ThemeBackend.surface1 : ThemeBackend.surface0
                Behavior on color { ColorAnimation { duration: 140 } }

                RowLayout {
                    id: filaMusica
                    anchors.fill: parent
                    anchors.margins: ventana.s(10)
                    spacing: ventana.s(12)
                    Rectangle {
                        Layout.preferredWidth: ventana.s(36)
                        Layout.preferredHeight: ventana.s(36)
                        radius: width / 2
                        color: ThemeBackend.text
                        Text {
                            anchors.centerIn: parent
                            text: "󰐊"
                            font.family: ThemeBackend.iconFont
                            font.pixelSize: ventana.s(18)
                            color: ThemeBackend.base
                        }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Text {
                            text: ventana.t("music", "Continue with your music?")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: ventana.s(12)
                            color: ThemeBackend.subtext0
                        }
                        Text {
                            Layout.fillWidth: true
                            text: ventana.cancion
                            elide: Text.ElideRight
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: ventana.s(14)
                            font.weight: Font.DemiBold
                            color: ThemeBackend.text
                        }
                    }
                }
                MouseArea {
                    id: zonaMusica
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        Quickshell.execDetached(["vortex-player", "--seguir"]);
                        ventana.cerrar();
                    }
                }
            }
        }
    }
}
