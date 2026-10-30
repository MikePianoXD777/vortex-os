// La ventana de Vortexine's Camera, como Imou Life en la PC: a la izquierda tus cámaras, arriba las
// ventanas del mosaico (pestañas) y cuántos cuadros lleva cada una, y al centro el mosaico en vivo.
// Doble clic en un cuadro la abre en grande; F11, pantalla completa; Ctrl+N agrega una cámara.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    width: 1440
    height: 900
    minimumWidth: 980
    minimumHeight: 620
    visible: true
    title: "Vortexine's Camera"
    color: Tema.fondo

    property string pagina: "vivo"          // vivo | grabaciones
    property int actual: 0                  // la ventana (pestaña) del mosaico que se ve
    property string grande: ""              // la cámara abierta en grande
    property string filtro: ""
    readonly property var vActual: cam.ventanas[Math.min(actual, cam.ventanas.length - 1)] || ({ division: 4, celdas: [] })
    readonly property bool completa: visibility === Window.FullScreen

    function verGrande(id) { pagina = "vivo"; grande = id }
    function verGrabaciones(id) { grande = ""; pagina = "grabaciones"; grabaciones.elegir(id) }
    function alternarCompleta() { visibility = completa ? Window.Windowed : Window.FullScreen }

    Connections {
        target: cam
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
        function onAviso(texto, esError) { aviso.mostrar(texto, esError) }
        function onCamarasCambio() { if (ventana.grande && !cam.camara(ventana.grande).id) ventana.grande = "" }
    }

    Shortcut { sequence: "Ctrl+N"; onActivated: dialogo.abrir("") }
    Shortcut { sequence: "Ctrl+1"; onActivated: ventana.pagina = "vivo" }
    Shortcut { sequence: "Ctrl+2"; onActivated: ventana.pagina = "grabaciones" }
    Shortcut { sequence: "F11"; onActivated: ventana.alternarCompleta() }
    Shortcut {
        sequence: "Esc"
        enabled: ventana.grande !== "" || ventana.completa
        onActivated: ventana.grande !== "" ? ventana.grande = "" : ventana.alternarCompleta()
    }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }

    component Navegar: Rectangle {
        id: nav
        property string texto: ""
        property string icono: ""
        property string atajo: ""
        property bool elegido: false
        signal pulsado()
        Layout.fillWidth: true
        implicitHeight: 42
        radius: 12
        color: elegido ? Tema.barraSel : zonaNav.containsMouse ? Tema.barraHover : "transparent"
        Behavior on color { ColorAnimation { duration: 120 } }
        Rectangle {
            width: 3; height: nav.elegido ? 18 : 0; radius: 2
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            color: Tema.azul
            Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 12
            spacing: 12
            Icono { Layout.preferredWidth: 19; Layout.preferredHeight: 19; nombre: nav.icono; color: nav.elegido ? Tema.azul : Tema.suave }
            Text {
                Layout.fillWidth: true
                text: nav.texto
                color: nav.elegido ? Tema.texto : Tema.suave
                font.pixelSize: 14
                font.weight: nav.elegido ? Font.DemiBold : Font.Normal
            }
            Text { text: nav.atajo; color: Tema.tenue; font.pixelSize: 11; opacity: zonaNav.containsMouse ? 1 : 0 }
        }
        MouseArea { id: zonaNav; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: nav.pulsado() }
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0

        // --- lateral: tus cámaras ---
        Rectangle {
            visible: !ventana.completa
            Layout.preferredWidth: Tema.anchoLateral
            Layout.fillHeight: true
            color: Tema.barra
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 6
                RowLayout {
                    Layout.leftMargin: 6
                    Layout.bottomMargin: 14
                    Layout.topMargin: 6
                    spacing: 10
                    Image { source: "logo.svg"; sourceSize.width: 64; sourceSize.height: 64; Layout.preferredWidth: 34; Layout.preferredHeight: 34 }
                    ColumnLayout {
                        spacing: 0
                        Text { text: "Vortexine's"; color: Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 1.2; font.capitalization: Font.AllUppercase }
                        Text { text: "Camera"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.Bold }
                    }
                }
                Navegar { texto: "En vivo"; icono: "vivo"; atajo: "Ctrl+1"; elegido: ventana.pagina === "vivo"; onPulsado: ventana.pagina = "vivo" }
                Navegar { texto: "Grabaciones"; icono: "historial"; atajo: "Ctrl+2"; elegido: ventana.pagina === "grabaciones"; onPulsado: ventana.pagina = "grabaciones" }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 14
                    Layout.leftMargin: 6
                    Text {
                        Layout.fillWidth: true
                        text: "Cámaras" + (cam.camaras.length ? " · " + cam.camaras.length : "")
                        color: Tema.tenue
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                        font.letterSpacing: 1.2
                        font.capitalization: Font.AllUppercase
                    }
                    BotonIcono { tam: 30; icono: "mas"; ayuda: "Agregar cámara (Ctrl+N)"; onClicked: dialogo.abrir("") }
                }
                Campo {
                    Layout.fillWidth: true
                    visible: cam.camaras.length > 6
                    placeholderText: "Buscar cámara"
                    onTextChanged: ventana.filtro = text.toLowerCase()
                }
                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 2
                    boundsBehavior: Flickable.StopAtBounds
                    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                    model: cam.camaras.filter(c => !ventana.filtro || c.nombre.toLowerCase().indexOf(ventana.filtro) >= 0)
                    delegate: Rectangle {
                        id: fila
                        required property var modelData
                        readonly property string estado: cam.estados[modelData.id] || ""
                        readonly property bool grabando: cam.grabando[modelData.id] === true
                        width: lista.width
                        height: 52
                        radius: 12
                        color: ventana.grande === modelData.id ? Tema.barraSel : zonaFila.containsMouse ? Tema.barraHover : "transparent"
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 10
                            anchors.rightMargin: 4
                            spacing: 10
                            Rectangle {
                                Layout.preferredWidth: 34
                                Layout.preferredHeight: 34
                                radius: 10
                                color: Tema.tarjeta
                                Icono { anchors.centerIn: parent; width: 18; height: 18; nombre: "camara"; color: Tema.suave }
                                Rectangle {
                                    width: 10; height: 10; radius: 5
                                    anchors.right: parent.right
                                    anchors.bottom: parent.bottom
                                    anchors.margins: -1
                                    border.width: 2
                                    border.color: Tema.barra
                                    color: fila.estado === "en_linea" ? Tema.exito : fila.estado === "revisando" ? Tema.tenue : Tema.peligro
                                }
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 1
                                Text {
                                    Layout.fillWidth: true
                                    text: fila.modelData.nombre
                                    color: Tema.texto
                                    font.pixelSize: 14
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: (fila.grabando ? "● Grabando · " : "")
                                        + (fila.estado === "en_linea" ? "En línea" : fila.estado === "sin_conexion" ? "Sin conexión"
                                           : fila.estado === "sin_ip" ? "Sin IP" : fila.estado === "codigo" ? "Código incorrecto" : "Revisando…")
                                    color: fila.grabando ? Tema.peligro : Tema.tenue
                                    font.pixelSize: 11
                                    elide: Text.ElideRight
                                }
                            }
                            BotonIcono {
                                tam: 30
                                icono: "mas_puntos"
                                ayuda: "Opciones"
                                opacity: zonaFila.containsMouse || menuFila.visible ? 1 : 0
                                onClicked: menuFila.popup()
                            }
                        }
                        MouseArea {
                            id: zonaFila
                            anchors.fill: parent
                            z: -1
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            cursorShape: Qt.PointingHandCursor
                            onClicked: (e) => e.button === Qt.RightButton ? menuFila.popup() : ventana.verGrande(fila.modelData.id)
                        }
                        MenuVortex {
                            id: menuFila
                            OpcionMenu { text: "Ver en grande"; icono: "pantalla"; onTriggered: ventana.verGrande(fila.modelData.id) }
                            OpcionMenu { text: "Ver grabaciones"; icono: "historial"; onTriggered: ventana.verGrabaciones(fila.modelData.id) }
                            OpcionMenu { text: "Sacar foto"; icono: "foto"; onTriggered: cam.foto(fila.modelData.id) }
                            OpcionMenu {
                                text: fila.grabando ? "Dejar de grabar" : "Grabar"
                                icono: fila.grabando ? "detener" : "grabar"
                                onTriggered: cam.alternarGrabar(fila.modelData.id)
                            }
                            OpcionMenu { text: "Editar"; icono: "editar"; onTriggered: dialogo.abrir(fila.modelData.id) }
                            OpcionMenu { text: "Quitar"; icono: "basura"; peligro: true; onTriggered: confirmar.abrir(fila.modelData) }
                        }
                    }
                }
                Text {
                    visible: cam.camaras.length === 0
                    Layout.fillWidth: true
                    Layout.leftMargin: 6
                    text: "Todavía no tienes cámaras."
                    color: Tema.tenue
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Boton { Layout.fillWidth: true; tipo: "fantasma"; icono: "foto"; text: "Fotos"; onClicked: cam.abrirCarpeta("fotos") }
                    Boton { Layout.fillWidth: true; tipo: "fantasma"; icono: "carpeta"; text: "Videos"; onClicked: cam.abrirCarpeta("videos") }
                }
            }
        }

        // --- centro ---
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: ventana.pagina === "grabaciones" ? 1 : 0

            Item {
                id: vivo
                ColumnLayout {
                    anchors.fill: parent
                    spacing: 0

                    // pestañas y división
                    RowLayout {
                        visible: !ventana.completa
                        Layout.fillWidth: true
                        Layout.preferredHeight: 60
                        Layout.leftMargin: 16
                        Layout.rightMargin: 16
                        spacing: 6
                        Repeater {
                            model: cam.ventanas
                            delegate: Rectangle {
                                id: pestana
                                required property var modelData
                                required property int index
                                readonly property bool elegida: index === ventana.actual
                                implicitWidth: textoPestana.implicitWidth + 32
                                implicitHeight: 36
                                radius: 18
                                color: elegida ? Tema.realceFuerte : zonaP.containsMouse ? Tema.realce : "transparent"
                                border.width: elegida ? 1 : 0
                                border.color: Tema.bordeHover
                                Text {
                                    id: textoPestana
                                    anchors.centerIn: parent
                                    text: pestana.modelData.nombre
                                    color: pestana.elegida ? Tema.texto : Tema.suave
                                    font.pixelSize: 13
                                    font.weight: pestana.elegida ? Font.DemiBold : Font.Normal
                                }
                                MouseArea {
                                    id: zonaP
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: (e) => { ventana.actual = pestana.index; if (e.button === Qt.RightButton) menuP.popup() }
                                    onDoubleClicked: renombrar.abrir(pestana.index)
                                }
                                MenuVortex {
                                    id: menuP
                                    OpcionMenu { text: "Cambiar nombre"; icono: "editar"; onTriggered: renombrar.abrir(pestana.index) }
                                    OpcionMenu {
                                        text: "Quitar esta ventana"
                                        icono: "cerrar"
                                        peligro: true
                                        enabled: cam.ventanas.length > 1
                                        onTriggered: { cam.quitarVentana(pestana.index); ventana.actual = Math.max(0, ventana.actual - 1) }
                                    }
                                }
                            }
                        }
                        BotonIcono { tam: 34; icono: "mas"; ayuda: "Otra ventana"; onClicked: ventana.actual = cam.nuevaVentana() }
                        Item { Layout.fillWidth: true }
                        Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "mosaico"; color: Tema.suave }
                        Selector {
                            Layout.preferredWidth: 150
                            model: cam.divisiones
                            textoDe: (n) => n === 1 ? "1 cámara" : n + " cuadros"
                            displayText: textoDe(ventana.vActual.division)
                            currentIndex: cam.divisiones.indexOf(ventana.vActual.division)
                            onActivated: (i) => cam.ponerDivision(ventana.actual, cam.divisiones[i])
                        }
                        BotonIcono { icono: "pantalla"; ayuda: "Pantalla completa (F11)"; onClicked: ventana.alternarCompleta() }
                    }

                    // el mosaico
                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Layout.leftMargin: ventana.completa ? 0 : 16
                        Layout.rightMargin: ventana.completa ? 0 : 16
                        Layout.bottomMargin: ventana.completa ? 0 : 16

                        Rectangle {
                            anchors.fill: parent
                            radius: ventana.completa ? 0 : Tema.radioTarjeta
                            color: "#020308"
                            clip: true
                            Grid {
                                id: rejilla
                                // Cuántas columnas dejan cada video (16:9) más grande en este espacio: 4 → 2×2,
                                // 9 → 3×3… y 32 en pantalla ancha → 6 columnas (la última fila con 2).
                                readonly property int lado: {
                                    var n = ventana.vActual.division, mejor = 1, mayor = -1
                                    for (var c = 1; c <= n; c++) {
                                        var filas = Math.ceil(n / c)
                                        var w = (width - spacing * (c - 1)) / c, h = (height - spacing * (filas - 1)) / filas
                                        var video = Math.min(w, h * 16 / 9)
                                        if (video > mayor + 0.5) { mayor = video; mejor = c }
                                    }
                                    return mejor
                                }
                                readonly property int filas: Math.ceil(ventana.vActual.division / lado)
                                anchors.fill: parent
                                columns: lado
                                spacing: 2
                                Repeater {
                                    model: ventana.vActual.celdas
                                    delegate: Celda {
                                        required property var modelData
                                        required property int index
                                        width: (rejilla.width - rejilla.spacing * (rejilla.lado - 1)) / rejilla.lado
                                        height: (rejilla.height - rejilla.spacing * (rejilla.filas - 1)) / rejilla.filas
                                        camId: modelData
                                        pos: index
                                        principal: ventana.vActual.division === 1
                                        activo: ventana.pagina === "vivo" && ventana.grande === ""
                                        onVerGrande: (id) => ventana.verGrande(id)
                                        onElegirCamara: (p) => elegir.abrir(p)
                                        onQuitarDeAqui: (p) => cam.ponerCelda(ventana.actual, p, "")
                                    }
                                }
                            }

                            // bienvenida, sin cámaras
                            Rectangle {
                                anchors.fill: parent
                                visible: cam.camaras.length === 0
                                color: Tema.fondo
                                ColumnLayout {
                                    anchors.centerIn: parent
                                    width: Math.min(460, parent.width - 48)
                                    spacing: 14
                                    Image {
                                        Layout.alignment: Qt.AlignHCenter
                                        source: "logo.svg"
                                        sourceSize.width: 192; sourceSize.height: 192
                                        Layout.preferredWidth: 96; Layout.preferredHeight: 96
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        horizontalAlignment: Text.AlignHCenter
                                        text: "Agrega tu primera cámara"
                                        color: Tema.texto
                                        font.pixelSize: 24
                                        font.weight: Font.DemiBold
                                    }
                                    Text {
                                        Layout.fillWidth: true
                                        horizontalAlignment: Text.AlignHCenter
                                        text: "Solo necesitas el número de serie y el código de seguridad de su etiqueta. "
                                            + "La busco en tu red y la ves directo, sin la nube ni límites."
                                        color: Tema.suave
                                        font.pixelSize: 14
                                        wrapMode: Text.Wrap
                                    }
                                    Boton {
                                        Layout.alignment: Qt.AlignHCenter
                                        Layout.topMargin: 6
                                        grande: true
                                        icono: "mas"
                                        text: "Agregar cámara"
                                        onClicked: dialogo.abrir("")
                                    }
                                }
                            }

                            VistaGrande {
                                anchors.fill: parent
                                visible: ventana.grande !== "" && ventana.pagina === "vivo"
                                camId: ventana.grande
                                onCerrar: ventana.grande = ""
                                onVerGrabaciones: (id) => ventana.verGrabaciones(id)
                            }
                        }
                    }
                }
            }

            PaginaGrabaciones {
                id: grabaciones
                activa: ventana.pagina === "grabaciones"
            }
        }
    }

    DialogoCamara { id: dialogo }

    // Qué cámara poner en un cuadro
    MenuVortex {
        id: elegir
        property int pos: 0
        function abrir(p) { pos = p; popup() }
        Instantiator {
            model: cam.camaras
            delegate: OpcionMenu {
                required property var modelData
                text: modelData.nombre
                icono: "camara"
                checked: ventana.vActual.celdas[elegir.pos] === modelData.id
                onTriggered: cam.ponerCelda(ventana.actual, elegir.pos, modelData.id)
            }
            onObjectAdded: (i, o) => elegir.insertItem(i, o)
            onObjectRemoved: (i, o) => elegir.removeItem(o)
        }
        MenuSeparator {}
        OpcionMenu { text: "Agregar cámara nueva…"; icono: "mas"; onTriggered: dialogo.abrir("") }
    }

    // ¿Quitar la cámara?
    Popup {
        id: confirmar
        property var camara: ({})
        function abrir(c) { camara = c; open() }
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 420
        modal: true
        padding: 26
        Overlay.modal: Rectangle { color: Tema.velo }
        background: Rectangle { radius: Tema.radioGrande; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
        contentItem: ColumnLayout {
            spacing: 14
            Text { text: "¿Quitar «" + (confirmar.camara.nombre || "") + "»?"; color: Tema.texto; font.pixelSize: 19; font.weight: Font.DemiBold; Layout.fillWidth: true; wrapMode: Text.Wrap }
            Text {
                Layout.fillWidth: true
                text: "Se quita de la app. La cámara sigue igual y lo que grabó en su microSD no se borra."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: confirmar.close() }
                Boton { tipo: "peligro"; icono: "basura"; text: "Quitar"; onClicked: { cam.quitarCamara(confirmar.camara.id); confirmar.close() } }
            }
        }
    }

    // Nombre de una ventana del mosaico
    Popup {
        id: renombrar
        property int indice: 0
        function abrir(i) { indice = i; campoNombre.text = cam.ventanas[i].nombre; open(); campoNombre.campo.selectAll(); campoNombre.campo.forceActiveFocus() }
        function guardar() { cam.renombrarVentana(indice, campoNombre.text); close() }
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 380
        modal: true
        padding: 24
        Overlay.modal: Rectangle { color: Tema.velo }
        background: Rectangle { radius: Tema.radioGrande; color: Tema.tarjeta; border.width: 1; border.color: Tema.borde }
        contentItem: ColumnLayout {
            spacing: 14
            Entrada { id: campoNombre; Layout.fillWidth: true; etiqueta: "Nombre de la ventana"; onAceptado: renombrar.guardar() }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: renombrar.close() }
                Boton { text: "Guardar"; onClicked: renombrar.guardar() }
            }
        }
    }

    // Aviso abajo al centro
    Rectangle {
        id: aviso
        property string texto: ""
        property bool esError: false
        function mostrar(t, e) { texto = t; esError = e; opacity = 1; ocultar.restart() }
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 28
        anchors.horizontalCenterOffset: ventana.completa ? 0 : Tema.anchoLateral / 2
        width: Math.min(600, textoAviso.implicitWidth + 64)
        height: Math.max(46, textoAviso.implicitHeight + 24)
        radius: 23
        color: Tema.tarjeta
        border.width: 1
        border.color: esError ? Tema.peligroBorde : Tema.borde
        opacity: 0
        visible: opacity > 0
        z: 10
        Behavior on opacity { NumberAnimation { duration: 180 } }
        Row {
            anchors.centerIn: parent
            spacing: 10
            Icono { width: 18; height: 18; nombre: aviso.esError ? "alerta" : "visto"; color: aviso.esError ? Tema.peligro : Tema.exito; anchors.verticalCenter: parent.verticalCenter }
            Text {
                id: textoAviso
                width: Math.min(implicitWidth, 520)
                text: aviso.texto
                color: Tema.texto
                font.pixelSize: 13
                wrapMode: Text.Wrap
                anchors.verticalCenter: parent.verticalCenter
            }
        }
        Timer { id: ocultar; interval: 5000; onTriggered: aviso.opacity = 0 }
    }
}
