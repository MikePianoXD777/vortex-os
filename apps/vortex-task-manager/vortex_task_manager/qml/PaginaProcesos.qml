// Procesos: arriba un resumen (cómo va la PC y qué es lo que más usa), abajo la tabla como la de
// Windows: Apps, Segundo plano y Del sistema; cada app se abre para ver sus procesos. Las celdas se
// pintan más fuerte mientras más usan. Clic derecho: cerrar, finalizar, forzar, suspender, prioridad…
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: pagina
    property bool activa: false
    property string seleccion: ""
    readonly property int anchoCpu: 92
    readonly property int anchoMem: 112
    readonly property int anchoDisco: 104
    readonly property var res: tm.resumen
    readonly property real memTotal: res.memTotal || 1

    function calor(r) {
        return r <= 0.004 ? "transparent" : Qt.rgba(Tema.azul.r, Tema.azul.g, Tema.azul.b, 0.07 + 0.42 * Math.min(1, r))
    }
    function finalizar(forzar) {
        if (seleccion === "") return
        tm.finalizar(seleccion, forzar)
        seleccion = ""
    }

    Shortcut { enabled: pagina.activa; sequence: "Ctrl+F"; onActivated: buscador.forceActiveFocus() }
    Shortcut { enabled: pagina.activa && pagina.seleccion !== "" && !buscador.activeFocus; sequence: "Delete"; onActivated: pagina.finalizar(false) }
    Shortcut { enabled: pagina.activa && pagina.seleccion !== "" && !buscador.activeFocus; sequence: "Shift+Delete"; onActivated: pagina.finalizar(true) }

    // Una tarjetita del resumen.
    component Resumen: Rectangle {
        id: tarjeta
        property string titulo: ""
        property string valor: ""
        property string detalle: ""
        property string icono: ""
        property string iconoApp: ""
        property color tinta: Tema.azul
        Layout.fillWidth: true
        implicitHeight: 76
        radius: Tema.radioTarjeta - 4
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde
        RowLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 14
            Rectangle {
                Layout.preferredWidth: 44; Layout.preferredHeight: 44
                radius: 14
                color: Qt.rgba(tarjeta.tinta.r, tarjeta.tinta.g, tarjeta.tinta.b, 0.14)
                Image {
                    anchors.centerIn: parent
                    visible: tarjeta.iconoApp !== ""
                    width: 26; height: 26
                    sourceSize.width: 52; sourceSize.height: 52
                    source: tarjeta.iconoApp !== "" ? "image://icono/" + tarjeta.iconoApp : ""
                }
                Icono {
                    anchors.centerIn: parent
                    visible: tarjeta.iconoApp === ""
                    width: 22; height: 22
                    nombre: tarjeta.icono
                    color: tarjeta.tinta
                }
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { Layout.fillWidth: true; text: tarjeta.titulo; color: Tema.tenue; font.pixelSize: 12; elide: Text.ElideRight }
                Text { Layout.fillWidth: true; text: tarjeta.valor; color: Tema.texto; font.pixelSize: 16; font.weight: Font.DemiBold; elide: Text.ElideRight }
                Text { Layout.fillWidth: true; text: tarjeta.detalle; color: Tema.suave; font.pixelSize: 12; elide: Text.ElideRight; visible: text !== "" }
            }
        }
    }

    // Cabecera de una columna numérica: el total arriba (como Windows) y el nombre abajo; clic = ordenar.
    component Columna: Item {
        id: col
        property string texto: ""
        property string total: ""
        property string orden: ""
        readonly property bool activa: tm.orden === orden
        Layout.preferredWidth: 100
        Layout.fillHeight: true
        Rectangle { anchors.fill: parent; radius: 10; color: zonaCol.containsMouse ? Tema.realce : "transparent" }
        Column {
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            Text { anchors.right: parent.right; text: col.total; color: col.activa ? Tema.azul : Tema.texto; font.pixelSize: 15; font.weight: Font.DemiBold; font.features: { "tnum": 1 } }
            Row {
                anchors.right: parent.right
                spacing: 4
                Icono { visible: col.activa; width: 11; height: 11; nombre: "derecha"; rotation: 90; color: Tema.azul; anchors.verticalCenter: parent.verticalCenter }
                Text { text: col.texto; color: col.activa ? Tema.azul : Tema.tenue; font.pixelSize: 11; font.weight: Font.DemiBold }
            }
        }
        MouseArea { id: zonaCol; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: tm.orden = col.orden }
    }

    component Celda: Rectangle {
        property string texto: ""
        property real fraccion: 0
        property bool fuerte: false
        Layout.fillHeight: true
        color: pagina.calor(fraccion)
        Behavior on color { ColorAnimation { duration: 300 } }
        Text {
            anchors.right: parent.right
            anchors.rightMargin: 12
            anchors.verticalCenter: parent.verticalCenter
            text: parent.texto
            color: parent.fraccion > 0.6 ? Tema.texto : Tema.suave
            font.pixelSize: 13
            font.weight: parent.fuerte ? Font.DemiBold : Font.Normal
            font.features: { "tnum": 1 }
        }
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        anchors.topMargin: 20
        spacing: 16

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: false
            spacing: 12
            Text { text: "Procesos"; color: Tema.texto; font.pixelSize: 26; font.weight: Font.Bold }
            Item { Layout.fillWidth: true }
            Campo {
                id: buscador
                Layout.preferredWidth: 300
                placeholderText: "Buscar por nombre o PID  (Ctrl+F)"
                onTextChanged: tm.filtro = text
            }
            Boton {
                tipo: "peligro"
                icono: "detener"
                text: "Finalizar tarea"
                enabled: pagina.seleccion !== ""
                onClicked: pagina.finalizar(false)
            }
        }

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: false
            spacing: 12
            Resumen {
                readonly property bool duro: (pagina.res.cpu || 0) > 85 || (pagina.res.memUsada || 0) / pagina.memTotal > 0.9
                icono: duro ? "alerta" : "visto"
                tinta: duro ? Tema.peligro : Tema.exito
                titulo: "Tu PC"
                valor: duro ? "Está trabajando duro" : "Va bien"
                detalle: "CPU " + Fmt.pct(pagina.res.cpu || 0) + " · RAM " + Fmt.bytes(pagina.res.memUsada || 0)
                         + " de " + Fmt.bytes(pagina.res.memTotal || 0)
            }
            Resumen {
                icono: "cpu"
                iconoApp: pagina.res.topCpu ? pagina.res.topCpu.icono || "" : ""
                titulo: "Lo que más usa CPU"
                valor: pagina.res.topCpu ? pagina.res.topCpu.nombre || "—" : "—"
                detalle: pagina.res.topCpu && pagina.res.topCpu.nombre ? Fmt.pct(pagina.res.topCpu.valor) + " de CPU" : ""
            }
            Resumen {
                icono: "memoria"
                tinta: Tema.violeta
                iconoApp: pagina.res.topMem ? pagina.res.topMem.icono || "" : ""
                titulo: "Lo que más usa RAM"
                valor: pagina.res.topMem ? pagina.res.topMem.nombre || "—" : "—"
                detalle: pagina.res.topMem && pagina.res.topMem.nombre ? Fmt.bytes(pagina.res.topMem.valor) + " de RAM" : ""
            }
        }

        // --- tabla ---
        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            clip: true

            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 6
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.fillHeight: false
                    Layout.preferredHeight: 52
                    spacing: 0
                    Item {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        Rectangle { anchors.fill: parent; radius: 10; color: zonaNombre.containsMouse ? Tema.realce : "transparent" }
                        Row {
                            anchors.left: parent.left
                            anchors.leftMargin: 16
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 4
                            Text { text: "Nombre"; color: tm.orden === "nombre" ? Tema.azul : Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold }
                            Icono { visible: tm.orden === "nombre"; width: 11; height: 11; nombre: "derecha"; rotation: 90; color: Tema.azul; anchors.verticalCenter: parent.verticalCenter }
                        }
                        MouseArea { id: zonaNombre; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: tm.orden = "nombre" }
                    }
                    Columna { Layout.preferredWidth: pagina.anchoCpu; texto: "CPU"; orden: "cpu"; total: Fmt.pct(tm.totales.cpu || 0) }
                    Columna { Layout.preferredWidth: pagina.anchoMem; texto: "Memoria"; orden: "mem"; total: Fmt.pct(tm.totales.mem || 0) }
                    Columna { Layout.preferredWidth: pagina.anchoDisco; texto: "Disco"; orden: "disco"; total: Fmt.mbs(tm.totales.disco || 0) }
                    Item { Layout.preferredWidth: 10 }
                }
                Rectangle { Layout.fillWidth: true; implicitHeight: 1; color: Tema.borde }

                ListView {
                    id: lista
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    model: tm.filas
                    boundsBehavior: Flickable.StopAtBounds
                    reuseItems: true
                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AsNeeded
                        contentItem: Rectangle { implicitWidth: 6; radius: 3; color: Tema.bordeCampo }
                    }

                    Text {
                        anchors.centerIn: parent
                        visible: lista.count === 0
                        text: tm.filtro ? "Nada coincide con «" + tm.filtro + "»" : "Leyendo procesos…"
                        color: Tema.tenue
                        font.pixelSize: 14
                    }

                    delegate: Item {
                        id: fila
                        required property var model
                        readonly property var f: model.fila
                        readonly property bool esSeccion: f.tipo === "seccion"
                        readonly property bool esProceso: f.tipo === "proceso"
                        readonly property bool elegida: f.clave === pagina.seleccion
                        width: ListView.view.width - 10
                        height: esSeccion ? 44 : 40

                        Rectangle {
                            anchors.fill: parent
                            anchors.topMargin: 1
                            anchors.bottomMargin: 1
                            radius: 10
                            visible: !fila.esSeccion
                            color: fila.elegida ? Tema.realceFuerte : zona.containsMouse ? Tema.realce : "transparent"
                            border.width: fila.elegida ? 1 : 0
                            border.color: Tema.bordeHover
                        }

                        // Encabezado de sección: «Apps (6)»
                        RowLayout {
                            visible: fila.esSeccion
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            spacing: 8
                            Icono {
                                Layout.preferredWidth: 14; Layout.preferredHeight: 14
                                nombre: "derecha"
                                rotation: fila.f.colapsada ? 0 : 90
                                color: Tema.tenue
                                Behavior on rotation { NumberAnimation { duration: 140 } }
                            }
                            Text {
                                text: fila.f.nombre + "  ·  " + fila.f.cantidad
                                color: Tema.suave
                                font.pixelSize: 12
                                font.weight: Font.DemiBold
                                font.letterSpacing: 0.6
                                font.capitalization: Font.AllUppercase
                            }
                            Rectangle { Layout.fillWidth: true; Layout.leftMargin: 8; Layout.rightMargin: 14; implicitHeight: 1; color: Tema.borde }
                        }

                        RowLayout {
                            visible: !fila.esSeccion
                            anchors.fill: parent
                            spacing: 0
                            Item { Layout.preferredWidth: fila.esProceso ? 52 : 8 }
                            Item {   // flechita para abrir la app y ver sus procesos
                                Layout.preferredWidth: 26
                                Layout.fillHeight: true
                                visible: !fila.esProceso
                                Icono {
                                    anchors.centerIn: parent
                                    visible: (fila.f.n || 0) > 1
                                    width: 14; height: 14
                                    nombre: "derecha"
                                    rotation: fila.f.expandido ? 90 : 0
                                    color: zonaFlecha.containsMouse ? Tema.texto : Tema.tenue
                                    Behavior on rotation { NumberAnimation { duration: 140 } }
                                }
                                MouseArea {
                                    id: zonaFlecha
                                    anchors.fill: parent
                                    enabled: (fila.f.n || 0) > 1
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: tm.alternarGrupo(fila.f.clave)
                                }
                            }
                            Item {
                                Layout.preferredWidth: fila.esProceso ? 18 : 30
                                Layout.preferredHeight: 24
                                Image {
                                    id: imagen
                                    anchors.centerIn: parent
                                    visible: !fila.esProceso && (fila.f.icono || "") !== ""
                                    width: 22; height: 22
                                    sourceSize.width: 44; sourceSize.height: 44
                                    asynchronous: true
                                    source: visible ? "image://icono/" + fila.f.icono : ""
                                }
                                Icono {
                                    anchors.centerIn: parent
                                    visible: !fila.esProceso && !imagen.visible
                                    width: 19; height: 19
                                    nombre: fila.f.seccion === "apps" ? "app" : fila.f.seccion === "sistema" ? "escudo" : "fondo"
                                    color: Tema.tenue
                                }
                                Rectangle {
                                    visible: fila.esProceso
                                    anchors.centerIn: parent
                                    width: 6; height: 6; radius: 3
                                    color: Tema.bordeCampo
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                Layout.leftMargin: 8
                                spacing: 8
                                Text {
                                    text: fila.f.nombre + ((fila.f.n || 0) > 1 ? "  (" + fila.f.n + ")" : "")
                                    color: Tema.texto
                                    font.pixelSize: 13
                                    font.weight: fila.esProceso ? Font.Normal : Font.Medium
                                    elide: Text.ElideRight
                                    Layout.maximumWidth: fila.width * 0.4
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: fila.esProceso ? "PID " + fila.f.pid + (fila.f.usuario && fila.f.seccion === "sistema" ? " · " + fila.f.usuario : "")
                                                         : fila.f.subtitulo || ""
                                    color: Tema.tenue
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                }
                                Rectangle {
                                    visible: !!fila.f.suspendido
                                    implicitWidth: textoSusp.implicitWidth + 16
                                    implicitHeight: 20
                                    radius: 10
                                    color: Qt.rgba(Tema.violeta.r, Tema.violeta.g, Tema.violeta.b, 0.16)
                                    Text { id: textoSusp; anchors.centerIn: parent; text: "En pausa"; color: Tema.violeta; font.pixelSize: 11; font.weight: Font.DemiBold }
                                }
                            }
                            Celda { Layout.preferredWidth: pagina.anchoCpu; texto: Fmt.pct(fila.f.cpu || 0); fraccion: (fila.f.cpu || 0) / 40; fuerte: !fila.esProceso }
                            Celda { Layout.preferredWidth: pagina.anchoMem; texto: Fmt.bytes(fila.f.mem || 0); fraccion: (fila.f.mem || 0) / (pagina.memTotal * 0.2); fuerte: !fila.esProceso }
                            Celda { Layout.preferredWidth: pagina.anchoDisco; texto: Fmt.mbs(fila.f.disco || 0); fraccion: (fila.f.disco || 0) / 52428800; fuerte: !fila.esProceso }
                        }

                        MouseArea {
                            id: zona
                            anchors.fill: parent
                            anchors.leftMargin: fila.esSeccion ? 0 : 34
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: (ev) => {
                                if (fila.esSeccion) { tm.alternarSeccion(fila.f.seccion); return }
                                pagina.seleccion = fila.f.clave
                                if (ev.button === Qt.RightButton) menu.abrir(fila.f.clave)
                            }
                            onDoubleClicked: if (!fila.esSeccion && (fila.f.n || 0) > 1) tm.alternarGrupo(fila.f.clave)
                        }
                    }
                }
            }
        }
    }

    MenuVortex {
        id: menu
        property string clave: ""
        property var info: ({})
        function abrir(c) {
            clave = c
            info = tm.detalles(c)
            if (info.pid !== undefined) popup()
        }
        OpcionMenu { text: "Cerrar ventana"; icono: "ventana"; visible: !!menu.info.ventana; onTriggered: tm.cerrarVentana(menu.clave) }
        OpcionMenu { text: "Finalizar tarea"; icono: "detener"; atajo: "Supr"; onTriggered: { tm.finalizar(menu.clave, false); pagina.seleccion = "" } }
        OpcionMenu { text: "Forzar cierre"; icono: "rayo"; atajo: "Mayús+Supr"; peligro: true; onTriggered: { tm.finalizar(menu.clave, true); pagina.seleccion = "" } }
        MenuSeparator { contentItem: Rectangle { implicitHeight: 1; color: Tema.borde } }
        OpcionMenu {
            text: menu.info.suspendido ? "Reanudar" : "Poner en pausa"
            icono: menu.info.suspendido ? "play" : "pausa"
            onTriggered: tm.suspender(menu.clave, !menu.info.suspendido)
        }
        MenuVortex {
            title: "Prioridad"
            OpcionMenu { text: "Alta"; checkable: true; checked: menu.info.prioridad === "alta"; onTriggered: tm.ponerPrioridad(menu.clave, "alta") }
            OpcionMenu { text: "Normal"; checkable: true; checked: menu.info.prioridad === "normal"; onTriggered: tm.ponerPrioridad(menu.clave, "normal") }
            OpcionMenu { text: "Baja"; checkable: true; checked: menu.info.prioridad === "baja"; onTriggered: tm.ponerPrioridad(menu.clave, "baja") }
            OpcionMenu { text: "Mínima"; checkable: true; checked: menu.info.prioridad === "minima"; onTriggered: tm.ponerPrioridad(menu.clave, "minima") }
        }
        MenuSeparator { contentItem: Rectangle { implicitHeight: 1; color: Tema.borde } }
        OpcionMenu { text: "Abrir ubicación del archivo"; icono: "carpeta"; enabled: !!menu.info.ubicacion; onTriggered: tm.abrirUbicacion(menu.clave) }
        OpcionMenu { text: "Copiar comando"; icono: "copiar"; enabled: !!menu.info.comando; onTriggered: tm.copiar(menu.info.comando) }
        OpcionMenu { text: "Copiar PID  " + (menu.info.pid || ""); icono: "info"; onTriggered: tm.copiar(String(menu.info.pid)) }
    }
}
