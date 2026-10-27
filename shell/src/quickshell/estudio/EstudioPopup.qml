import QtQuick
import QtQuick.Layouts
import Quickshell
import "../"
import "../reusables"

// Vortex 1.10: panel «Temporizador de estudio» (lanzador → «Estudio», o clic en la cuenta de la barra).
// Solo muestra y maneja el singleton Estudio, que es el que cuenta: cerrar el panel no lo detiene.
// Espacio empieza, pausa y sigue. Mientras corre, un clic afuera no lo cierra (Main.qml lee
// mantenerAbierto): se cierra con Esc o con la ✕.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("estudio." + clave, porDefecto);
    }

    readonly property var opciones: [
        { estudio: 25, descanso: 5 }, { estudio: 50, descanso: 10 }, { estudio: 15, descanso: 3 }
    ]
    readonly property real total: (Estudio.fase === "descanso"
        ? (Estudio.sesionesHoy % 4 === 0 ? Estudio.minutosDescansoLargo : Estudio.minutosDescanso)
        : Estudio.minutosEstudio) * 60000
    readonly property real avance: Estudio.activo ? Math.max(0, Math.min(1, 1 - Estudio.restante / Math.max(1, total))) : 0
    readonly property bool mantenerAbierto: Estudio.activo
    readonly property color tinte: Estudio.fase === "descanso" ? ThemeBackend.green : ThemeBackend.mauve

    onAvanceChanged: anillo.requestPaint()
    onTinteChanged: anillo.requestPaint()

    function principal() {
        if (!Estudio.activo) Estudio.empezar();
        else if (Estudio.pausado) Estudio.seguir();
        else Estudio.pausar();
    }

    Keys.onPressed: event => {
        if (event.key === Qt.Key_Space) {
            principal();
            event.accepted = true;
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
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
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
        property color tinte: window.tinte
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
                    text: "󰔛"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(30)
                    color: window.tinte
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Study timer")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(18)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: window.t("subtitle", "Study in blocks with breaks in between. It keeps counting in the bar.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                    }
                }
                Boton {
                    Layout.alignment: Qt.AlignTop
                    icono: "󰅖"
                    tinte: ThemeBackend.overlay1
                    onPulsado: Quickshell.execDetached(["hyprctl", "dispatch", "hl.dsp.event(\"serpantinum:close\")"])
                }
            }

            // El anillo con lo que falta.
            Item {
                Layout.alignment: Qt.AlignHCenter
                implicitWidth: window.s(210)
                implicitHeight: window.s(210)

                Canvas {
                    id: anillo
                    anchors.fill: parent
                    onPaint: {
                        let ctx = getContext("2d");
                        ctx.reset();
                        let r = width / 2 - window.s(10);
                        ctx.lineWidth = window.s(12);
                        ctx.lineCap = "round";
                        ctx.strokeStyle = Qt.alpha(ThemeBackend.surface1, 0.8);
                        ctx.beginPath();
                        ctx.arc(width / 2, height / 2, r, 0, 2 * Math.PI);
                        ctx.stroke();
                        if (window.avance > 0) {
                            ctx.strokeStyle = window.tinte;
                            ctx.beginPath();
                            ctx.arc(width / 2, height / 2, r, -Math.PI / 2, -Math.PI / 2 + 2 * Math.PI * window.avance);
                            ctx.stroke();
                        }
                    }
                }

                ColumnLayout {
                    anchors.centerIn: parent
                    spacing: window.s(2)
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: Estudio.activo ? Estudio.texto(Estudio.restante) : Estudio.minutosEstudio + ":00"
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(52)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: !Estudio.activo ? window.t("ready", "Ready")
                            : Estudio.pausado ? window.t("paused", "Paused")
                            : Estudio.fase === "estudio" ? window.t("studying", "Studying")
                            : window.t("resting", "Break")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(14)
                        font.weight: Font.DemiBold
                        color: window.tinte
                    }
                }
            }

            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: window.s(10)
                Boton {
                    principal: true
                    implicitWidth: window.s(140)
                    implicitHeight: window.s(44)
                    icono: !Estudio.activo || Estudio.pausado ? "󰐊" : "󰏤"
                    texto: !Estudio.activo ? window.t("start", "Start") : Estudio.pausado ? window.t("resume", "Resume") : window.t("pause", "Pause")
                    onPulsado: window.principal()
                }
                Boton {
                    visible: Estudio.activo
                    implicitHeight: window.s(44)
                    icono: "󰒭"
                    texto: Estudio.fase === "estudio" ? window.t("skip_to_break", "Break now") : window.t("skip_to_study", "Study now")
                    onPulsado: Estudio.saltar()
                }
                Boton {
                    visible: Estudio.activo
                    implicitHeight: window.s(44)
                    icono: "󰓛"
                    tinte: ThemeBackend.red
                    onPulsado: Estudio.parar()
                }
            }

            // Cuánto dura cada bloque: minutos de estudio / de descanso.
            ColumnLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: window.s(6)
                Row {
                    Layout.alignment: Qt.AlignHCenter
                    spacing: window.s(6)
                    Repeater {
                        model: window.opciones
                        Boton {
                            required property var modelData
                            texto: modelData.estudio + " / " + modelData.descanso
                            elegido: Estudio.minutosEstudio === modelData.estudio && Estudio.minutosDescanso === modelData.descanso
                            onPulsado: Estudio.ajustar(modelData.estudio, modelData.descanso)
                        }
                    }
                }
                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: window.t("option", "minutes of study / break")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: ThemeBackend.subtext0
                }
            }

            Item { Layout.fillHeight: true }

            // No molestar mientras estudias.
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: filaDnd.implicitHeight + window.s(20)
                radius: window.s(12)
                color: zonaDnd.containsMouse ? Qt.alpha(ThemeBackend.surface0, 0.8) : Qt.alpha(ThemeBackend.surface0, 0.5)
                RowLayout {
                    id: filaDnd
                    anchors.fill: parent
                    anchors.margins: window.s(10)
                    spacing: window.s(10)
                    Text {
                        text: "󰂛"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(18)
                        color: Estudio.noMolestar ? window.tinte : ThemeBackend.overlay1
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: window.t("dnd", "Silence notifications while you study")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.text
                    }
                    Rectangle {
                        implicitWidth: window.s(40)
                        implicitHeight: window.s(22)
                        radius: height / 2
                        color: Estudio.noMolestar ? window.tinte : Qt.alpha(ThemeBackend.surface1, 0.9)
                        Rectangle {
                            width: parent.height - window.s(6)
                            height: width
                            radius: width / 2
                            anchors.verticalCenter: parent.verticalCenter
                            x: Estudio.noMolestar ? parent.width - width - window.s(3) : window.s(3)
                            color: Estudio.noMolestar ? ThemeBackend.crust : ThemeBackend.text
                            Behavior on x { NumberAnimation { duration: 150 } }
                        }
                    }
                }
                MouseArea {
                    id: zonaDnd
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Estudio.noMolestar = !Estudio.noMolestar
                }
            }

            Text {
                Layout.alignment: Qt.AlignHCenter
                text: Estudio.sesionesHoy === 0 ? window.t("today_none", "No study blocks yet today")
                    : Estudio.sesionesHoy === 1 ? window.t("today_one", "Today: 1 study block")
                    : window.t("today", "Today: {n} study blocks").replace("{n}", Estudio.sesionesHoy)
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                color: ThemeBackend.subtext0
            }
        }
    }
}
