// La biblioteca o una playlist: encabezado con «Reproducir» y «Aleatorio», búsqueda, orden y la lista.
// Doble clic toca desde esa canción; clic derecho: agregar a una playlist, quitar, abrir su carpeta.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: raiz
    signal irAAgregar()
    signal verResumen()
    signal buscarEnInternet(string texto)
    readonly property var playlist: {
        for (var i = 0; i < vp.playlists.length; i++)
            if (vp.playlists[i].id === vp.vistaPlaylist) return vp.playlists[i]
        return null
    }
    property bool renombrando: false
    // 2.4: el encabezado toma el color de la portada (de la playlist o de la primera canción), como Spotify
    property string colorPortada: ""
    readonly property string rutaColor: raiz.playlist && raiz.playlist.portadas && raiz.playlist.portadas.length
        ? raiz.playlist.portadas[0] : vp.canciones.length ? vp.canciones[0].ruta : ""
    onRutaColorChanged: colorPortada = rutaColor ? vp.colorDe(rutaColor) : ""
    Connections {
        target: vp
        function onPortadaLista(r) { if (r === raiz.rutaColor) raiz.colorPortada = vp.colorDe(r) }
    }
    readonly property bool sonandoAqui: !rep.pausado && rep.origen === (raiz.playlist ? raiz.playlist.nombre : "Tu biblioteca")

    function tiempo(s) {
        s = Math.max(0, Math.round(s || 0))
        var m = Math.floor(s / 60), r = s % 60
        return m + ":" + (r < 10 ? "0" : "") + r
    }

    function duracionTotal() {
        var t = 0
        for (var i = 0; i < vp.canciones.length; i++) t += vp.canciones[i].duracion || 0
        var h = Math.floor(t / 3600), m = Math.round((t % 3600) / 60)
        return h > 0 ? h + " h " + m + " min" : m + " min"
    }

    function cuantasHiRes() {
        var n = 0
        for (var i = 0; i < vp.canciones.length; i++) n += vp.canciones[i].nivel === "hires"
        return n
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        // --- encabezado ---
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: 210
            readonly property color tono: raiz.colorPortada ? raiz.colorPortada : Qt.rgba(Tema.azul.r, Tema.azul.g, Tema.azul.b, 0.22)
            gradient: Gradient {
                GradientStop { position: 0; color: parent.tono; Behavior on color { ColorAnimation { duration: 500 } } }
                GradientStop { position: 1; color: "transparent" }
            }
            RowLayout {
                anchors.fill: parent
                anchors.margins: 28
                spacing: 22
                Mosaico {   // 2.4: la portada de la playlist: sus primeras 4 canciones
                    visible: !!raiz.playlist
                    Layout.preferredWidth: 154
                    Layout.preferredHeight: 154
                    radio: 6
                    rutas: raiz.playlist ? raiz.playlist.portadas || [] : []
                    meGusta: !!raiz.playlist && !!raiz.playlist.meGusta
                }
                Rectangle {
                    visible: !raiz.playlist
                    Layout.preferredWidth: 154
                    Layout.preferredHeight: 154
                    radius: 14
                    gradient: Gradient {
                        orientation: Gradient.Horizontal
                        GradientStop { position: 0; color: Tema.azul }
                        GradientStop { position: 1; color: Tema.violeta }
                    }
                    Icono {
                        anchors.centerIn: parent
                        width: 56; height: 56
                        nombre: raiz.playlist ? "playlist" : "musica"
                        color: Tema.sobreAcento
                        trazo: 1.6
                    }
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 6
                    Text {
                        text: raiz.playlist ? (raiz.playlist.url ? "Playlist importada" : "Playlist") : "Biblioteca"
                        color: Tema.suave
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                        font.capitalization: Font.AllUppercase
                        font.letterSpacing: 1
                    }
                    Text {
                        visible: !raiz.renombrando
                        Layout.fillWidth: true
                        text: raiz.playlist ? raiz.playlist.nombre : "Tus canciones"
                        color: Tema.texto
                        font.pixelSize: 38
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                        MouseArea {
                            anchors.fill: parent
                            enabled: !!raiz.playlist
                            onDoubleClicked: { raiz.renombrando = true; campoNombre.text = raiz.playlist.nombre; campoNombre.forceActiveFocus(); campoNombre.selectAll() }
                        }
                    }
                    TextField {
                        id: campoNombre
                        visible: raiz.renombrando
                        Layout.fillWidth: true
                        font.pixelSize: 30
                        font.weight: Font.Bold
                        color: Tema.texto
                        background: Rectangle { radius: 10; color: Tema.campo; border.color: Tema.azul; border.width: 1 }
                        onAccepted: { vp.renombrarPlaylist(raiz.playlist.id, text); raiz.renombrando = false }
                        onActiveFocusChanged: if (!activeFocus && raiz.renombrando) { vp.renombrarPlaylist(raiz.playlist.id, text); raiz.renombrando = false }
                        Keys.onEscapePressed: raiz.renombrando = false
                    }
                    Text {
                        text: vp.canciones.length + (vp.canciones.length === 1 ? " canción" : " canciones")
                              + (vp.canciones.length ? " · " + raiz.duracionTotal() : "")
                              + (raiz.cuantasHiRes() ? " · " + raiz.cuantasHiRes() + " en Hi-Res" : "")
                              + (vp.escaneando ? " · buscando música nueva…" : "")
                        color: Tema.tenue
                        font.pixelSize: 13
                    }
                    RowLayout {
                        Layout.topMargin: 8
                        spacing: 10
                        BotonIcono {   // 2.4: el botón redondo de Spotify (del color de acento)
                            relleno: true; acento: true; tam: 52
                            icono: raiz.sonandoAqui ? "pausa" : "play"
                            ayuda: raiz.sonandoAqui ? "Pausa" : "Reproducir"
                            enabled: vp.canciones.length > 0
                            onClicked: {
                                if (rep.origen === (raiz.playlist ? raiz.playlist.nombre : "Tu biblioteca") && rep.cancion.ruta) rep.alternar()
                                else { if (rep.aleatorio) rep.alternarAleatorio(); vp.tocar(0) }
                            }
                        }
                        Boton { text: "Aleatorio"; icono: "aleatorio"; tipo: "secundario"; enabled: vp.canciones.length > 0; onClicked: vp.tocarAleatorio() }
                        Boton {
                            visible: !!raiz.playlist && !raiz.playlist.meGusta
                            text: "Renombrar"; icono: "editar"; tipo: "fantasma"
                            onClicked: { raiz.renombrando = true; campoNombre.text = raiz.playlist.nombre; campoNombre.forceActiveFocus(); campoNombre.selectAll() }
                        }
                        Boton {
                            visible: !!raiz.playlist && !raiz.playlist.meGusta
                            text: "Borrar playlist"; icono: "papelera"; tipo: "peligro"
                            onClicked: confirmarBorrar.open()
                        }
                    }
                }
            }
        }

        // 1.4: el aviso de «Tu resumen» de la semana pasada, una vez por semana (siempre está en Ajustes)
        Rectangle {
            id: avisoResumen
            readonly property var pasada: vp.resumen.pasada || ({})
            readonly property var mejor: pasada.canciones && pasada.canciones.length ? pasada.canciones[0] : null
            visible: !raiz.playlist && !!vp.resumen.aviso
            Layout.fillWidth: true
            Layout.leftMargin: 28
            Layout.rightMargin: 28
            Layout.bottomMargin: 14
            implicitHeight: filaResumen.implicitHeight + 28
            radius: Tema.radioTarjeta
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: Qt.rgba(Tema.azul.r, Tema.azul.g, Tema.azul.b, 0.30) }
                GradientStop { position: 1; color: Qt.rgba(Tema.violeta.r, Tema.violeta.g, Tema.violeta.b, 0.16) }
            }
            RowLayout {
                id: filaResumen
                anchors.fill: parent
                anchors.margins: 14
                anchors.leftMargin: 18
                spacing: 14
                Portada {
                    visible: !!avisoResumen.mejor
                    Layout.preferredWidth: 52; Layout.preferredHeight: 52
                    ruta: avisoResumen.mejor ? avisoResumen.mejor.ruta : ""
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 3
                    Text { text: "Tu semana en música"; color: Tema.texto; font.pixelSize: 15; font.weight: Font.Bold }
                    Text {
                        Layout.fillWidth: true
                        text: Tema.minutos(avisoResumen.pasada.minutos) + " escuchando"
                              + (avisoResumen.mejor ? " · la que más sonó: «" + avisoResumen.mejor.titulo + "»"
                                 + (avisoResumen.mejor.artista ? " de " + avisoResumen.mejor.artista : "") : "")
                        color: Tema.suave
                        font.pixelSize: 13
                        elide: Text.ElideRight
                    }
                }
                Boton { text: "Ver tu resumen"; icono: "resumen"; onClicked: { vp.resumenVisto(); raiz.verResumen() } }
                BotonIcono { icono: "cerrar"; tam: 30; ayuda: "Cerrar"; onClicked: vp.resumenVisto() }
            }
        }

        // --- búsqueda y orden ---
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 28
            Layout.rightMargin: 28
            Layout.bottomMargin: 8
            spacing: 10
            TextField {
                id: busqueda
                Layout.preferredWidth: 320
                placeholderText: "Buscar canción, artista o álbum"
                placeholderTextColor: Tema.tenue
                color: Tema.texto
                leftPadding: 38
                onTextChanged: buscarTimer.restart()
                background: Rectangle {
                    radius: 18
                    color: Tema.campo
                    border.width: busqueda.activeFocus ? 1 : 0
                    border.color: Tema.azul
                    Icono { x: 12; anchors.verticalCenter: parent.verticalCenter; width: 17; height: 17; nombre: "buscar"; color: Tema.tenue }
                }
                Timer { id: buscarTimer; interval: 180; onTriggered: vp.buscar(busqueda.text) }
            }
            // 2.1: lo que no tienes, se busca en YouTube Music (página «Agregar música»)
            Boton {
                visible: busqueda.text.trim() !== ""
                text: "Buscar en YouTube Music"
                icono: "descargar"
                tipo: "fantasma"
                onClicked: raiz.buscarEnInternet(busqueda.text)
            }
            Item { Layout.fillWidth: true }
            Text { visible: !raiz.playlist; text: "Ordenar:"; color: Tema.tenue; font.pixelSize: 12 }
            Selector {
                id: orden
                visible: !raiz.playlist
                Layout.preferredWidth: 170
                model: [{ t: "Recientes", v: "agregada" }, { t: "Título", v: "titulo" }, { t: "Artista", v: "artista" }, { t: "Mejor calidad", v: "calidad" }]
                textRole: "t"
                valueRole: "v"
                onActivated: vp.ordenar(currentValue)
            }
        }

        // --- encabezados de columnas ---
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 28
            Layout.rightMargin: 28
            spacing: 14
            visible: vp.canciones.length > 0
            Text { Layout.preferredWidth: 28; text: "#"; color: Tema.tenue; font.pixelSize: 12; horizontalAlignment: Text.AlignRight }
            Text { Layout.fillWidth: true; Layout.leftMargin: 52; text: "Título"; color: Tema.tenue; font.pixelSize: 12 }
            Text { Layout.preferredWidth: lista.width * 0.24; text: "Álbum"; color: Tema.tenue; font.pixelSize: 12 }
            Text { Layout.preferredWidth: 120; text: "Calidad"; color: Tema.tenue; font.pixelSize: 12 }
            Text { Layout.preferredWidth: 48; text: "Duración"; color: Tema.tenue; font.pixelSize: 12; horizontalAlignment: Text.AlignRight }
        }
        Rectangle { Layout.fillWidth: true; Layout.leftMargin: 28; Layout.rightMargin: 28; Layout.topMargin: 6; height: 1; color: Tema.borde; visible: vp.canciones.length > 0 }

        // --- la lista ---
        ListView {
            id: lista
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 16
            Layout.rightMargin: 16
            Layout.topMargin: 6
            clip: true
            model: vp.canciones
            boundsBehavior: Flickable.StopAtBounds
            reuseItems: true
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

            // 2.1: abajo de una playlist, canciones parecidas de YouTube Music (salen solas); «Agregar» la baja
            // a tu biblioteca y a esta playlist
            footer: ColumnLayout {
                id: recomendadas
                readonly property bool aqui: !!raiz.playlist && vp.recomendandoPara === raiz.playlist.id
                                             && vp.canciones.length > 0 && !busqueda.text
                visible: aqui
                width: lista.width - 12
                height: aqui ? implicitHeight : 0
                spacing: 4
                RowLayout {
                    Layout.fillWidth: true
                    Layout.topMargin: 28
                    Layout.leftMargin: 12
                    Layout.bottomMargin: 6
                    spacing: 10
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text { text: "Recomendadas para esta playlist"; color: Tema.texto; font.pixelSize: 18; font.weight: Font.Bold }
                        Text {
                            text: vp.cargandoRecomendaciones ? "Buscando canciones parecidas en YouTube Music…"
                                : vp.errorRecomendaciones ? vp.errorRecomendaciones
                                : "Parecidas a las que tienes aquí, de YouTube Music"
                            color: vp.errorRecomendaciones && !vp.cargandoRecomendaciones ? Tema.peligro : Tema.tenue
                            font.pixelSize: 12
                        }
                    }
                    Boton {
                        text: "Otras"
                        icono: "girar"
                        tipo: "fantasma"
                        enabled: !vp.cargandoRecomendaciones
                        onClicked: vp.recomendar(raiz.playlist.id, true)
                    }
                }
                Repeater {
                    model: recomendadas.aqui ? vp.recomendaciones : []
                    delegate: Rectangle {
                        id: reco
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        implicitHeight: 58
                        radius: 8
                        color: zonaReco.hovered || botonAvance.mio ? Tema.tarjetaHover : "transparent"
                        HoverHandler { id: zonaReco }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 14
                            // 2.1.1: ▶ escucha 30 s antes de agregarla (tu música se pausa y luego sigue)
                            Item {
                                Layout.preferredWidth: 28
                                Layout.fillHeight: true
                                BotonAvance {
                                    id: botonAvance
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    anchors.rightMargin: -6
                                    vid: reco.modelData.id
                                    onClicked: vp.escucharRecomendada(reco.index)
                                }
                            }
                            Portada {
                                Layout.preferredWidth: 42
                                Layout.preferredHeight: 42
                                url: reco.modelData.portada
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text { Layout.fillWidth: true; text: reco.modelData.titulo; color: botonAvance.mio ? Tema.azul : Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideRight }
                                Text { Layout.fillWidth: true; text: reco.modelData.artistas; color: Tema.suave; font.pixelSize: 12; elide: Text.ElideRight }
                            }
                            Text {
                                Layout.preferredWidth: lista.width * 0.24
                                text: reco.modelData.album
                                color: Tema.suave
                                font.pixelSize: 13
                                elide: Text.ElideRight
                            }
                            Text {
                                Layout.preferredWidth: 48
                                text: reco.modelData.duracion ? raiz.tiempo(reco.modelData.duracion) : ""
                                color: Tema.tenue
                                font.pixelSize: 13
                                font.features: { "tnum": 1 }
                                horizontalAlignment: Text.AlignRight
                            }
                            Item {
                                Layout.preferredWidth: 120
                                Layout.preferredHeight: 34
                                Boton {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: reco.modelData.estado === ""
                                    text: "Agregar"
                                    icono: "mas"
                                    tipo: "secundario"
                                    enabled: vp.ajustes.hayYtDlp
                                    onClicked: vp.agregarRecomendada(reco.index)
                                }
                                Text {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    visible: reco.modelData.estado !== ""
                                    text: reco.modelData.estado === "biblioteca" ? "✓ Agregada" : "Bajando…"
                                    color: reco.modelData.estado === "biblioteca" ? Tema.exito : Tema.suave
                                    font.pixelSize: 12
                                }
                            }
                        }
                    }
                }
                Item { Layout.preferredHeight: 20 }
            }

            delegate: Rectangle {
                id: fila
                required property var modelData
                required property int index
                readonly property bool sonando: rep.cancion.ruta === modelData.ruta
                width: lista.width - 12
                height: 58
                radius: 8
                color: zona.containsMouse ? Tema.tarjetaHover : "transparent"

                MouseArea {
                    id: zona
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onDoubleClicked: (e) => { if (e.button === Qt.LeftButton) vp.tocar(fila.index) }
                    onClicked: (e) => { if (e.button === Qt.RightButton) { menu.cancion = fila.modelData; menu.popup() } }
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 12
                    anchors.rightMargin: 12
                    spacing: 14
                    Item {
                        Layout.preferredWidth: 28
                        Layout.fillHeight: true
                        Text {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            visible: !zona.containsMouse && !fila.sonando
                            text: fila.index + 1
                            color: Tema.tenue
                            font.pixelSize: 13
                            font.features: { "tnum": 1 }
                        }
                        BotonIcono {
                            anchors.right: parent.right
                            anchors.verticalCenter: parent.verticalCenter
                            anchors.rightMargin: -6
                            visible: zona.containsMouse || fila.sonando
                            tam: 30
                            icono: fila.sonando && !rep.pausado ? "pausa" : "play"
                            activo: fila.sonando
                            onClicked: fila.sonando ? rep.alternar() : vp.tocar(fila.index)
                        }
                    }
                    Portada {
                        Layout.preferredWidth: 42
                        Layout.preferredHeight: 42
                        ruta: fila.modelData.ruta
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Text {
                            Layout.fillWidth: true
                            text: fila.modelData.titulo
                            color: fila.sonando ? Tema.azul : Tema.texto
                            font.pixelSize: 14
                            font.weight: Font.Medium
                            elide: Text.ElideRight
                        }
                        RowLayout {
                            spacing: 6
                            Layout.fillWidth: true
                            Rectangle {
                                visible: fila.modelData.origen === "youtube"
                                implicitWidth: yt.implicitWidth + 8
                                implicitHeight: 15
                                radius: 3
                                color: Tema.realce
                                Text { id: yt; anchors.centerIn: parent; text: "YT"; color: Tema.tenue; font.pixelSize: 9; font.weight: Font.Bold }
                            }
                            Text {
                                Layout.fillWidth: true
                                text: fila.modelData.artista || "Artista desconocido"
                                color: Tema.tenue
                                font.pixelSize: 12
                                elide: Text.ElideRight
                            }
                        }
                    }
                    BotonIcono {   // 2.4: ❤️ al pasar el mouse, o siempre si ya te gusta
                        readonly property bool gusta: vp.meGusta.indexOf(fila.modelData.ruta) >= 0
                        opacity: gusta || zona.containsMouse || hovered ? 1 : 0
                        tam: 30
                        icono: gusta ? "corazonLleno" : "corazon"
                        activo: gusta
                        ayuda: gusta ? "Quitar de Tus me gusta" : "Guardar en Tus me gusta"
                        onClicked: vp.alternarMeGusta(fila.modelData.ruta)
                    }
                    Text {
                        Layout.preferredWidth: lista.width * 0.24
                        text: fila.modelData.album || ""
                        color: Tema.tenue
                        font.pixelSize: 13
                        elide: Text.ElideRight
                    }
                    Item {
                        Layout.preferredWidth: 120
                        Layout.fillHeight: true
                        Etiqueta {
                            anchors.verticalCenter: parent.verticalCenter
                            texto: fila.modelData.etiqueta
                            nivel: fila.modelData.nivel
                        }
                    }
                    Text {
                        Layout.preferredWidth: 48
                        text: raiz.tiempo(fila.modelData.duracion)
                        color: Tema.tenue
                        font.pixelSize: 13
                        font.features: { "tnum": 1 }
                        horizontalAlignment: Text.AlignRight
                    }
                }
            }

            // --- vacío ---
            ColumnLayout {
                anchors.centerIn: parent
                visible: lista.count === 0
                spacing: 12
                width: Math.min(440, parent.width - 40)
                Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 46; Layout.preferredHeight: 46; nombre: busqueda.text ? "buscar" : "musica"; color: Tema.tenue; trazo: 1.5 }
                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: busqueda.text ? "Nada coincide con «" + busqueda.text + "»"
                        : raiz.playlist ? "Esta playlist está vacía" : "Todavía no hay música"
                    color: Tema.texto
                    font.pixelSize: 18
                    font.weight: Font.DemiBold
                    wrapMode: Text.WordWrap
                }
                Text {
                    Layout.fillWidth: true
                    visible: !busqueda.text
                    horizontalAlignment: Text.AlignHCenter
                    text: raiz.playlist ? "Clic derecho en una canción de tu biblioteca → «Agregar a la playlist»."
                        : "Pega un enlace de YouTube Music, YouTube o Spotify, o agrega tus archivos FLAC y WAV. También puedes arrastrarlos a la ventana."
                    color: Tema.tenue
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Boton {
                    visible: !busqueda.text && !raiz.playlist
                    Layout.alignment: Qt.AlignHCenter
                    Layout.topMargin: 6
                    text: "Agregar música"
                    icono: "descargar"
                    onClicked: raiz.irAAgregar()
                }
            }
        }
    }

    Menu {
        id: menu
        property var cancion: ({})
        // 2.4: ❤️ y la cola «A continuación»
        MenuItem {
            text: vp.meGusta.indexOf(menu.cancion.ruta || "") >= 0 ? "Quitar de Tus me gusta" : "Guardar en Tus me gusta"
            onTriggered: vp.alternarMeGusta(menu.cancion.ruta)
        }
        MenuItem { text: "Tocar después"; onTriggered: vp.tocarDespues(menu.cancion) }
        MenuItem { text: "Agregar a la cola"; onTriggered: vp.agregarACola(menu.cancion) }
        MenuSeparator {}
        Menu {
            id: submenu
            title: "Agregar a la playlist"
            Instantiator {
                model: vp.playlists
                delegate: MenuItem {
                    required property var modelData
                    text: modelData.nombre
                    onTriggered: vp.agregarAPlaylist(modelData.id, menu.cancion.ruta)
                }
                onObjectAdded: (i, o) => submenu.insertItem(i, o)
                onObjectRemoved: (i, o) => submenu.removeItem(o)
            }
            MenuSeparator {}
            MenuItem {
                text: "Nueva playlist…"
                onTriggered: { var id = vp.crearPlaylist("Nueva playlist"); vp.agregarAPlaylist(id, menu.cancion.ruta) }
            }
        }
        MenuItem {
            visible: !!raiz.playlist
            height: visible ? implicitHeight : 0
            text: "Quitar de esta playlist"
            onTriggered: vp.quitarDePlaylist(raiz.playlist.id, menu.cancion.ruta)
        }
        MenuItem { text: "Abrir su carpeta"; onTriggered: vp.abrirCarpeta(menu.cancion.ruta) }
        MenuSeparator {}
        MenuItem {
            text: menu.cancion.origen === "youtube" ? "Borrar (va a la papelera)" : "Quitar de la biblioteca"
            onTriggered: vp.quitarDeBiblioteca(menu.cancion.ruta)
        }
    }

    Dialog {
        id: confirmarBorrar
        anchors.centerIn: parent
        modal: true
        title: "¿Borrar «" + (raiz.playlist ? raiz.playlist.nombre : "") + "»?"
        standardButtons: Dialog.Yes | Dialog.No
        Label { text: "Las canciones se quedan en tu biblioteca."; color: Tema.suave }
        onAccepted: vp.borrarPlaylist(raiz.playlist.id)
    }
}
