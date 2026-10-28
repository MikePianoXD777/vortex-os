import QtQuick
import QtQuick.Layouts
import QtCore
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.10: Nota rápida (Super+Alt+N o el lanzador). Una nota para apuntar algo al vuelo: se abre
// con el cursor al final, se guarda sola mientras escribes y la vuelves a abrir igual. Vive en
// Documentos/Nota rápida.md, así que también se encuentra en el explorador y se edita con cualquier
// editor (al abrir el panel se vuelve a leer).
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("notas." + clave, porDefecto);
    }

    readonly property string carpeta: decodeURIComponent(String(StandardPaths.writableLocation(StandardPaths.DocumentsLocation)).replace(/^file:\/\//, ""))
    readonly property string ruta: carpeta + "/" + t("file", "Quick note") + ".md"

    property bool cargando: false
    property bool sinGuardar: false
    property real guardadoEn: 0

    function guardar() {
        guardarTimer.stop();
        if (!sinGuardar) return;
        archivo.setText(editor.text);
        sinGuardar = false;
        guardadoEn = Date.now();
    }

    FileView {
        id: archivo
        path: window.ruta
        printErrors: false
        blockLoading: true
        onLoaded: {
            let nuevo = text();
            if (nuevo !== editor.text && !window.sinGuardar) {
                window.cargando = true;
                editor.text = nuevo;
                window.cargando = false;
            }
            editor.cursorPosition = editor.length;
        }
    }

    // Guarda medio segundo después de dejar de escribir (y al cerrar)
    Timer {
        id: guardarTimer
        interval: 500
        onTriggered: window.guardar()
    }

    // --- abrir y cerrar ---------------------------------------------------------------------------

    property var exitCallback: null

    function startExit(onFinished) {
        guardar();
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
            if (!sinGuardar) archivo.reload();
            editor.forceActiveFocus();
            editor.cursorPosition = editor.length;
            resetAndPlayIntro();
        } else {
            guardar();
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        if (visible) {
            editor.forceActiveFocus();
            resetAndPlayIntro();
        }
    }

    Component.onDestruction: guardar()

    property real introMain: 0

    NumberAnimation {
        id: introAnim
        target: window; property: "introMain"; from: 0; to: 1.0; duration: 600; easing.type: Easing.OutExpo
    }

    SequentialAnimation {
        id: exitAnim
        NumberAnimation { target: window; property: "introMain"; to: 0.0; duration: 160; easing.type: Easing.InQuart }
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
        property string texto: ""
        signal pulsado()
        implicitHeight: window.s(30)
        implicitWidth: textoBoton.implicitWidth + window.s(24)
        radius: height / 2
        color: zonaBoton.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.95) : Qt.alpha(ThemeBackend.surface0, 0.75)
        Text {
            id: textoBoton
            anchors.centerIn: parent
            text: boton.texto
            font.family: ThemeBackend.fontFamily
            font.pixelSize: window.s(12)
            font.weight: Font.Bold
            color: ThemeBackend.text
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
            anchors.margins: window.s(20)
            spacing: window.s(12)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(10)

                Text {
                    text: "󰎞"
                    font.family: ThemeBackend.iconFont
                    font.pixelSize: window.s(22)
                    color: ThemeBackend.yellow
                }
                Text {
                    Layout.fillWidth: true
                    text: window.t("title", "Quick note")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(18)
                    font.weight: Font.Bold
                    color: ThemeBackend.text
                }
                Text {
                    text: window.sinGuardar ? window.t("saving", "Saving…") : window.guardadoEn > 0 ? window.t("saved", "Saved") : ""
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: ThemeBackend.subtext0
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.fillHeight: true
                radius: ThemeBackend.borderRadius
                color: Qt.alpha(ThemeBackend.surface0, 0.5)

                Flickable {
                    id: desliza
                    anchors.fill: parent
                    anchors.margins: window.s(14)
                    clip: true
                    contentWidth: width
                    contentHeight: Math.max(editor.implicitHeight, height)
                    boundsBehavior: Flickable.StopAtBounds

                    function asegurar(r) {
                        if (contentY >= r.y) contentY = r.y;
                        else if (contentY + height <= r.y + r.height) contentY = r.y + r.height - height;
                    }

                    TextEdit {
                        id: editor
                        width: desliza.width
                        height: Math.max(implicitHeight, desliza.height)
                        wrapMode: TextEdit.Wrap
                        selectByMouse: true
                        persistentSelection: true
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(14)
                        color: ThemeBackend.text
                        selectionColor: Qt.alpha(ThemeBackend.yellow, 0.35)
                        onCursorRectangleChanged: desliza.asegurar(cursorRectangle)
                        onTextChanged: {
                            if (window.cargando) return;
                            window.sinGuardar = true;
                            guardarTimer.restart();
                        }

                        Text {
                            visible: editor.text === ""
                            text: window.t("placeholder", "Write whatever: it saves itself.")
                            font: editor.font
                            color: ThemeBackend.overlay1
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(8)

                Text {
                    Layout.fillWidth: true
                    elide: Text.ElideMiddle
                    text: window.t("where", "Saved in Documents") + " · " + window.ruta.replace(Quickshell.env("HOME"), "~")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(10)
                    color: ThemeBackend.overlay1
                }
                Boton {
                    texto: window.t("copy", "Copy all")
                    onPulsado: Quickshell.execDetached(["wl-copy", "--", editor.text])
                }
                Boton {
                    texto: window.t("open", "Open in editor")
                    onPulsado: {
                        window.guardar();
                        Quickshell.execDetached(["xdg-open", window.ruta]);
                    }
                }
            }
        }
    }
}
