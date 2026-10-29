// Agregar música: buscar en YouTube Music o YouTube (2.1), pegar un enlace (YouTube, YouTube Music,
// Spotify) para bajarlo, o agregar tus archivos sin pérdida. Abajo, lo que se está bajando.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

Flickable {
    id: raiz
    contentHeight: columna.implicitHeight + 56
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    function bajar() {
        if (enlace.text.trim() === "") return
        vp.bajar(enlace.text)
        enlace.text = ""
    }
    // Desde el buscador de tu biblioteca («Buscar en YouTube Music») o al dar Enter aquí
    function buscar(texto) {
        if (texto !== undefined) campoBuscar.text = texto
        vp.buscarEnLinea(campoBuscar.text, fuente.currentIndex === 0 ? "ytmusic" : "youtube")
        raiz.contentY = 0
    }
    function tiempo(s) {
        if (!s) return ""
        var m = Math.floor(s / 60), x = s % 60
        return m + ":" + (x < 10 ? "0" : "") + x
    }

    ColumnLayout {
        id: columna
        x: 28
        y: 28
        width: Math.min(raiz.width - 56, 860)
        spacing: 18

        Text { text: "Agregar música"; color: Tema.texto; font.pixelSize: 32; font.weight: Font.Bold }

        // --- buscar (2.1) ---
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: cajaBuscar.implicitHeight + 40
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            ColumnLayout {
                id: cajaBuscar
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12
                Text { text: "Buscar canciones"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold }
                Text {
                    Layout.fillWidth: true
                    text: "Escribe una canción o un artista. Lo que bajes queda en tu biblioteca para armar tus playlists."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10
                    TextField {
                        id: campoBuscar
                        Layout.fillWidth: true
                        implicitHeight: 44
                        placeholderText: "Canción o artista"
                        placeholderTextColor: Tema.tenue
                        color: Tema.texto
                        leftPadding: 42
                        selectByMouse: true
                        onAccepted: raiz.buscar()
                        background: Rectangle {
                            radius: 22
                            color: Tema.campo
                            border.width: 1
                            border.color: campoBuscar.activeFocus ? Tema.azul : Tema.bordeCampo
                            Icono { x: 14; anchors.verticalCenter: parent.verticalCenter; width: 18; height: 18; nombre: "buscar"; color: Tema.tenue }
                        }
                    }
                    Selector {
                        id: fuente
                        Layout.preferredWidth: 170
                        implicitHeight: 44
                        model: ["YouTube Music", "YouTube"]
                        onActivated: if (campoBuscar.text.trim() !== "") raiz.buscar()
                    }
                    Boton { text: "Buscar"; icono: "buscar"; grande: true; enabled: campoBuscar.text.trim() !== "" && !vp.buscandoEnLinea; onClicked: raiz.buscar() }
                }
                Text {
                    Layout.fillWidth: true
                    visible: vp.buscandoEnLinea || vp.errorEnLinea !== ""
                    text: vp.buscandoEnLinea ? "Buscando…" : vp.errorEnLinea
                    color: vp.errorEnLinea && !vp.buscandoEnLinea ? Tema.peligro : Tema.tenue
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Repeater {
                    model: vp.enLinea
                    delegate: Rectangle {
                        id: resultado
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        implicitHeight: 60
                        radius: 12
                        color: zona.hovered || avanceResultado.mio ? Tema.campo : "transparent"
                        HoverHandler { id: zona }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 8
                            anchors.rightMargin: 8
                            spacing: 12
                            BotonAvance {      // 2.1.1: escuchar 30 s antes de bajarla
                                id: avanceResultado
                                vid: resultado.modelData.id
                                onClicked: vp.escucharResultado(resultado.index)
                            }
                            Portada {
                                Layout.preferredWidth: 46
                                Layout.preferredHeight: 46
                                url: resultado.modelData.portada
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    Layout.fillWidth: true
                                    text: resultado.modelData.titulo
                                    color: avanceResultado.mio ? Tema.azul : Tema.texto
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: [resultado.modelData.artistas, resultado.modelData.album].filter(x => x).join(" · ")
                                    color: Tema.suave
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                }
                            }
                            Text {
                                text: raiz.tiempo(resultado.modelData.duracion)
                                color: Tema.tenue
                                font.pixelSize: 12
                                font.features: { "tnum": 1 }
                            }
                            // Ya la tienes / se está bajando / bajar (o bajar directo a una playlist)
                            RowLayout {
                                visible: resultado.modelData.estado !== ""
                                spacing: 6
                                Icono {
                                    Layout.preferredWidth: 16; Layout.preferredHeight: 16
                                    nombre: resultado.modelData.estado === "biblioteca" ? "visto" : "descargar"
                                    color: resultado.modelData.estado === "biblioteca" ? Tema.exito : Tema.azul
                                }
                                Text {
                                    text: resultado.modelData.estado === "biblioteca" ? "En tu biblioteca" : "Bajando…"
                                    color: Tema.suave
                                    font.pixelSize: 12
                                }
                            }
                            Boton {
                                visible: resultado.modelData.estado === ""
                                text: "Bajar"
                                icono: "descargar"
                                tipo: "secundario"
                                enabled: vp.ajustes.hayYtDlp
                                onClicked: vp.bajarResultado(resultado.index, -1)
                            }
                            BotonIcono {
                                visible: resultado.modelData.estado === ""
                                icono: "playlist"
                                ayuda: "Bajar y agregar a una playlist"
                                enabled: vp.ajustes.hayYtDlp
                                onClicked: { menuPlaylist.indice = resultado.index; menuPlaylist.popup() }
                            }
                        }
                    }
                }
            }
        }

        // --- enlace ---
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: cajaEnlace.implicitHeight + 40
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            ColumnLayout {
                id: cajaEnlace
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12
                Text { text: "Desde YouTube Music, YouTube o Spotify"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold }
                Text {
                    Layout.fillWidth: true
                    text: "Pega el enlace de una canción, un álbum o una playlist. Se baja a tu biblioteca y queda para siempre, sin anuncios y sin internet. "
                        + "De Spotify se leen los nombres y cada canción se baja de YouTube Music. Las playlists llegan con su nombre."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10
                    TextField {
                        id: enlace
                        Layout.fillWidth: true
                        implicitHeight: 44
                        placeholderText: "https://music.youtube.com/playlist?list=…   o   https://open.spotify.com/playlist/…"
                        placeholderTextColor: Tema.tenue
                        color: Tema.texto
                        leftPadding: 42
                        selectByMouse: true
                        onAccepted: raiz.bajar()
                        background: Rectangle {
                            radius: 22
                            color: Tema.campo
                            border.width: 1
                            border.color: enlace.activeFocus ? Tema.azul : Tema.bordeCampo
                            Icono { x: 14; anchors.verticalCenter: parent.verticalCenter; width: 18; height: 18; nombre: "enlace"; color: Tema.tenue }
                        }
                    }
                    Boton { text: "Bajar"; icono: "descargar"; grande: true; enabled: enlace.text.trim() !== "" && vp.ajustes.hayYtDlp; onClicked: raiz.bajar() }
                }
                RowLayout {
                    visible: !vp.ajustes.hayYtDlp
                    spacing: 8
                    Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; nombre: "alerta"; color: Tema.peligro }
                    Text { text: "Falta yt-dlp para bajar. Instálalo con:  sudo pacman -S yt-dlp"; color: Tema.peligro; font.pixelSize: 13 }
                }
                Text {
                    Layout.fillWidth: true
                    text: vp.ajustes.navegador_premium
                        ? "Con tu YouTube Music Premium (sesión de " + vp.ajustes.navegador_premium + "): AAC a 256 kbps."
                        : "Sin YouTube Music Premium baja en Opus a ~160 kbps. Con Premium, AAC a 256 kbps (actívalo en Ajustes). Lo Hi-Res de verdad son tus archivos FLAC o WAV."
                    color: Tema.tenue
                    font.pixelSize: 12
                    wrapMode: Text.WordWrap
                }
            }
        }

        // --- lo que se baja ---
        RowLayout {
            Layout.fillWidth: true
            visible: vp.descargasLista.length > 0
            Text { text: "Bajando"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold; Layout.fillWidth: true }
            Boton { text: "Parar"; tipo: "fantasma"; onClicked: vp.pararDescargas() }
            Boton { text: "Limpiar"; tipo: "fantasma"; onClicked: vp.limpiarDescargas() }
        }
        Repeater {
            model: vp.descargasLista
            delegate: Rectangle {
                required property var modelData
                Layout.fillWidth: true
                implicitHeight: filaD.implicitHeight + 28
                radius: 16
                color: Tema.tarjeta
                ColumnLayout {
                    id: filaD
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 8
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 10
                        Rectangle {
                            implicitWidth: servicio.implicitWidth + 12
                            implicitHeight: 20
                            radius: 5
                            color: modelData.servicio === "spotify" ? "#1db95433" : "#ff000026"
                            Text {
                                id: servicio
                                anchors.centerIn: parent
                                text: modelData.servicio === "spotify" ? "Spotify" : "YouTube"
                                color: modelData.servicio === "spotify" ? "#5fdc8c" : "#ff8a8a"
                                font.pixelSize: 11
                                font.weight: Font.Bold
                            }
                        }
                        Text { Layout.fillWidth: true; text: modelData.nombre; color: Tema.texto; font.pixelSize: 14; font.weight: Font.DemiBold; elide: Text.ElideMiddle }
                        Text {
                            text: modelData.total > 1 ? modelData.hechas + " de " + modelData.total : ""
                            color: Tema.tenue
                            font.pixelSize: 12
                        }
                        Icono {
                            Layout.preferredWidth: 18; Layout.preferredHeight: 18
                            nombre: modelData.estado === "lista" ? "visto" : modelData.estado === "error" ? "alerta" : "descargar"
                            color: modelData.estado === "lista" ? Tema.exito : modelData.estado === "error" ? Tema.peligro : Tema.azul
                        }
                    }
                    Text {
                        Layout.fillWidth: true
                        text: modelData.actual
                        color: modelData.estado === "error" ? Tema.peligro : Tema.suave
                        font.pixelSize: 12
                        elide: Text.ElideRight
                    }
                    Rectangle {
                        visible: modelData.estado === "bajando"
                        Layout.fillWidth: true
                        height: 4
                        radius: 2
                        color: Tema.borde
                        Rectangle {
                            height: parent.height
                            radius: 2
                            color: Tema.azul
                            width: parent.width * (modelData.total > 1
                                ? Math.min(1, (modelData.hechas + modelData.porcentaje / 100) / modelData.total)
                                : modelData.porcentaje / 100)
                            Behavior on width { NumberAnimation { duration: 200 } }
                        }
                    }
                    Repeater {
                        model: modelData.avisos
                        delegate: Text {
                            required property string modelData
                            Layout.fillWidth: true
                            text: "• " + modelData
                            color: Tema.tenue
                            font.pixelSize: 12
                            elide: Text.ElideRight
                        }
                    }
                }
            }
        }

        // --- archivos ---
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: cajaArchivos.implicitHeight + 40
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            ColumnLayout {
                id: cajaArchivos
                anchors.fill: parent
                anchors.margins: 20
                spacing: 12
                RowLayout {
                    spacing: 10
                    Text { text: "Tus archivos sin pérdida"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold }
                    Etiqueta { texto: "Hi-Res"; nivel: "hires" }
                }
                Text {
                    Layout.fillWidth: true
                    text: "FLAC, WAV, ALAC, AIFF, WavPack o DSD: suenan a su frecuencia y sus bits originales (sin pasar todo a 48 kHz). "
                        + "Con tus audífonos LDAC llegan hasta 24 bits / 96 kHz. Se agregan donde están, sin copiarlos. "
                        + "Todo lo que pongas en " + vp.ajustes.musica + " aparece solo."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                RowLayout {
                    spacing: 10
                    Boton { text: "Agregar archivos"; icono: "archivo"; tipo: "secundario"; onClicked: dialogoArchivos.open() }
                    Boton { text: "Agregar una carpeta"; icono: "carpeta"; tipo: "secundario"; onClicked: dialogoCarpeta.open() }
                    Text { text: "o arrástralos a la ventana"; color: Tema.tenue; font.pixelSize: 12; Layout.leftMargin: 6 }
                }
            }
        }
    }

    Menu {
        id: menuPlaylist
        property int indice: -1
        title: "Bajar a la playlist"
        Instantiator {
            model: vp.playlists
            delegate: MenuItem {
                required property var modelData
                text: modelData.nombre
                onTriggered: vp.bajarResultado(menuPlaylist.indice, modelData.id)
            }
            onObjectAdded: (i, o) => menuPlaylist.insertItem(i, o)
            onObjectRemoved: (i, o) => menuPlaylist.removeItem(o)
        }
        MenuSeparator {}
        MenuItem {
            text: "Nueva playlist…"
            onTriggered: vp.bajarResultado(menuPlaylist.indice, vp.crearPlaylist("Nueva playlist"))
        }
    }

    FileDialog {
        id: dialogoArchivos
        title: "Agregar canciones"
        fileMode: FileDialog.OpenFiles
        nameFilters: ["Audio (*.flac *.wav *.aiff *.aif *.m4a *.alac *.mp3 *.ogg *.opus *.wv *.ape *.dsf *.dff)", "Todo (*)"]
        onAccepted: vp.agregarArchivos(selectedFiles)
    }
    FolderDialog {
        id: dialogoCarpeta
        title: "Agregar una carpeta de música"
        onAccepted: vp.agregarCarpeta(selectedFolder)
    }
}
