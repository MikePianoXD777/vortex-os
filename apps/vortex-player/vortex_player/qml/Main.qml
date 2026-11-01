// La ventana de Vortex Player: a la izquierda la navegación y las playlists, al centro la página y abajo
// la barra del reproductor. Se le pueden arrastrar archivos o carpetas, o un enlace.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

ApplicationWindow {
    id: ventana
    onActiveChanged: if (active) vp.revisarOtroAparato()   // 1.5: ¿sonaba algo en el cel?
    width: 1240
    height: 800
    minimumWidth: 940
    minimumHeight: 600
    visible: true
    title: rep.cancion.titulo ? rep.cancion.titulo + (rep.cancion.artista ? " · " + rep.cancion.artista : "") : "Vortex Player"
    color: Tema.fondo

    property string pagina: "canciones"   // canciones | agregar | ecualizador | ajustes | letra | cola
    property string paginaAntes: "canciones"

    function ir(p, playlist) {
        pagina = p
        if (p === "canciones") vp.verPlaylist(playlist === undefined ? -1 : playlist)
    }

    // 2.4: la letra ocupa la página (como en Spotify); otra vez el botón, regresa a donde estabas
    function alternarLetra() { alternar("letra") }
    // 2.4: la cola «A continuación» igual: ocupa la página y otra vez el botón regresa
    function alternar(p) {
        if (pagina === p) pagina = paginaAntes
        else { if (pagina !== "letra" && pagina !== "cola") paginaAntes = pagina; pagina = p }
    }

    Connections {
        target: vp
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
        function onAviso(texto) { aviso.mostrar(texto) }
    }
    Connections {
        target: rep
        function onError(texto) { aviso.mostrar(texto) }
    }

    Shortcut { sequence: "Space"; enabled: !(activeFocusItem instanceof TextInput) && !(activeFocusItem instanceof TextEdit); onActivated: rep.alternar() }
    Shortcut { sequence: "Ctrl+Right"; onActivated: rep.siguiente() }
    Shortcut { sequence: "Ctrl+Left"; onActivated: rep.anterior() }
    Shortcut { sequence: "Ctrl+L"; onActivated: ventana.ir("agregar") }
    Shortcut { sequences: [StandardKey.Paste]; enabled: !(activeFocusItem instanceof TextInput); onActivated: ventana.ir("agregar") }
    Shortcut { sequence: "Ctrl+Y"; onActivated: ventana.alternarLetra() }
    Shortcut { sequence: "Ctrl+Q"; onActivated: ventana.alternar("cola") }
    Shortcut { sequence: "Ctrl+M"; onActivated: mini.alternar() }

    // 2.4: el mini reproductor, una ventanita encima de todo (Ctrl+M)
    MiniReproductor { id: mini; objectName: "mini"; principal: ventana }

    component Navegar: Rectangle {
        id: nav
        property string texto: ""
        property string icono: ""
        property bool elegido: false
        property int contador: -1
        signal pulsado()
        Layout.fillWidth: true
        implicitHeight: 40
        radius: 10
        color: elegido ? Tema.barraSel : zonaNav.containsMouse ? Tema.barraHover : "transparent"
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 12
            Icono { Layout.preferredWidth: 19; Layout.preferredHeight: 19; nombre: nav.icono; color: nav.elegido ? Tema.texto : Tema.suave }
            Text {
                Layout.fillWidth: true
                text: nav.texto
                color: nav.elegido ? Tema.texto : Tema.suave
                font.pixelSize: 14
                font.weight: nav.elegido ? Font.DemiBold : Font.Normal
                elide: Text.ElideRight
            }
            Text { visible: nav.contador >= 0; text: nav.contador; color: Tema.tenue; font.pixelSize: 12 }
        }
        MouseArea { id: zonaNav; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: nav.pulsado() }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            // --- lateral ---
            Rectangle {
                Layout.preferredWidth: Tema.anchoLateral
                Layout.fillHeight: true
                color: Tema.barra
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 14
                    spacing: 4
                    RowLayout {
                        Layout.leftMargin: 6
                        Layout.bottomMargin: 14
                        Layout.topMargin: 6
                        spacing: 10
                        Image { source: "logo.svg"; sourceSize.width: 64; sourceSize.height: 64; Layout.preferredWidth: 32; Layout.preferredHeight: 32 }
                        Text { text: "Vortex Player"; color: Tema.texto; font.pixelSize: 17; font.weight: Font.Bold }
                    }
                    Navegar { texto: "Tu biblioteca"; icono: "musica"; elegido: ventana.pagina === "canciones" && vp.vistaPlaylist < 0; onPulsado: ventana.ir("canciones") }
                    Navegar { texto: "Agregar música"; icono: "descargar"; elegido: ventana.pagina === "agregar"; onPulsado: ventana.ir("agregar")
                        contador: { var n = 0; for (var i = 0; i < vp.descargasLista.length; i++) n += vp.descargasLista[i].estado === "bajando" || vp.descargasLista[i].estado === "espera"; return n > 0 ? n : -1 } }
                    Navegar { visible: !!vp.ajustes.hayEq; texto: "Ecualizador"; icono: "ecualizador"; elegido: ventana.pagina === "ecualizador"; onPulsado: ventana.ir("ecualizador") }
                    Navegar { texto: "Ajustes"; icono: "ajustes"; elegido: ventana.pagina === "ajustes"; onPulsado: ventana.ir("ajustes") }

                    RowLayout {
                        Layout.fillWidth: true
                        Layout.topMargin: 18
                        Layout.leftMargin: 12
                        Text { Layout.fillWidth: true; text: "Playlists"; color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold; font.capitalization: Font.AllUppercase; font.letterSpacing: 1 }
                        BotonIcono {
                            icono: "mas"; tam: 28; ayuda: "Nueva playlist"
                            onClicked: ventana.ir("canciones", vp.crearPlaylist("Nueva playlist"))
                        }
                    }
                    ListView {
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        clip: true
                        spacing: 2
                        model: vp.playlists
                        boundsBehavior: Flickable.StopAtBounds
                        // 2.4: como «Tu biblioteca» de Spotify: el mosaico de sus primeras 4 canciones
                        delegate: Rectangle {
                            id: filaPl
                            required property var modelData
                            readonly property bool elegido: ventana.pagina === "canciones" && vp.vistaPlaylist === modelData.id
                            width: ListView.view.width
                            implicitHeight: 56
                            radius: 8
                            color: elegido ? Tema.barraSel : zonaPl.containsMouse ? Tema.barraHover : "transparent"
                            Behavior on color { ColorAnimation { duration: 150 } }
                            RowLayout {
                                anchors.fill: parent
                                anchors.leftMargin: 6
                                anchors.rightMargin: 10
                                spacing: 10
                                Mosaico { Layout.preferredWidth: 44; Layout.preferredHeight: 44; rutas: filaPl.modelData.portadas || []; radio: 5; meGusta: !!filaPl.modelData.meGusta }
                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1
                                    Text { Layout.fillWidth: true; text: filaPl.modelData.nombre; color: rep.origen === filaPl.modelData.nombre ? Tema.azul : Tema.texto; font.pixelSize: 14; font.weight: Font.Medium; elide: Text.ElideRight }
                                    Text {
                                        Layout.fillWidth: true
                                        text: (filaPl.modelData.url ? "Importada" : "Playlist") + " · " + filaPl.modelData.canciones + (filaPl.modelData.canciones === 1 ? " canción" : " canciones")
                                        color: Tema.tenue; font.pixelSize: 12; elide: Text.ElideRight
                                    }
                                }
                            }
                            MouseArea { id: zonaPl; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ventana.ir("canciones", filaPl.modelData.id) }
                        }
                        Text {
                            visible: parent.count === 0
                            width: parent.width
                            leftPadding: 12
                            rightPadding: 12
                            text: "Las playlists que importes o crees aparecen aquí."
                            color: Tema.tenue
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }

            // --- página (2.4: cada una entra con desenfoque de movimiento) ---
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                Entrar {
                    anchors.fill: parent; mostrado: ventana.pagina === "canciones"
                    PaginaCanciones {
                        id: paginaCanciones
                        anchors.fill: parent
                        onIrAAgregar: ventana.ir("agregar")
                        onVerResumen: ventana.ir("ajustes")
                        onBuscarEnInternet: (texto) => { ventana.ir("agregar"); paginaAgregar.buscar(texto) }
                    }
                }
                Entrar { anchors.fill: parent; mostrado: ventana.pagina === "agregar"; PaginaAgregar { id: paginaAgregar; anchors.fill: parent } }
                Entrar { anchors.fill: parent; mostrado: ventana.pagina === "ecualizador"; PaginaEcualizador { anchors.fill: parent } }
                Entrar { anchors.fill: parent; mostrado: ventana.pagina === "ajustes"; PaginaAjustes { anchors.fill: parent } }
                Entrar {
                    anchors.fill: parent; mostrado: ventana.pagina === "letra"; desde: 1.04
                    PaginaLetra { anchors.fill: parent; onCerrar: ventana.alternarLetra() }
                }
                Entrar {
                    anchors.fill: parent; mostrado: ventana.pagina === "cola"; desde: 1.04
                    PaginaCola { anchors.fill: parent; onCerrar: ventana.alternar("cola") }
                }
            }
        }

        // 1.5: «¿Seguir aquí?»: lo que dejaste sonando en el cel
        Rectangle {
            id: seguir
            objectName: "seguir"
            visible: !!vp.seguir.id
            Layout.fillWidth: true
            implicitHeight: visible ? 56 : 0
            color: Qt.rgba(Tema.violeta.r, Tema.violeta.g, Tema.violeta.b, 0.22)
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 16
                anchors.rightMargin: 12
                spacing: 12
                Rectangle {
                    Layout.preferredWidth: 40; Layout.preferredHeight: 40
                    radius: 6
                    color: Tema.tarjeta
                    clip: true
                    Image { anchors.fill: parent; source: vp.seguir.portada || ""; fillMode: Image.PreserveAspectCrop; asynchronous: true }
                }
                Text {
                    Layout.fillWidth: true
                    text: "En " + (vp.seguir.nombre || "tu cel") + " sonaba «" + (vp.seguir.titulo || "") + "»"
                          + (vp.seguir.artista ? " de " + vp.seguir.artista : "") + " · " + (vp.seguir.hace || "")
                    color: Tema.texto
                    font.pixelSize: 13
                    elide: Text.ElideRight
                }
                Boton { text: "Seguir aquí"; icono: "play"; onClicked: vp.seguirAqui() }
                BotonIcono { icono: "cerrar"; tam: 30; ayuda: "Cerrar"; onClicked: vp.descartarSeguir() }
            }
        }

        Barra {
            Layout.fillWidth: true
            letraAbierta: ventana.pagina === "letra"
            colaAbierta: ventana.pagina === "cola"
            miniAbierto: mini.visible
            onAlternarLetra: ventana.alternarLetra()
            onAlternarCola: ventana.alternar("cola")
            onAlternarMini: mini.alternar()
        }
    }

    // Arrastrar archivos, carpetas o un enlace a la ventana
    DropArea {
        id: soltar
        anchors.fill: parent
        onDropped: (d) => {
            if (d.hasUrls && d.urls.length && String(d.urls[0]).startsWith("file:")) vp.agregarArchivos(d.urls)
            else if (d.hasText) { vp.bajar(d.text); ventana.ir("agregar") }
            else if (d.hasUrls && d.urls.length) { vp.bajar(String(d.urls[0])); ventana.ir("agregar") }
        }
        Rectangle {
            anchors.fill: parent
            anchors.margins: 12
            visible: soltar.containsDrag
            radius: Tema.radioGrande
            color: Qt.rgba(Tema.azul.r, Tema.azul.g, Tema.azul.b, 0.12)
            border.width: 2
            border.color: Tema.azul
            Text { anchors.centerIn: parent; text: "Suelta aquí para agregarlo a tu biblioteca"; color: Tema.texto; font.pixelSize: 20; font.weight: Font.DemiBold }
        }
    }

    // Avisos cortos abajo al centro
    Rectangle {
        id: aviso
        property alias texto: textoAviso.text
        function mostrar(t) { texto = t; opacity = 1; ocultar.restart() }
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 108
        implicitWidth: Math.min(textoAviso.implicitWidth + 36, ventana.width - 80)
        implicitHeight: 40
        radius: 20
        color: Tema.texto
        opacity: 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 200 } }
        Text { id: textoAviso; anchors.centerIn: parent; width: parent.width - 36; color: Tema.fondo; font.pixelSize: 13; font.weight: Font.DemiBold; elide: Text.ElideRight; horizontalAlignment: Text.AlignHCenter }
        Timer { id: ocultar; interval: 3200; onTriggered: aviso.opacity = 0 }
    }
}
