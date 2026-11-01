import QtQuick
import QtQuick.Layouts
import Quickshell
import "../"
import "../reusables"

// Vortex 2.5: panel «Tareas» (clic en la cuenta de la barra, o lanzador → «Tareas»). Solo muestra y
// maneja el singleton Tareas: las tareas viven en tu Obsidian (Tareas.md y las notas con 📅 fecha), así
// que también las ves y editas ahí. Clic en el círculo la marca hecha; Enter agrega la nueva.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return Tareas.t(clave, porDefecto);
    }

    // Para cuándo es la nueva: días desde hoy (-1 = sin fecha)
    property int dentroDe: 0
    readonly property var opciones: [
        { dias: 0, texto: t("today", "Today") }, { dias: 1, texto: t("tomorrow", "Tomorrow") },
        { dias: 2, texto: t("day_after", "Day after") }, { dias: 7, texto: t("next_week", "In a week") },
        { dias: -1, texto: t("no_date", "No date") }
    ]

    function agregar() {
        if (nueva.text.trim() === "") return;
        Tareas.agregar(nueva.text, dentroDe < 0 ? "" : Tareas.fechaEn(dentroDe));
        nueva.text = "";
    }

    function colorDe(tarea) {
        if (tarea.dias === null) return ThemeBackend.overlay1;
        if (tarea.dias < 0) return ThemeBackend.red;
        if (tarea.dias === 0) return ThemeBackend.mauve;
        if (tarea.dias === 1) return ThemeBackend.peach;
        return ThemeBackend.subtext0;
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
            Tareas.refrescar();
            nueva.forceActiveFocus();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        if (visible) {
            Tareas.refrescar();
            nueva.forceActiveFocus();
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

    component Boton: Rectangle {
        id: boton
        property string icono: ""
        property string texto: ""
        property color tinte: ThemeBackend.mauve
        property bool principal: false
        property bool elegido: false
        signal pulsado()
        implicitHeight: window.s(32)
        implicitWidth: Math.max(implicitHeight, fila.implicitWidth + window.s(24))
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
            spacing: window.s(14)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(12)
                Text {
                    text: "󰄲"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(30)
                    color: ThemeBackend.mauve
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Tasks")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(18)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: window.t("subtitle", "The ones for today and tomorrow show up in the bar, and I remind you a day before.")
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

            // La nueva tarea y para cuándo es.
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: window.s(46)
                radius: window.s(12)
                color: Qt.alpha(ThemeBackend.surface0, 0.7)
                border.width: nueva.activeFocus ? window.s(2) : 0
                border.color: ThemeBackend.mauve

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: window.s(14)
                    anchors.rightMargin: window.s(6)
                    spacing: window.s(8)
                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        TextInput {
                            id: nueva
                            objectName: "nueva"
                            anchors.fill: parent
                            verticalAlignment: TextInput.AlignVCenter
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(14)
                            color: ThemeBackend.text
                            selectionColor: Qt.alpha(ThemeBackend.mauve, 0.5)
                            clip: true
                            maximumLength: 200
                            onAccepted: window.agregar()
                            Keys.onEscapePressed: event => { event.accepted = false; }
                        }
                        Text {
                            anchors.fill: parent
                            verticalAlignment: Text.AlignVCenter
                            visible: nueva.text === ""
                            text: window.t("new", "New task… (Enter to add)")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(14)
                            color: ThemeBackend.overlay1
                        }
                    }
                    Boton {
                        principal: true
                        icono: "󰐕"
                        opacity: nueva.text.trim() === "" ? 0.5 : 1
                        onPulsado: window.agregar()
                    }
                }
            }

            Flow {
                Layout.fillWidth: true
                spacing: window.s(6)
                Repeater {
                    model: window.opciones
                    Boton {
                        required property var modelData
                        texto: modelData.texto
                        elegido: window.dentroDe === modelData.dias
                        onPulsado: { window.dentroDe = modelData.dias; nueva.forceActiveFocus(); }
                    }
                }
            }

            // La lista.
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true

                ListView {
                    id: lista
                    objectName: "lista"
                    anchors.fill: parent
                    clip: true
                    spacing: window.s(6)
                    boundsBehavior: Flickable.StopAtBounds
                    model: Tareas.tareas

                    delegate: Rectangle {
                        id: fila
                        required property var modelData
                        width: lista.width
                        implicitHeight: contenido.implicitHeight + window.s(18)
                        radius: window.s(12)
                        color: zonaFila.containsMouse ? Qt.alpha(ThemeBackend.surface0, 0.85) : Qt.alpha(ThemeBackend.surface0, 0.5)

                        MouseArea {
                            id: zonaFila
                            anchors.fill: parent
                            hoverEnabled: true
                        }

                        RowLayout {
                            id: contenido
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.leftMargin: window.s(12)
                            anchors.rightMargin: window.s(14)
                            spacing: window.s(12)

                            // El círculo: clic y queda hecha (✅ en la nota)
                            Rectangle {
                                implicitWidth: window.s(22)
                                implicitHeight: window.s(22)
                                radius: width / 2
                                color: zonaHecha.containsMouse ? Qt.alpha(ThemeBackend.green, 0.3) : "transparent"
                                border.width: window.s(2)
                                border.color: zonaHecha.containsMouse ? ThemeBackend.green : window.colorDe(fila.modelData)
                                Text {
                                    anchors.centerIn: parent
                                    visible: zonaHecha.containsMouse
                                    text: "󰄬"
                                    font.family: ThemeBackend.iconFont
                                    font.pixelSize: window.s(13)
                                    color: ThemeBackend.green
                                }
                                MouseArea {
                                    id: zonaHecha
                                    objectName: "hecha"
                                    anchors.fill: parent
                                    anchors.margins: -window.s(6)
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        Sounds.playSfx("notifications/Progress.wav", 0.6);
                                        Tareas.hecha(fila.modelData);
                                    }
                                }
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: window.s(2)
                                Text {
                                    Layout.fillWidth: true
                                    wrapMode: Text.WordWrap
                                    text: fila.modelData.texto
                                    textFormat: Text.PlainText
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(14)
                                    color: ThemeBackend.text
                                }
                                Text {
                                    Layout.fillWidth: true
                                    visible: fila.modelData.archivo !== Tareas.archivo
                                    elide: Text.ElideRight
                                    text: "󰈙 " + fila.modelData.nota
                                    textFormat: Text.PlainText
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(11)
                                    color: ThemeBackend.subtext0
                                }
                            }

                            Text {
                                visible: text !== ""
                                text: Tareas.cuando(fila.modelData)
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(12)
                                font.weight: Font.DemiBold
                                color: window.colorDe(fila.modelData)
                            }
                        }
                    }
                }

                // Sin tareas
                ColumnLayout {
                    anchors.centerIn: parent
                    width: parent.width - window.s(40)
                    visible: Tareas.cargado && Tareas.tareas.length === 0
                    spacing: window.s(6)
                    Text {
                        Layout.alignment: Qt.AlignHCenter
                        text: "󰄲"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(42)
                        color: Qt.alpha(ThemeBackend.mauve, 0.6)
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        text: window.t("empty", "Nothing pending. Write one above, or put 📅 2026-11-03 on any task in your notes.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }
                }
            }

            // Dónde se guardan, y abrirlas en Obsidian.
            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(10)
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: Tareas.boveda !== "" ? window.t("saved_obsidian", "Saved in your Obsidian (Tareas.md)")
                                               : window.t("saved_file", "Saved in Documents/Tareas.md")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: ThemeBackend.subtext0
                }
                Boton {
                    visible: Tareas.archivo !== ""
                    icono: "󰏌"
                    texto: window.t("open", "Open")
                    onPulsado: Quickshell.execDetached(["vortex-tareas", "abrir"])
                }
            }
        }
    }
}
