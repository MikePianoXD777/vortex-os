import QtQuick
import QtQuick.Layouts
import Quickshell
import Quickshell.Io
import "../"
import "../reusables"

// Vortex 1.10: «Mods de Minecraft» (calendario → botones Modrinth y CurseForge, o el lanzador). Arriba
// eliges a qué Minecraft van (las instancias de Prism, TLauncher, la app de Modrinth… o «Otra carpeta»);
// en Modrinth buscas e instalas ahí mismo con sus dependencias, y en CurseForge, que no deja instalar
// sin llave, abres su página y lo que bajes se pasa a la instancia con un clic. Todo lo hace vortex-mods.
// ModrinthPopup.qml y CurseForgePopup.qml son este mismo panel abierto en su pestaña.
Item {
    id: window
    visible: false
    focus: true

    property string pestana: "modrinth"

    function s(val) {
        return Scaler.s(val);
    }

    function t(clave, porDefecto) {
        return I18n.t("minecraft." + clave, porDefecto);
    }

    readonly property string logos: "file://" + Caching.serpantinumDir + "/assets/logos/"

    // --- instancias -------------------------------------------------------------------------------

    property var instancias: []
    property bool instanciasCargadas: false
    property string rutaElegida: ""
    readonly property var elegida: {
        for (let i = 0; i < instancias.length; i++) {
            if (instancias[i].ruta === rutaElegida) return instancias[i];
        }
        return instancias.length > 0 ? instancias[0] : null;
    }
    property string version: ""
    property string loader: ""
    readonly property var loaders: ["fabric", "neoforge", "forge", "quilt", ""]

    function elegir(inst) {
        rutaElegida = inst.ruta;
        version = inst.version || "";
        loader = inst.loader || "";
        elegidaArchivo.setText(inst.ruta + "\n");
        buscar();
        cargarInstalados();
    }

    FileView {
        id: elegidaArchivo
        printErrors: false
        path: Caching.getStateDir("minecraft") + "/instancia"
        onLoaded: window.rutaElegida = text().trim()
    }

    Process {
        id: instanciasProc
        command: ["vortex-mods", "instancias"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.instancias = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.instancias = [];
                }
                window.instanciasCargadas = true;
                if (window.elegida) {
                    window.rutaElegida = window.elegida.ruta;
                    window.version = window.elegida.version || "";
                    window.loader = window.elegida.loader || "";
                }
                window.buscar();
                window.cargarInstalados();
            }
        }
    }

    Process {
        id: agregarProc
        command: ["vortex-mods", "agregar"]
        stdout: StdioCollector {
            onStreamFinished: {
                let ruta = this.text.trim();
                if (ruta !== "") {
                    window.rutaElegida = ruta;
                    elegidaArchivo.setText(ruta + "\n");
                    instanciasProc.running = true;
                }
            }
        }
    }

    // --- Modrinth ---------------------------------------------------------------------------------

    property string tipo: "mod"
    property var resultados: []
    property bool buscando: false
    property var instalados: []
    property string instalando: ""
    property string mensaje: ""
    property bool mensajeError: false

    function filtros() {
        let args = ["--tipo", tipo];
        if (version !== "") args = args.concat(["--version", version]);
        if (loader !== "") args = args.concat(["--loader", loader]);
        return args;
    }

    // Columna izquierda: los filtros de Modrinth para el tipo elegido (se piden una vez por tipo)
    property var categorias: []
    property var categoriasPorTipo: ({})
    property var elegidas: []

    function cargarCategorias() {
        if (categoriasPorTipo[tipo]) {
            categorias = categoriasPorTipo[tipo];
            return;
        }
        categorias = [];
        categoriasProc.running = false;
        categoriasProc.deTipo = tipo;
        categoriasProc.command = ["vortex-mods", "categorias", "--tipo", tipo];
        categoriasProc.running = true;
    }

    function alternar(nombre) {
        let lista = elegidas.slice();
        let i = lista.indexOf(nombre);
        if (i === -1) lista.push(nombre); else lista.splice(i, 1);
        elegidas = lista;
        buscar();
    }

    function cambiarTipo(nuevo) {
        tipo = nuevo;
        elegidas = [];
        cargarCategorias();
        buscar();
        cargarInstalados();
    }

    function nombreCategoria(nombre) {
        let ingles = nombre.charAt(0).toUpperCase() + nombre.slice(1).replace(/-/g, " ");
        return /^[0-9]/.test(nombre) ? nombre : window.t("cat." + nombre, ingles);
    }

    function nombreGrupo(grupo) {
        let porDefecto = { "categories": "Categories", "features": "Features", "resolutions": "Resolution",
                           "performance impact": "Performance impact" };
        return window.t("group." + grupo.replace(/ /g, "_"), porDefecto[grupo] || grupo);
    }

    // El ícono de Modrinth es un SVG con «currentColor»: se pinta con el color del tema
    function iconoCategoria(svg, color) {
        if (!svg || svg.indexOf("<svg") === -1) return "";
        return "data:image/svg+xml;base64," + Qt.btoa(svg.replace(/currentColor/g, String(color)));
    }

    Process {
        id: categoriasProc
        property string deTipo: ""
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    if (Array.isArray(d)) {
                        let c = Object.assign({}, window.categoriasPorTipo);
                        c[categoriasProc.deTipo] = d;
                        window.categoriasPorTipo = c;
                        if (window.tipo === categoriasProc.deTipo) window.categorias = d;
                    }
                } catch (e) {}
            }
        }
    }

    function buscar() {
        if (!instanciasCargadas) return;
        buscarProc.running = false;
        buscando = true;
        let cats = [];
        for (let i = 0; i < elegidas.length; i++) cats = cats.concat(["--categoria", elegidas[i]]);
        buscarProc.command = ["vortex-mods", "buscar"].concat(filtros()).concat(cats).concat([busquedaInput.text.trim()]);
        buscarProc.running = true;
    }

    function cargarInstalados() {
        if (!elegida) { instalados = []; return; }
        instaladosProc.running = false;
        instaladosProc.command = ["vortex-mods", "instalados", elegida.ruta, "--tipo", tipo];
        instaladosProc.running = true;
    }

    function instalar(proyecto) {
        if (!elegida || instalarProc.running) return;
        instalando = proyecto.id;
        mensaje = "";
        instalarProc.titulo = proyecto.titulo;
        instalarProc.command = ["vortex-mods", "instalar", proyecto.id, elegida.ruta].concat(filtros());
        instalarProc.running = true;
    }

    Process {
        id: buscarProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.resultados = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.resultados = [];
                }
                window.buscando = false;
            }
        }
        onExited: function(exitCode) {
            if (exitCode !== 0) {
                window.buscando = false;
                window.mensajeError = true;
                window.mensaje = window.t("offline", "Couldn't reach Modrinth. Are you online?");
            }
        }
    }

    Process {
        id: instaladosProc
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.instalados = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.instalados = [];
                }
            }
        }
    }

    Process {
        id: instalarProc
        property string titulo: ""
        stderr: StdioCollector { id: erroresInstalar }
        onExited: function(exitCode) {
            window.instalando = "";
            window.mensajeError = exitCode !== 0;
            window.mensaje = exitCode === 0
                ? window.t("installed_in", "Installed in") + " " + window.nombreInstancia(window.elegida) + ": " + instalarProc.titulo
                : (erroresInstalar.text.trim().replace(/^vortex mods: /gm, "") || window.t("error", "It didn't work."));
            window.cargarInstalados();
        }
    }

    Timer {
        id: esperaBusqueda
        interval: 450
        onTriggered: window.buscar()
    }

    // --- CurseForge -------------------------------------------------------------------------------

    property var descargas: []
    property string moviendo: ""

    Process {
        id: descargasProc
        command: ["vortex-mods", "descargas"]
        stdout: StdioCollector {
            onStreamFinished: {
                try {
                    let d = JSON.parse(this.text);
                    window.descargas = Array.isArray(d) ? d : [];
                } catch (e) {
                    window.descargas = [];
                }
            }
        }
    }

    Process {
        id: moverProc
        property string nombre: ""
        stderr: StdioCollector { id: erroresMover }
        onExited: function(exitCode) {
            window.moviendo = "";
            window.mensajeError = exitCode !== 0;
            window.mensaje = exitCode === 0
                ? moverProc.nombre + " → " + window.nombreInstancia(window.elegida)
                : (erroresMover.text.trim().replace(/^vortex mods: /gm, "") || window.t("error", "It didn't work."));
            descargasProc.running = true;
        }
    }

    function mover(archivo) {
        if (!elegida || moverProc.running) return;
        moviendo = archivo.ruta;
        mensaje = "";
        moverProc.nombre = archivo.nombre;
        moverProc.command = ["vortex-mods", "mover", archivo.ruta, elegida.ruta];
        moverProc.running = true;
    }

    // Lo que bajes de CurseForge aparece solo mientras ves esa pestaña
    Timer {
        interval: 3000
        repeat: true
        running: window.visible && window.pestana === "curseforge"
        onTriggered: if (!descargasProc.running) descargasProc.running = true
    }

    function abrirCurseForge() {
        let url = "https://www.curseforge.com/minecraft/search?class=mc-mods";
        if (version !== "") url += "&version=" + encodeURIComponent(version);
        Qt.openUrlExternally(url);
    }

    // --- textos -----------------------------------------------------------------------------------

    function nombreInstancia(inst) {
        if (!inst) return "";
        return inst.launcher === "Prism" || inst.launcher === "MultiMC" || inst.launcher === "PolyMC"
            ? inst.launcher + " · " + inst.nombre : inst.launcher === "Tuya" ? inst.nombre : inst.launcher;
    }

    function nombreLoader(l) {
        return l === "fabric" ? "Fabric" : l === "neoforge" ? "NeoForge" : l === "forge" ? "Forge"
             : l === "quilt" ? "Quilt" : window.t("any_loader", "Any loader");
    }

    function cifra(n) {
        if (n >= 1000000) return (n / 1000000).toFixed(n >= 10000000 ? 0 : 1) + " M";
        if (n >= 1000) return Math.round(n / 1000) + " k";
        return "" + n;
    }

    // --- abrir y cerrar ---------------------------------------------------------------------------

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
            mensaje = "";
            busquedaInput.forceActiveFocus();
            instanciasProc.running = true;
            descargasProc.running = true;
            cargarCategorias();
            resetAndPlayIntro();
        } else {
            introAnim.stop();
            exitAnim.stop();
            introMain = 0;
        }
    }

    Component.onCompleted: {
        if (visible) {
            instanciasProc.running = true;
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
        property string texto: ""
        property string logo: ""
        property color fondo: ThemeBackend.surface1
        property color letra: ThemeBackend.text
        property bool activo: false
        signal pulsado()
        implicitHeight: window.s(32)
        implicitWidth: filaBoton.implicitWidth + window.s(26)
        radius: height / 2
        opacity: enabled ? 1 : 0.5
        color: zonaBoton.containsMouse ? Qt.lighter(fondo, 1.15) : fondo
        border.width: activo ? 1 : 0
        border.color: ThemeBackend.green
        Row {
            id: filaBoton
            anchors.centerIn: parent
            spacing: window.s(7)
            Image {
                visible: boton.logo !== ""
                anchors.verticalCenter: parent.verticalCenter
                source: boton.logo
                sourceSize: Qt.size(window.s(16), window.s(16))
                width: window.s(16)
                height: window.s(16)
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: boton.texto
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(12)
                font.weight: Font.Bold
                color: boton.letra
            }
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
            anchors.margins: window.s(24)
            spacing: window.s(14)

            // Título y pestañas
            RowLayout {
                Layout.fillWidth: true
                spacing: window.s(14)

                Rectangle {
                    Layout.preferredWidth: window.s(52)
                    Layout.preferredHeight: window.s(52)
                    radius: width / 2
                    color: Qt.alpha(ThemeBackend.green, 0.2)
                    Text {
                        anchors.centerIn: parent
                        text: window.instalando !== "" || window.buscando ? "󰔟" : "󰍳"
                        font.family: ThemeBackend.iconFont
                        font.pixelSize: window.s(26)
                        color: ThemeBackend.green
                    }
                }

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: window.s(2)
                    Text {
                        text: window.t("title", "Minecraft mods")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(22)
                        font.weight: Font.Bold
                        color: ThemeBackend.text
                    }
                    Text {
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: window.t("intro", "Pick your Minecraft first: each launcher has its own folder.")
                        font.family: ThemeBackend.fontFamily
                        font.pixelSize: window.s(13)
                        color: ThemeBackend.subtext0
                    }
                }

                Boton {
                    Layout.preferredHeight: window.s(38)
                    texto: "Modrinth"
                    logo: window.logos + "modrinth.svg"
                    activo: window.pestana === "modrinth"
                    fondo: Qt.alpha(ThemeBackend.surface1, activo ? 0.9 : 0.4)
                    onPulsado: window.pestana = "modrinth"
                }
                Boton {
                    Layout.preferredHeight: window.s(38)
                    texto: "CurseForge"
                    logo: window.logos + "curseforge.svg"
                    activo: window.pestana === "curseforge"
                    fondo: Qt.alpha(ThemeBackend.surface1, activo ? 0.9 : 0.4)
                    onPulsado: { window.pestana = "curseforge"; descargasProc.running = true; }
                }
            }

            // ¿A qué Minecraft?
            Flow {
                Layout.fillWidth: true
                spacing: window.s(8)

                Text {
                    height: window.s(34)
                    verticalAlignment: Text.AlignVCenter
                    text: window.t("install_to", "Install to:")
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(13)
                    font.weight: Font.Bold
                    color: ThemeBackend.subtext1
                }

                Repeater {
                    model: window.instancias
                    delegate: Rectangle {
                        id: chip
                        required property var modelData
                        readonly property bool esta: window.elegida && window.elegida.ruta === modelData.ruta
                        height: window.s(34)
                        width: chipTexto.implicitWidth + window.s(26)
                        radius: height / 2
                        color: esta ? Qt.alpha(ThemeBackend.green, 0.22) : (chipMa.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.6) : Qt.alpha(ThemeBackend.surface0, 0.5))
                        border.width: esta ? 1 : 0
                        border.color: ThemeBackend.green
                        Text {
                            id: chipTexto
                            anchors.centerIn: parent
                            textFormat: Text.StyledText
                            text: "<b>" + window.nombreInstancia(chip.modelData) + "</b>"
                                  + (chip.modelData.version !== "" && chip.modelData.version !== chip.modelData.nombre ? "  ·  " + chip.modelData.version : "")
                                  + (chip.modelData.loader !== "" ? " " + window.nombreLoader(chip.modelData.loader) : "")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(12)
                            color: chip.esta ? ThemeBackend.green : ThemeBackend.text
                        }
                        MouseArea {
                            id: chipMa
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: window.elegir(chip.modelData)
                        }
                    }
                }

                Boton {
                    height: window.s(34)
                    texto: "󰉗  " + window.t("other_folder", "Other folder…")
                    fondo: Qt.alpha(ThemeBackend.surface0, 0.5)
                    enabled: !agregarProc.running
                    onPulsado: agregarProc.running = true
                }
            }

            Text {
                visible: window.instanciasCargadas && window.instancias.length === 0
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                text: window.t("no_instances", "I didn't find any Minecraft. Open your launcher once, or add its folder with «Other folder…».")
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(13)
                color: ThemeBackend.peach
            }

            // Mensaje de lo último que pasó
            Text {
                visible: window.mensaje !== ""
                Layout.fillWidth: true
                wrapMode: Text.WordWrap
                maximumLineCount: 3
                text: window.mensaje
                font.family: ThemeBackend.fontFamily
                font.pixelSize: window.s(13)
                font.weight: Font.DemiBold
                color: window.mensajeError ? ThemeBackend.red : ThemeBackend.green
            }

            // --- Modrinth: filtros a la izquierda, como en su página ---
            RowLayout {
                visible: window.pestana === "modrinth"
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(16)

            Rectangle {
                Layout.preferredWidth: window.s(210)
                Layout.fillHeight: true
                radius: window.s(14)
                color: Qt.alpha(ThemeBackend.surface0, 0.35)

                Flickable {
                    anchors.fill: parent
                    anchors.margins: window.s(10)
                    clip: true
                    contentHeight: columnaFiltros.height
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: columnaFiltros
                        width: parent.width
                        spacing: window.s(2)

                        Text {
                            visible: window.categorias.length === 0
                            width: parent.width
                            wrapMode: Text.WordWrap
                            topPadding: window.s(8)
                            text: categoriasProc.running ? "󰔟" : window.t("no_filters", "No filters (are you online?)")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(12)
                            color: ThemeBackend.overlay1
                        }

                        Repeater {
                            model: window.categorias
                            delegate: Column {
                                id: filtro
                                required property var modelData
                                required property int index
                                readonly property bool marcado: window.elegidas.indexOf(modelData.nombre) !== -1
                                readonly property bool grupoNuevo: index === 0 || window.categorias[index - 1].grupo !== modelData.grupo
                                width: columnaFiltros.width

                                Text {
                                    visible: filtro.grupoNuevo
                                    topPadding: filtro.index === 0 ? window.s(4) : window.s(14)
                                    bottomPadding: window.s(6)
                                    leftPadding: window.s(6)
                                    text: window.nombreGrupo(filtro.modelData.grupo)
                                    font.family: ThemeBackend.fontFamily
                                    font.pixelSize: window.s(13)
                                    font.weight: Font.Bold
                                    color: ThemeBackend.text
                                }

                                Rectangle {
                                    width: parent.width
                                    height: window.s(30)
                                    radius: height / 2
                                    color: filtro.marcado ? Qt.alpha(ThemeBackend.green, 0.2) : (filtroMa.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.5) : "transparent")

                                    Row {
                                        anchors.verticalCenter: parent.verticalCenter
                                        x: window.s(10)
                                        spacing: window.s(9)
                                        Image {
                                            anchors.verticalCenter: parent.verticalCenter
                                            width: window.s(16)
                                            height: window.s(16)
                                            sourceSize: Qt.size(window.s(16), window.s(16))
                                            source: window.iconoCategoria(filtro.modelData.icono, filtro.marcado ? ThemeBackend.green : ThemeBackend.subtext0)
                                        }
                                        Text {
                                            anchors.verticalCenter: parent.verticalCenter
                                            text: window.nombreCategoria(filtro.modelData.nombre)
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(12)
                                            font.weight: filtro.marcado ? Font.Bold : Font.Normal
                                            color: filtro.marcado ? ThemeBackend.green : ThemeBackend.subtext1
                                        }
                                    }
                                    MouseArea {
                                        id: filtroMa
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: window.alternar(filtro.modelData.nombre)
                                    }
                                }
                            }
                        }

                        Text {
                            visible: window.elegidas.length > 0
                            topPadding: window.s(12)
                            leftPadding: window.s(6)
                            text: "󰅖  " + window.t("clear_filters", "Clear filters")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(12)
                            font.weight: Font.Bold
                            color: ThemeBackend.peach
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: { window.elegidas = []; window.buscar(); }
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)

                RowLayout {
                    Layout.fillWidth: true
                    spacing: window.s(8)

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: window.s(38)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface0, 0.6)
                        border.width: busquedaInput.activeFocus ? 1 : 0
                        border.color: ThemeBackend.green

                        TextInput {
                            id: busquedaInput
                            anchors.fill: parent
                            anchors.leftMargin: window.s(16)
                            anchors.rightMargin: window.s(16)
                            verticalAlignment: TextInput.AlignVCenter
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            color: ThemeBackend.text
                            clip: true
                            onTextChanged: esperaBusqueda.restart()
                            onAccepted: window.buscar()
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                visible: busquedaInput.text === ""
                                text: window.t("search_placeholder", "Search Modrinth: sodium, create, jei…")
                                font: busquedaInput.font
                                color: ThemeBackend.overlay1
                            }
                        }
                    }

                    // La versión se puede cambiar (en .minecraft o en otra carpeta se adivina)
                    Rectangle {
                        Layout.preferredWidth: window.s(90)
                        Layout.preferredHeight: window.s(38)
                        radius: height / 2
                        color: Qt.alpha(ThemeBackend.surface0, 0.6)
                        border.width: versionInput.activeFocus ? 1 : 0
                        border.color: ThemeBackend.green
                        TextInput {
                            id: versionInput
                            anchors.fill: parent
                            anchors.leftMargin: window.s(12)
                            anchors.rightMargin: window.s(12)
                            verticalAlignment: TextInput.AlignVCenter
                            horizontalAlignment: TextInput.AlignHCenter
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            color: ThemeBackend.text
                            clip: true
                            text: window.version
                            onEditingFinished: if (text.trim() !== window.version) { window.version = text.trim(); window.buscar(); }
                            Text {
                                anchors.centerIn: parent
                                visible: versionInput.text === ""
                                text: window.t("version", "Version")
                                font: versionInput.font
                                color: ThemeBackend.overlay1
                            }
                        }
                    }

                    Boton {
                        Layout.preferredHeight: window.s(38)
                        visible: window.tipo === "mod"
                        texto: window.nombreLoader(window.loader)
                        fondo: Qt.alpha(ThemeBackend.surface0, 0.6)
                        onPulsado: {
                            let i = window.loaders.indexOf(window.loader);
                            window.loader = window.loaders[(i + 1) % window.loaders.length];
                            window.buscar();
                        }
                    }
                }

                RowLayout {
                    spacing: window.s(8)
                    Repeater {
                        model: [
                            { id: "mod", icono: "󰏗", clave: "mods", texto: "Mods" },
                            { id: "resourcepack", icono: "󰉼", clave: "resourcepacks", texto: "Resource packs" },
                            { id: "shader", icono: "󰖨", clave: "shaders", texto: "Shaders" }
                        ]
                        delegate: Boton {
                            required property var modelData
                            texto: modelData.icono + "  " + window.t(modelData.clave, modelData.texto)
                            activo: window.tipo === modelData.id
                            fondo: Qt.alpha(ThemeBackend.surface1, activo ? 0.9 : 0.35)
                            onPulsado: window.cambiarTipo(modelData.id)
                        }
                    }
                }

                Flickable {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentHeight: lista.height
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: lista
                        width: parent.width
                        spacing: window.s(8)

                        Text {
                            visible: !window.buscando && window.resultados.length === 0 && window.instanciasCargadas
                            width: lista.width
                            horizontalAlignment: Text.AlignHCenter
                            topPadding: window.s(30)
                            wrapMode: Text.WordWrap
                            text: window.t("no_results", "Nothing for that version. Try another word or change the version.")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            color: ThemeBackend.subtext0
                        }

                        Repeater {
                            model: window.resultados
                            delegate: Rectangle {
                                id: fila
                                required property var modelData
                                readonly property bool yaEsta: window.instalados.indexOf(modelData.id) !== -1
                                readonly property bool bajando: window.instalando === modelData.id
                                width: lista.width
                                height: window.s(76)
                                radius: window.s(12)
                                color: filaMa.containsMouse ? Qt.alpha(ThemeBackend.surface1, 0.5) : Qt.alpha(ThemeBackend.surface0, 0.4)

                                MouseArea {
                                    id: filaMa
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.NoButton
                                }

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.margins: window.s(12)
                                    spacing: window.s(12)

                                    Rectangle {
                                        Layout.preferredWidth: window.s(52)
                                        Layout.preferredHeight: window.s(52)
                                        radius: window.s(10)
                                        color: Qt.alpha(ThemeBackend.surface1, 0.5)
                                        clip: true
                                        Image {
                                            anchors.fill: parent
                                            source: fila.modelData.icono
                                            sourceSize: Qt.size(window.s(52), window.s(52))
                                            fillMode: Image.PreserveAspectCrop
                                            asynchronous: true
                                        }
                                    }

                                    ColumnLayout {
                                        Layout.fillWidth: true
                                        spacing: window.s(3)
                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            textFormat: Text.StyledText
                                            text: "<b>" + fila.modelData.titulo + "</b>  <font color=\"" + ThemeBackend.overlay1 + "\">"
                                                  + window.t("by", "by") + " " + fila.modelData.autor + " · 󰇚 " + window.cifra(fila.modelData.descargas) + "</font>"
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(14)
                                            color: ThemeBackend.text
                                        }
                                        Text {
                                            Layout.fillWidth: true
                                            elide: Text.ElideRight
                                            maximumLineCount: 2
                                            wrapMode: Text.WordWrap
                                            text: fila.modelData.descripcion
                                            font.family: ThemeBackend.fontFamily
                                            font.pixelSize: window.s(12)
                                            color: ThemeBackend.subtext0
                                        }
                                    }

                                    Boton {
                                        Layout.preferredHeight: window.s(34)
                                        texto: fila.bajando ? "󰔟  " + window.t("installing", "Installing…")
                                             : fila.yaEsta ? "󰄬  " + window.t("installed", "Installed")
                                             : "󰇚  " + window.t("install", "Install")
                                        fondo: fila.yaEsta ? Qt.alpha(ThemeBackend.green, 0.18) : ThemeBackend.green
                                        letra: fila.yaEsta ? ThemeBackend.green : ThemeBackend.crust
                                        enabled: window.elegida !== null && window.instalando === "" && !fila.yaEsta
                                        onPulsado: window.instalar(fila.modelData)
                                    }
                                }
                            }
                        }
                    }
                }
            }

            }

            // --- CurseForge ---
            ColumnLayout {
                visible: window.pestana === "curseforge"
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: window.s(12)

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: cfColumna.implicitHeight + window.s(32)
                    radius: window.s(14)
                    color: Qt.alpha("#F16436", 0.12)
                    border.width: 1
                    border.color: Qt.alpha("#F16436", 0.4)

                    RowLayout {
                        anchors.fill: parent
                        anchors.margins: window.s(16)
                        spacing: window.s(16)

                        Image {
                            source: window.logos + "curseforge.svg"
                            sourceSize: Qt.size(window.s(48), window.s(48))
                            Layout.preferredWidth: window.s(48)
                            Layout.preferredHeight: window.s(48)
                        }

                        ColumnLayout {
                            id: cfColumna
                            Layout.fillWidth: true
                            spacing: window.s(4)
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: window.t("cf_title", "1. Download the mod from CurseForge")
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(15)
                                font.weight: Font.Bold
                                color: ThemeBackend.text
                            }
                            Text {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: window.t("cf_help", "CurseForge doesn't let other apps install, so it opens in your browser. Pick the file for your version; when it lands in Downloads it shows up below.")
                                font.family: ThemeBackend.fontFamily
                                font.pixelSize: window.s(12)
                                color: ThemeBackend.subtext0
                            }
                        }

                        Boton {
                            Layout.preferredHeight: window.s(40)
                            texto: "󰏌  " + window.t("cf_open", "Open CurseForge")
                            fondo: "#F16436"
                            letra: "#ffffff"
                            onPulsado: window.abrirCurseForge()
                        }
                    }
                }

                Text {
                    text: window.t("cf_downloads", "2. Move what you downloaded to") + " " + window.nombreInstancia(window.elegida)
                    font.family: ThemeBackend.fontFamily
                    font.pixelSize: window.s(15)
                    font.weight: Font.Bold
                    color: ThemeBackend.text
                }

                Flickable {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    contentHeight: listaDescargas.height
                    boundsBehavior: Flickable.StopAtBounds

                    Column {
                        id: listaDescargas
                        width: parent.width
                        spacing: window.s(8)

                        Text {
                            visible: window.descargas.length === 0
                            width: listaDescargas.width
                            horizontalAlignment: Text.AlignHCenter
                            topPadding: window.s(24)
                            wrapMode: Text.WordWrap
                            text: window.t("cf_empty", "No .jar or .zip downloaded in the last few days. Download one and it appears here by itself.")
                            font.family: ThemeBackend.fontFamily
                            font.pixelSize: window.s(13)
                            color: ThemeBackend.subtext0
                        }

                        Repeater {
                            model: window.descargas
                            delegate: Rectangle {
                                id: filaDescarga
                                required property var modelData
                                width: listaDescargas.width
                                height: window.s(56)
                                radius: window.s(12)
                                color: Qt.alpha(ThemeBackend.surface0, 0.4)

                                RowLayout {
                                    anchors.fill: parent
                                    anchors.leftMargin: window.s(16)
                                    anchors.rightMargin: window.s(12)
                                    spacing: window.s(12)
                                    Text {
                                        text: filaDescarga.modelData.nombre.toLowerCase().endsWith(".jar") ? "󰏗" : "󰗄"
                                        font.family: ThemeBackend.iconFont
                                        font.pixelSize: window.s(20)
                                        color: ThemeBackend.peach
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        elide: Text.ElideMiddle
                                        text: filaDescarga.modelData.nombre
                                        font.family: ThemeBackend.fontFamily
                                        font.pixelSize: window.s(13)
                                        font.weight: Font.DemiBold
                                        color: ThemeBackend.text
                                    }
                                    Boton {
                                        Layout.preferredHeight: window.s(34)
                                        texto: window.moviendo === filaDescarga.modelData.ruta ? "󰔟" : "󰁔  " + window.t("move", "Move there")
                                        fondo: ThemeBackend.green
                                        letra: ThemeBackend.crust
                                        enabled: window.elegida !== null && window.moviendo === ""
                                        onPulsado: window.mover(filaDescarga.modelData)
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
