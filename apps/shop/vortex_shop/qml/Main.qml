// Ventana de Vortexine's Shop: barra lateral, buscador y las páginas (una pila: el detalle se apila
// sobre la página desde la que se abrió, y «Atrás» vuelve a ella tal cual estaba).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    visible: true
    width: 1180
    height: 760
    minimumWidth: 860
    minimumHeight: 560
    title: "Vortexine's Shop"
    color: Tema.fondo

    property string seccion: "inicio"      // la sección activa en la barra lateral
    property string categoriaId: ""
    property bool salidaConfirmada: false
    property bool _limpiando: false         // evita que borrar el buscador a mano lance otra búsqueda

    // --- Navegación (también la usan las pruebas) ---

    function navegar(nombre, arg) {
        seccion = nombre
        categoriaId = nombre === "categoria" ? arg : ""
        if (nombre !== "busqueda")
            limpiarBuscador()
        var pagina = nombre === "inicio" ? cInicio
                   : nombre === "categoria" ? cCategoria
                   : nombre === "busqueda" ? cBusqueda
                   : nombre === "instaladas" ? cInstaladas : cActualizaciones
        if (nombre === "categoria")
            tienda.abrirCategoria(arg)
        if (nombre === "actualizaciones")
            tienda.buscarActualizaciones()
        pila.replace(null, pagina)
    }

    function verDetalle(appId) {
        tienda.abrirDetalle(appId)
        pila.push(cDetalle)
    }

    function atras() {
        if (pila.depth > 1)
            pila.pop()
    }

    function pedirQuitar(appId, nombre) {
        dialogoQuitar.appId = appId
        dialogoQuitar.nombre = nombre
        dialogoQuitar.abierto = true
    }

    function cerrarDialogos() {   // para las pruebas
        dialogoQuitar.abierto = false
        dialogoSalir.abierto = false
    }

    function limpiarBuscador() {
        _limpiando = true
        buscador.text = ""
        _limpiando = false
    }

    function escribirBusqueda(texto) {   // como si se escribiera en el buscador y pasara la espera
        buscador.text = texto
        esperaBusqueda.stop()
        buscar()
    }

    function buscar() {
        var t = buscador.text.trim()
        if (t === "") {
            if (seccion === "busqueda")
                navegar("inicio", "")
            return
        }
        tienda.buscar(t)
        if (seccion !== "busqueda" || pila.depth > 1) {
            seccion = "busqueda"
            categoriaId = ""
            pila.replace(null, cBusqueda)
        }
    }

    Connections {
        target: Nav
        function onDetalle(appId) { ventana.verDetalle(appId) }
        function onSeccion(nombre, arg) { ventana.navegar(nombre, arg) }
        function onAtras() { ventana.atras() }
        function onQuitarPedido(appId, nombre) { ventana.pedirQuitar(appId, nombre) }
    }
    Connections {
        target: tienda
        function onAviso(texto) { aviso.mostrar(texto) }
        function onMostrarVentana() {
            ventana.show()
            ventana.raise()
            ventana.requestActivate()
        }
    }

    // Cerrar con algo instalándose pregunta antes.
    onClosing: function (cierre) {
        if (tienda.ocupado && !salidaConfirmada) {
            cierre.accepted = false
            dialogoSalir.abierto = true
        }
    }

    Shortcut { sequence: "Ctrl+F"; onActivated: { buscador.forceActiveFocus(); buscador.selectAll() } }
    Shortcut { sequence: "Alt+Left"; onActivated: ventana.atras() }

    Component { id: cInicio; PaginaInicio {} }
    Component { id: cCategoria; PaginaCategoria {} }
    Component { id: cBusqueda; PaginaBusqueda {} }
    Component { id: cDetalle; PaginaDetalle {} }
    Component { id: cInstaladas; PaginaInstaladas {} }
    Component { id: cActualizaciones; PaginaActualizaciones {} }

    BarraLateral {
        id: lateral
        anchors.left: parent.left
        anchors.top: parent.top
        anchors.bottom: parent.bottom
        width: Tema.anchoLateral
        seccion: ventana.seccion
        categoriaId: ventana.categoriaId
    }

    Item {
        anchors.left: lateral.right
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.bottom: parent.bottom

        // Buscador
        Item {
            id: cabecera
            anchors.left: parent.left
            anchors.right: parent.right
            height: 76
            readonly property int margen: width < 1000 ? 24 : 36

            TextField {
                id: buscador
                x: cabecera.margen
                anchors.verticalCenter: parent.verticalCenter
                width: Math.min(parent.width - cabecera.margen * 2, 720)
                height: 48
                leftPadding: 50
                rightPadding: 46
                placeholderText: "Busca apps y juegos…"
                placeholderTextColor: Tema.tenue
                color: Tema.texto
                selectionColor: Tema.bordeHover
                selectedTextColor: Tema.texto
                font.pixelSize: 16
                verticalAlignment: TextInput.AlignVCenter
                inputMethodHints: Qt.ImhNoPredictiveText
                background: Rectangle {
                    radius: 24
                    color: Tema.campo
                    border.width: buscador.activeFocus ? 2 : 1
                    border.color: buscador.activeFocus ? Tema.azul : Tema.bordeCampo
                }
                Icono {
                    x: 18
                    anchors.verticalCenter: parent.verticalCenter
                    width: 20
                    height: 20
                    nombre: "buscar"
                    color: buscador.activeFocus ? Tema.azul : Tema.tenue
                }
                Rectangle {   // limpiar
                    visible: buscador.text !== ""
                    anchors.right: parent.right
                    anchors.rightMargin: 8
                    anchors.verticalCenter: parent.verticalCenter
                    width: 32
                    height: 32
                    radius: 16
                    color: limpiar.containsMouse ? Tema.realceFuerte : "transparent"
                    Icono { anchors.centerIn: parent; width: 16; height: 16; nombre: "cerrar"; color: Tema.suave }
                    MouseArea {
                        id: limpiar
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { buscador.text = ""; buscador.forceActiveFocus() }
                    }
                    Accessible.role: Accessible.Button
                    Accessible.name: "Borrar búsqueda"
                }
                onTextChanged: if (!ventana._limpiando) esperaBusqueda.restart()
                onAccepted: { esperaBusqueda.stop(); ventana.buscar() }
                Keys.onEscapePressed: text = ""
            }
            Timer { id: esperaBusqueda; interval: 300; onTriggered: ventana.buscar() }
        }

        StackView {
            id: pila
            anchors.top: cabecera.bottom
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            initialItem: cInicio
            clip: true
            pushEnter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160 } }
            pushExit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 120 } }
            popEnter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160 } }
            popExit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 120 } }
            replaceEnter: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160 } }
            replaceExit: Transition { NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 100 } }
        }
    }

    Aviso { id: aviso }

    Dialogo {
        id: dialogoQuitar
        property string appId: ""
        property string nombre: ""
        titulo: "¿Quitar " + nombre + "?"
        texto: "Se quitará la app de tu equipo. Podrás volver a instalarla cuando quieras."
        textoAceptar: "Quitar"
        tipoAceptar: "peligro"
        onAceptado: { abierto = false; tienda.quitar(appId) }
        onCancelado: abierto = false
    }

    Dialogo {
        id: dialogoSalir
        titulo: "¿Salir de la tienda?"
        texto: "Todavía se está instalando o actualizando algo. Si sales ahora, se interrumpirá."
        textoAceptar: "Salir de todos modos"
        textoCancelar: "Seguir aquí"
        tipoAceptar: "peligro"
        onAceptado: { abierto = false; ventana.salidaConfirmada = true; ventana.close() }
        onCancelado: abierto = false
    }
}
