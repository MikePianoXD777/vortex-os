// Vortexine's OS 4.0: la pantalla de inicio (login gráfico). La arranca vortex-greeter dentro de un Hyprland
// mínimo, como el usuario «greeter» de greetd. Diseño de la 4.0: negro de verdad, blanco y grises, todo
// sólido. En cada monitor va el reloj; el login, en el principal (un clic en otro monitor lo pasa ahí).
//
// El flujo con greetd: createSession(usuario) → greetd pide la contraseña (authMessage) → respond() →
// readyToLaunch → launch(["…/entrar", sesión]) y Quickshell se cierra; greetd arranca el escritorio.
// 4.0: si hay otros escritorios instalados (vortex escritorio instalar kde…), abajo a la izquierda se elige
// cuál; se recuerda el último de cada persona en /var/cache/vortex-greeter/sesion-<usuario>.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import Quickshell.Services.Greetd

ShellRoot {
    id: raiz

    readonly property string dir: "/usr/share/vortex/greeter"
    readonly property string estadoDir: "/var/cache/vortex-greeter"

    // Colores de la 4.0 (los mismos de fábrica del shell).
    readonly property color negro: "#000000"
    readonly property color tarjeta: "#111111"
    readonly property color borde: "#2A2A2A"
    readonly property color blanco: "#FFFFFF"
    readonly property color gris: "#B3B3B3"
    readonly property color grisOscuro: "#6E6E6E"

    property var usuarios: []          // [{ nombre, real }]
    property int elegido: 0
    readonly property var usuario: usuarios.length ? usuarios[Math.min(elegido, usuarios.length - 1)] : null
    property string ultimoNombre: ""

    // Los escritorios: Vortex Shell y los que haya instalados (vortex-escritorio sesiones).
    property var sesiones: [{ id: "vortex", tipo: "wayland", escritorios: "", nombre: "Vortex Shell" }]
    property string sesionId: "vortex"
    readonly property var sesion: {
        for (const x of sesiones) if (x.id === sesionId) return x;
        return sesiones[0];
    }
    property bool eligiendoSesion: false

    // listo | revisando | pregunta (PAM pide algo más) | entrando
    property string estado: "listo"
    property string mensaje: ""
    property bool esError: false
    property string pendiente: ""
    property bool respondido: false
    property real falloEn: 0
    property string principal: Quickshell.screens.length ? Quickshell.screens[0].name : ""
    property date ahora: new Date()
    property bool visible_: false

    signal sacudir()

    Timer { interval: 1000; running: true; repeat: true; onTriggered: raiz.ahora = new Date() }
    Timer { interval: 60; running: true; onTriggered: raiz.visible_ = true }

    FontLoader { id: unbounded; source: "file:///usr/share/serpantinum/src/assets/fonts/Unbounded-SemiBold.ttf" }
    readonly property string letraTitulos: unbounded.status === FontLoader.Ready ? unbounded.name : "Sans"

    // --- Usuarios: los de /etc/passwd con uid de persona y una shell de verdad. ---
    FileView {
        id: passwd
        path: "/etc/passwd"
        onLoaded: raiz.leerUsuarios(text())
    }
    FileView {
        id: ultimo
        path: raiz.estadoDir + "/ultimo"
        printErrors: false
        onLoaded: { raiz.ultimoNombre = text().trim(); raiz.elegirUltimo(); }
    }

    Process {
        id: listaSesiones
        command: ["vortex-escritorio", "sesiones"]
        running: true
        stdout: StdioCollector {
            onStreamFinished: {
                let l = [raiz.sesiones[0]];
                for (const linea of text.split("\n")) {
                    const c = linea.split("\t");
                    if (c.length < 4 || c[0] === "" || c[0] === "vortex") continue;
                    l.push({ id: c[0], tipo: c[1], escritorios: c[2], nombre: c[3] });
                }
                raiz.sesiones = l;
                raiz.leerSesion();
            }
        }
    }
    // El último escritorio de la persona elegida (cambia de archivo al cambiar de persona).
    FileView {
        id: sesionGuardada
        path: raiz.usuario ? raiz.estadoDir + "/sesion-" + raiz.usuario.nombre : ""
        printErrors: false
        onLoaded: raiz.leerSesion()
        onLoadFailed: raiz.sesionId = "vortex"
    }
    function leerSesion() {
        let id = "vortex";
        try { id = sesionGuardada.text().trim() || "vortex"; } catch (e) {}
        sesionId = sesiones.some(x => x.id === id) ? id : "vortex";
    }
    function elegirSesion(id) {
        sesionId = id;
        eligiendoSesion = false;
    }
    // Lo que pone un gestor de inicio de sesión para ese escritorio (Vortex Shell no necesita nada).
    function variablesSesion() {
        if (sesion.id === "vortex") return [];
        let v = ["XDG_SESSION_TYPE=" + sesion.tipo, "XDG_SESSION_DESKTOP=" + sesion.id];
        if (sesion.escritorios !== "") v.push("XDG_CURRENT_DESKTOP=" + sesion.escritorios);
        return v;
    }

    function leerUsuarios(t) {
        let lista = [];
        for (const linea of t.split("\n")) {
            const c = linea.split(":");
            if (c.length < 7) continue;
            const uid = parseInt(c[2]);
            const sh = c[6].trim();
            if (!(uid >= 1000 && uid < 60000)) continue;
            if (sh === "" || sh.endsWith("nologin") || sh.endsWith("/false")) continue;
            const real = (c[4] || "").split(",")[0].trim();
            lista.push({ nombre: c[0], real: real !== "" ? real : c[0] });
        }
        usuarios = lista;
        elegirUltimo();
    }
    function elegirUltimo() {
        for (let i = 0; i < usuarios.length; i++) {
            if (usuarios[i].nombre === ultimoNombre) { elegido = i; return; }
        }
    }
    function cambiarUsuario(d) {
        if (usuarios.length < 2 || estado === "revisando" || estado === "entrando") return;
        elegido = (elegido + d + usuarios.length) % usuarios.length;
        limpiar();
    }
    function limpiar() {
        if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession();
        estado = "listo"; mensaje = ""; esError = false; pendiente = ""; respondido = false;
    }

    function entrar(texto) {
        if (estado === "entrando" || estado === "revisando") return;
        if (estado === "pregunta") {
            estado = "revisando";
            Greetd.respond(texto);
            return;
        }
        if (!usuario) return;
        if (texto === "") { sacudir(); return; }
        if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession();
        mensaje = ""; esError = false;
        pendiente = texto;
        respondido = false;
        estado = "revisando";
        Greetd.createSession(usuario.nombre);
    }

    function fallar(texto) {
        estado = "listo";
        mensaje = texto;
        esError = true;
        pendiente = "";
        respondido = false;
        sacudir();
    }

    Connections {
        target: Greetd
        function onAuthMessage(message, error, responseRequired, echoResponse) {
            if (responseRequired) {
                if (!raiz.respondido) {
                    raiz.respondido = true;
                    Greetd.respond(raiz.pendiente);
                    raiz.pendiente = "";
                } else {
                    // PAM pide algo más (un código, cambiar la contraseña…): se le pregunta a la persona.
                    raiz.estado = "pregunta";
                    raiz.mensaje = message;
                    raiz.esError = false;
                }
            } else if (message !== "") {
                raiz.mensaje = message;
                raiz.esError = error;
            }
        }
        function onAuthFailure(message) {
            raiz.falloEn = Date.now();
            raiz.fallar("Contraseña incorrecta");
        }
        function onError(error) {
            // Tras una contraseña mala, greetd corta la conexión y llega un «Connection refused»: ya se
            // dijo «Contraseña incorrecta», no se tapa con el error técnico.
            if (Date.now() - raiz.falloEn < 5000) return;
            raiz.fallar("No se pudo iniciar sesión. Inténtalo otra vez.");
            console.warn("greetd:", error);
            if (Greetd.state !== GreetdState.Inactive) Greetd.cancelSession();
        }
        function onReadyToLaunch() {
            raiz.estado = "entrando";
            raiz.mensaje = "";
            if (raiz.usuario) ultimo.setText(raiz.usuario.nombre + "\n");
            if (raiz.usuario && raiz.sesiones.length > 1) sesionGuardada.setText(raiz.sesion.id + "\n");
            salida.start();
        }
    }

    // Un fundido a negro antes de pasar al escritorio.
    Timer {
        id: salida
        interval: 420
        onTriggered: Greetd.launch([raiz.dir + "/entrar", raiz.sesion.id], raiz.variablesSesion(), true)
    }

    Process { id: energia }
    function apagar(accion) {
        if (estado === "entrando") return;
        energia.command = ["systemctl", accion];
        energia.running = true;
    }

    Variants {
        model: Quickshell.screens

        PanelWindow {
            id: ventana
            required property var modelData
            readonly property bool esPrincipal: modelData.name === raiz.principal
            screen: modelData
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            color: raiz.negro
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "vortex-greeter"
            WlrLayershell.keyboardFocus: esPrincipal ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

            readonly property real u: Math.max(0.6, Math.min(width / 1920, height / 1080))

            MouseArea {
                anchors.fill: parent
                enabled: !ventana.esPrincipal
                onClicked: raiz.principal = ventana.modelData.name
            }

            Item {
                id: contenido
                anchors.fill: parent
                opacity: raiz.visible_ && raiz.estado !== "entrando" ? 1 : 0
                Behavior on opacity { NumberAnimation { duration: raiz.estado === "entrando" ? 380 : 700; easing.type: Easing.OutCubic } }

                Column {
                    id: centro
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: ventana.esPrincipal ? -40 * ventana.u : 0
                    spacing: 0
                    transform: Translate { y: contenido.opacity < 1 ? (1 - contenido.opacity) * 18 : 0 }

                    Image {
                        anchors.horizontalCenter: parent.horizontalCenter
                        source: "file://" + raiz.dir + "/logo.svg"
                        sourceSize.width: Math.round(44 * ventana.u)
                        sourceSize.height: Math.round(44 * ventana.u)
                        smooth: true
                    }
                    Item { width: 1; height: 26 * ventana.u }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Qt.formatTime(raiz.ahora, "HH:mm")
                        color: raiz.blanco
                        font.family: raiz.letraTitulos
                        font.weight: Font.DemiBold
                        font.pixelSize: Math.round(112 * ventana.u)
                        font.letterSpacing: -2 * ventana.u
                    }
                    Item { width: 1; height: 4 * ventana.u }
                    Text {
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: Qt.locale().toString(raiz.ahora, "dddd, d 'de' MMMM")
                        color: raiz.gris
                        font.pixelSize: Math.round(20 * ventana.u)
                    }

                    // --- El login (solo en el monitor principal) ---
                    Item { width: 1; height: ventana.esPrincipal ? 64 * ventana.u : 0 }
                    Column {
                        id: login
                        visible: ventana.esPrincipal
                        anchors.horizontalCenter: parent.horizontalCenter
                        spacing: 14 * ventana.u

                        Row {
                            anchors.horizontalCenter: parent.horizontalCenter
                            spacing: 22 * ventana.u
                            Flecha { texto: "‹"; visible: raiz.usuarios.length > 1; onPulsado: raiz.cambiarUsuario(-1); u: ventana.u }
                            Rectangle {
                                width: 76 * ventana.u; height: width; radius: width / 2
                                color: raiz.blanco
                                Text {
                                    anchors.centerIn: parent
                                    text: raiz.usuario ? raiz.usuario.real.charAt(0).toUpperCase() : ""
                                    color: raiz.negro
                                    font.family: raiz.letraTitulos
                                    font.weight: Font.DemiBold
                                    font.pixelSize: Math.round(32 * ventana.u)
                                }
                            }
                            Flecha { texto: "›"; visible: raiz.usuarios.length > 1; onPulsado: raiz.cambiarUsuario(1); u: ventana.u }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: raiz.usuario ? raiz.usuario.real : "Sin usuarios"
                            color: raiz.blanco
                            font.pixelSize: Math.round(19 * ventana.u)
                            font.weight: Font.Medium
                        }

                        Rectangle {
                            id: campoCaja
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 340 * ventana.u
                            height: 52 * ventana.u
                            radius: height / 2
                            color: raiz.tarjeta
                            border.width: 1
                            border.color: campo.activeFocus ? "#5A5A5A" : raiz.borde
                            Behavior on border.color { ColorAnimation { duration: 150 } }
                            property real sacudida: 0
                            transform: Translate { x: campoCaja.sacudida }

                            SequentialAnimation {
                                id: animSacudir
                                NumberAnimation { target: campoCaja; property: "sacudida"; to: -10; duration: 50 }
                                NumberAnimation { target: campoCaja; property: "sacudida"; to: 9; duration: 70 }
                                NumberAnimation { target: campoCaja; property: "sacudida"; to: -6; duration: 60 }
                                NumberAnimation { target: campoCaja; property: "sacudida"; to: 3; duration: 50 }
                                NumberAnimation { target: campoCaja; property: "sacudida"; to: 0; duration: 40 }
                            }
                            Connections {
                                target: raiz
                                function onSacudir() { if (ventana.esPrincipal) { animSacudir.restart(); campo.text = ""; } }
                            }

                            TextInput {
                                id: campo
                                anchors.left: parent.left
                                anchors.right: boton.left
                                anchors.leftMargin: 22 * ventana.u
                                anchors.rightMargin: 10 * ventana.u
                                anchors.verticalCenter: parent.verticalCenter
                                color: raiz.blanco
                                font.pixelSize: Math.round(17 * ventana.u)
                                echoMode: raiz.estado === "pregunta" ? TextInput.Normal : TextInput.Password
                                passwordCharacter: "•"
                                focus: ventana.esPrincipal
                                enabled: raiz.estado === "listo" || raiz.estado === "pregunta"
                                clip: true
                                selectionColor: "#444444"
                                onAccepted: { const t = text; text = ""; raiz.entrar(t); }
                                onTextChanged: if (raiz.esError && text !== "") { raiz.mensaje = ""; raiz.esError = false; }
                                Keys.onPressed: (ev) => {
                                    if (campo.text === "" && (ev.key === Qt.Key_Left || ev.key === Qt.Key_Right)) {
                                        raiz.cambiarUsuario(ev.key === Qt.Key_Left ? -1 : 1);
                                        ev.accepted = true;
                                    } else if (ev.key === Qt.Key_Escape) {
                                        if (raiz.eligiendoSesion) { raiz.eligiendoSesion = false; ev.accepted = true; return; }
                                        campo.text = "";
                                        raiz.limpiar();
                                        ev.accepted = true;
                                    }
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: campo.text === ""
                                    text: raiz.estado === "pregunta" ? "Escribe la respuesta" : "Contraseña"
                                    color: raiz.grisOscuro
                                    font.pixelSize: campo.font.pixelSize
                                }
                            }
                            // Se recupera el foco si un clic lo quita (en el login no hay nada más que escribir).
                            Connections {
                                target: ventana
                                function onEsPrincipalChanged() { if (ventana.esPrincipal) campo.forceActiveFocus(); }
                            }

                            Rectangle {
                                id: boton
                                anchors.right: parent.right
                                anchors.rightMargin: 6 * ventana.u
                                anchors.verticalCenter: parent.verticalCenter
                                width: 40 * ventana.u; height: width; radius: width / 2
                                color: campo.text !== "" || raiz.estado === "revisando" ? raiz.blanco : "#1E1E1E"
                                Behavior on color { ColorAnimation { duration: 150 } }
                                Text {
                                    anchors.centerIn: parent
                                    visible: raiz.estado !== "revisando"
                                    text: "→"
                                    color: campo.text !== "" ? raiz.negro : raiz.grisOscuro
                                    font.pixelSize: Math.round(18 * ventana.u)
                                    font.weight: Font.DemiBold
                                }
                                // Tres puntos mientras greetd revisa la contraseña.
                                Row {
                                    anchors.centerIn: parent
                                    visible: raiz.estado === "revisando"
                                    spacing: 3 * ventana.u
                                    Repeater {
                                        model: 3
                                        Rectangle {
                                            required property int index
                                            width: 5 * ventana.u; height: width; radius: width / 2
                                            color: raiz.negro
                                            SequentialAnimation on opacity {
                                                running: raiz.estado === "revisando"
                                                loops: Animation.Infinite
                                                PauseAnimation { duration: index * 140 }
                                                NumberAnimation { from: 0.25; to: 1; duration: 280 }
                                                NumberAnimation { from: 1; to: 0.25; duration: 280 }
                                                PauseAnimation { duration: (2 - index) * 140 }
                                            }
                                        }
                                    }
                                }
                                MouseArea {
                                    anchors.fill: parent
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: { const t = campo.text; campo.text = ""; raiz.entrar(t); }
                                }
                            }
                        }

                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 380 * ventana.u
                            horizontalAlignment: Text.AlignHCenter
                            wrapMode: Text.WordWrap
                            text: raiz.mensaje
                            color: raiz.esError ? raiz.blanco : raiz.gris
                            opacity: raiz.mensaje !== "" ? 1 : 0
                            Behavior on opacity { NumberAnimation { duration: 180 } }
                            font.pixelSize: Math.round(14 * ventana.u)
                        }
                    }
                }

                // Apagar, reiniciar y suspender (abajo a la derecha del principal).
                Row {
                    visible: ventana.esPrincipal
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.margins: 32 * ventana.u
                    spacing: 8 * ventana.u
                    Boton { texto: "Suspender"; u: ventana.u; onPulsado: raiz.apagar("suspend") }
                    Boton { texto: "Reiniciar"; u: ventana.u; onPulsado: raiz.apagar("reboot") }
                    Boton { texto: "Apagar"; u: ventana.u; onPulsado: raiz.apagar("poweroff") }
                }
                // El escritorio (solo si hay más de uno): un botón que abre la lista hacia arriba.
                Column {
                    id: selector
                    visible: ventana.esPrincipal && raiz.sesiones.length > 1
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    anchors.margins: 32 * ventana.u
                    spacing: 8 * ventana.u

                    Rectangle {
                        id: lista
                        width: Math.max(220 * ventana.u, opciones.implicitWidth + 12 * ventana.u)
                        height: opciones.implicitHeight + 12 * ventana.u
                        radius: 18 * ventana.u
                        color: raiz.tarjeta
                        border.width: 1
                        border.color: raiz.borde
                        visible: opacity > 0
                        opacity: raiz.eligiendoSesion ? 1 : 0
                        Behavior on opacity { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
                        transform: Translate { y: raiz.eligiendoSesion ? 0 : 8 * ventana.u; Behavior on y { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } } }
                        Column {
                            id: opciones
                            anchors.centerIn: parent
                            Repeater {
                                model: raiz.sesiones
                                Rectangle {
                                    id: opcion
                                    required property var modelData
                                    readonly property bool actual: modelData.id === raiz.sesionId
                                    width: lista.width - 12 * ventana.u
                                    implicitWidth: nombreOpcion.implicitWidth + 64 * ventana.u
                                    height: 40 * ventana.u
                                    radius: height / 2
                                    color: zonaOpcion.containsMouse ? "#1E1E1E" : "transparent"
                                    Text {
                                        id: nombreOpcion
                                        anchors.left: parent.left
                                        anchors.leftMargin: 18 * ventana.u
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: opcion.modelData.nombre
                                        color: opcion.actual || zonaOpcion.containsMouse ? raiz.blanco : raiz.gris
                                        font.pixelSize: Math.round(14 * ventana.u)
                                        font.weight: opcion.actual ? Font.DemiBold : Font.Normal
                                    }
                                    Rectangle {
                                        visible: opcion.actual
                                        anchors.right: parent.right
                                        anchors.rightMargin: 18 * ventana.u
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: 7 * ventana.u; height: width; radius: width / 2
                                        color: raiz.blanco
                                    }
                                    MouseArea {
                                        id: zonaOpcion
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: raiz.elegirSesion(opcion.modelData.id)
                                    }
                                }
                            }
                        }
                    }
                    Boton {
                        texto: raiz.sesion.nombre + "  ▾"
                        u: ventana.u
                        onPulsado: raiz.eligiendoSesion = !raiz.eligiendoSesion
                    }
                }
                Text {
                    visible: ventana.esPrincipal && !selector.visible
                    anchors.left: parent.left
                    anchors.bottom: parent.bottom
                    anchors.margins: 36 * ventana.u
                    text: "Vortexine's OS"
                    color: raiz.grisOscuro
                    font.family: raiz.letraTitulos
                    font.pixelSize: Math.round(13 * ventana.u)
                }
            }
        }
    }

    component Boton: Rectangle {
        id: b
        property string texto
        property real u: 1
        signal pulsado()
        width: etiqueta.implicitWidth + 32 * u
        height: 38 * u
        radius: height / 2
        color: zona.containsMouse ? "#1E1E1E" : raiz.tarjeta
        border.width: 1
        border.color: raiz.borde
        Behavior on color { ColorAnimation { duration: 120 } }
        Text {
            id: etiqueta
            anchors.centerIn: parent
            text: b.texto
            color: zona.containsMouse ? raiz.blanco : raiz.gris
            font.pixelSize: Math.round(14 * b.u)
        }
        MouseArea { id: zona; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: b.pulsado() }
    }

    component Flecha: Rectangle {
        id: f
        property string texto
        property real u: 1
        signal pulsado()
        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        width: 36 * u; height: width; radius: width / 2
        color: zf.containsMouse ? "#1E1E1E" : "transparent"
        Text { anchors.centerIn: parent; text: f.texto; color: zf.containsMouse ? raiz.blanco : raiz.grisOscuro; font.pixelSize: Math.round(26 * f.u) }
        MouseArea { id: zf; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: f.pulsado() }
    }
}
