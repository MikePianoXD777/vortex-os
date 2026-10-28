import QtQuick
import QtQuick.Layouts
import Qt.labs.folderlistmodel
import Quickshell
import Quickshell.Io
import Quickshell.Hyprland
import "../"
import "../reusables"
import "../widgets/faces/html/Html.js" as Html

// Vortex 1.10: «Widgets en HTML» (lanzador). Tus widgets en HTML sencillo: a la izquierda la lista
// (~/.config/vortex/widgets-html/*.html), en medio el editor y abajo cómo se ve, al instante. «Guardar»
// (o Ctrl+S) y el widget del escritorio cambia solo; «Ponerlo en el escritorio» agrega un widget «HTML»
// con ese archivo en el monitor de ahora. Qué HTML sirve y los datos que se llenan solos: Html.js.
Item {
    id: window
    visible: false
    focus: true

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("htmlwidgets." + clave, porDefecto);
    }

    readonly property string carpeta: Quickshell.env("HOME") + "/.config/vortex/widgets-html"
    property string elegido: ""          // ruta del .html que se edita
    property bool sinGuardar: false
    property string mensaje: ""
    property date ahora: new Date()
    property string equipo: ""
    property bool ignorarCarga: false   // el archivo nuevo se escribe aparte: el editor ya tiene la plantilla

    function nombre(ruta) {
        let n = ruta.substring(ruta.lastIndexOf("/") + 1);
        return n.endsWith(".html") ? n.substring(0, n.length - 5) : n;
    }

    function escribir(ruta, texto) {
        Quickshell.execDetached(["sh", "-c", "mkdir -p \"$(dirname \"$1\")\" && printf '%s' \"$2\" > \"$1\"", "_", ruta, texto]);
    }

    function abrir(ruta) {
        if (window.sinGuardar && window.elegido !== "") guardar();
        window.ignorarCarga = false;
        window.elegido = ruta;
        window.sinGuardar = false;
        window.mensaje = "";
        archivo.reload();
    }

    function guardar() {
        if (window.elegido === "") return;
        escribir(window.elegido, editor.text);
        window.sinGuardar = false;
        window.mensaje = window.t("saved", "Saved: the widget on your desktop is already updated.");
    }

    function nuevo() {
        let usados = {};
        for (let i = 0; i < archivos.count; i++) usados[archivos.get(i, "fileName")] = true;
        let n = 1;
        while (usados["widget-" + n + ".html"]) n++;
        let ruta = window.carpeta + "/widget-" + n + ".html";
        escribir(ruta, Html.PLANTILLA);
        window.ignorarCarga = true;
        window.elegido = ruta;
        window.sinGuardar = false;
        editor.text = Html.PLANTILLA;
        window.mensaje = window.t("created", "New widget. Edit it and press «Put it on the desktop».");
    }

    function alEscritorio() {
        if (window.elegido === "") return;
        if (window.sinGuardar) guardar();
        let mon = Hyprland.focusedMonitor;
        let w = 320, h = 160;
        let x = mon ? Math.round(mon.width / 2 - w / 2) : 400;
        let y = mon ? Math.round(mon.height / 2 - h / 2) : 300;
        WidgetSync.addFromRegistry(mon ? mon.name : "", "html", { imagePath: window.elegido, x: x, y: y, w: w, h: h });
        window.mensaje = window.t("placed", "Done: it's in the middle of your screen. Move it or resize it from «Edit widgets».");
    }

    FolderListModel {
        id: archivos
        folder: "file://" + window.carpeta
        nameFilters: ["*.html", "*.htm"]
        showDirs: false
        sortField: FolderListModel.Name
        onStatusChanged: {
            if (status === FolderListModel.Ready && window.elegido === "" && count > 0)
                window.abrir(window.carpeta + "/" + get(0, "fileName"));
        }
    }

    FileView {
        id: archivo
        path: window.elegido
        printErrors: false
        onLoaded: {
            if (window.ignorarCarga) { window.ignorarCarga = false; return; }
            editor.text = text();
            window.sinGuardar = false;
        }
    }

    FileView {
        path: "/etc/hostname"
        printErrors: false
        onLoaded: window.equipo = text().trim()
    }

    Timer {
        interval: 1000
        repeat: true
        running: window.visible && Html.tieneDatos(editor.text)
        onTriggered: window.ahora = new Date()
    }

    property var exitCallback: null

    function startExit(onFinished) {
        if (window.sinGuardar) guardar();
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
            resetAndPlayIntro();
        } else {
            if (window.sinGuardar) guardar();
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: if (visible) resetAndPlayIntro()

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
        property string texto: ""
        property color fondo: ThemeBackend.surface1
        property color letra: ThemeBackend.text
        signal pulsado()
        implicitHeight: window.s(34)
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
            anchors.margins: window.s(22)
            spacing: window.s(12)

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)
                Rectangle {
                    Layout.preferredWidth: window.s(46)
                    Layout.preferredHeight: window.s(46)
                    radius: width / 2
                    color: Qt.alpha(ThemeBackend.peach, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: "󰌝"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(24)
                        color: ThemeBackend.peach
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "HTML widgets")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(21)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        text: window.t("intro", "Program your own desktop widgets in simple HTML. Save and it updates on its own.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(12)
                        color: ThemeBackend.subtext0
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)

                // La lista de tus widgets
                ColumnLayout {
                    Layout.fillWidth: false
                    Layout.preferredWidth: window.s(170)
                    Layout.maximumWidth: window.s(170)
                    Layout.fillHeight: true
                    spacing: window.s(8)

                    Boton {
                        Layout.fillWidth: true
                        texto: "󰐕  " + window.t("new", "New widget")
                        fondo: ThemeBackend.peach
                        letra: ThemeBackend.crust
                        onPulsado: window.nuevo()
                    }
                    ListView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: window.s(4)
                        model: archivos
                        delegate: Rectangle {
                            id: filaArchivo
                            required property string fileName
                            required property string filePath
                            readonly property bool activo: window.elegido === filePath
                            width: ListView.view.width
                            height: window.s(34)
                            radius: window.s(10)
                            color: activo ? Qt.alpha(ThemeBackend.peach, 0.2) : (zonaArchivo.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.5) : "transparent")
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                x: window.s(12)
                                width: parent.width - window.s(24)
                                elide: Text.ElideRight
                                text: (filaArchivo.activo && window.sinGuardar ? "● " : "") + window.nombre(filaArchivo.fileName)
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(12)
                                font.weight: filaArchivo.activo ? Font.Bold : Font.Normal
                                color: ThemeBackend.text
                            }
                            MouseArea {
                                id: zonaArchivo
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: window.abrir(filaArchivo.filePath)
                            }
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        visible: archivos.count === 0
                        wrapMode: Text.WordWrap
                        text: window.t("none", "You don't have any yet: press «New widget».")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(11)
                        color: ThemeBackend.subtext0
                    }
                }

                // Editor y vista previa
                ColumnLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    spacing: window.s(10)
                    enabled: window.elegido !== ""
                    opacity: enabled ? 1 : 0.5

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredHeight: 3
                        radius: window.s(12)
                        color: Qt.alpha(ThemeBackend.crust, 0.6)
                        border.width: editor.activeFocus ? 1 : 0
                        border.color: ThemeBackend.peach

                        Flickable {
                            id: desliza
                            anchors.fill: parent
                            anchors.margins: window.s(12)
                            clip: true
                            contentWidth: width
                            contentHeight: editor.implicitHeight
                            boundsBehavior: Flickable.StopAtBounds

                            function asegurar(r) {
                                if (contentY >= r.y) contentY = r.y;
                                else if (contentY + height <= r.y + r.height) contentY = r.y + r.height - height;
                            }

                            TextEdit {
                                id: editor
                                width: desliza.width
                                wrapMode: TextEdit.Wrap
                                selectByMouse: true
                                persistentSelection: true
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(13)
                                color: ThemeBackend.text
                                selectionColor: Qt.alpha(ThemeBackend.peach, 0.4)
                                onCursorRectangleChanged: desliza.asegurar(cursorRectangle)
                                onTextChanged: if (activeFocus) window.sinGuardar = true
                                Keys.onPressed: function(evento) {
                                    if ((evento.modifiers & Qt.ControlModifier) && evento.key === Qt.Key_S) {
                                        window.guardar();
                                        evento.accepted = true;
                                    } else if (evento.key === Qt.Key_Tab) {
                                        editor.insert(editor.cursorPosition, "  ");
                                        evento.accepted = true;
                                    }
                                }
                            }
                        }
                    }

                    Text {
                        text: window.t("preview", "How it looks")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(11)
                        font.weight: Font.Bold
                        color: ThemeBackend.subtext0
                    }
                    Rectangle {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.preferredHeight: 2
                        radius: ThemeBackend.borderRadius
                        color: Qt.alpha(ThemeBackend.base, 0.55)
                        border.width: 1
                        border.color: Qt.alpha(ThemeBackend.surface1, 0.4)
                        clip: true
                        Text {
                            anchors.fill: parent
                            anchors.margins: 14
                            textFormat: Text.RichText
                            wrapMode: Text.Wrap
                            color: ThemeBackend.text
                            linkColor: ThemeBackend.blue
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: 14
                            text: Html.llenar(editor.text, window.ahora, Quickshell.env("USER"), window.equipo)
                        }
                    }
                }

                // Chuleta
                Rectangle {
                    Layout.preferredWidth: window.s(210)
                    Layout.fillHeight: true
                    radius: window.s(12)
                    color: Qt.alpha(ThemeBackend.surface0, 0.4)
                    Flickable {
                        anchors.fill: parent
                        anchors.margins: window.s(12)
                        clip: true
                        contentHeight: chuleta.implicitHeight
                        boundsBehavior: Flickable.StopAtBounds
                        Text {
                            id: chuleta
                            width: parent.width
                            wrapMode: Text.WordWrap
                            textFormat: Text.StyledText
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(11)
                            color: ThemeBackend.subtext1
                            text: window.t("cheatsheet", "<b>What works</b><br>&lt;h1&gt;…&lt;h6&gt; titles<br>&lt;p&gt; &lt;br&gt; &lt;hr&gt;<br>&lt;b&gt; &lt;i&gt; &lt;u&gt; &lt;s&gt;<br>&lt;font color=\"#f38ba8\" size=\"5\"&gt;<br>&lt;span style=\"color: red; font-size: 20px\"&gt;<br>&lt;div align=\"center\"&gt;<br>&lt;ul&gt; &lt;ol&gt; &lt;li&gt;<br>&lt;table&gt; &lt;tr&gt; &lt;td&gt;<br>&lt;img src=\"/path/image.png\" width=\"80\"&gt;<br>&lt;a href=\"https://…\"&gt; link<br><br><b>Filled in by itself</b><br>{hora} {hora12} {segundos}<br>{fecha} {fechaLarga}<br>{dia} {mes} {año}<br>{usuario} {equipo}<br><br>No JavaScript: it uses almost no memory.")
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(10)
                Text {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: window.mensaje !== "" ? window.mensaje
                        : window.elegido !== "" ? window.t("file", "File: {f}").replace("{f}", window.elegido.replace(Quickshell.env("HOME"), "~"))
                        : ""
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(11)
                    color: window.mensaje !== "" ? ThemeBackend.green : ThemeBackend.overlay1
                }
                Boton {
                    texto: "󰆓  " + window.t("save", "Save")
                    enabled: window.elegido !== ""
                    fondo: window.sinGuardar ? ThemeBackend.peach : ThemeBackend.surface1
                    letra: window.sinGuardar ? ThemeBackend.crust : ThemeBackend.text
                    onPulsado: window.guardar()
                }
                Boton {
                    texto: "󰍹  " + window.t("place", "Put it on the desktop")
                    enabled: window.elegido !== ""
                    onPulsado: window.alEscritorio()
                }
            }
        }
    }
}
