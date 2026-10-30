// Grabaciones: lo que una cámara guardó en su microSD. Eliges cámara, día y hora, y la cámara manda el
// video desde ahí (salta sola los huecos sin grabación). Los botones de saltar vuelven a pedirlo desde
// otra hora; arriba del video se ve la hora que estás viendo.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

Item {
    id: raiz
    property string camId: ""
    property bool activa: visible
    property string fecha: Qt.formatDate(new Date(), "yyyy-MM-dd")
    property string hora: Qt.formatTime(new Date(Date.now() - 3600 * 1000), "HH:mm")
    property string url: ""
    property bool audio: false
    property bool fallo: false
    readonly property bool conVideo: player.hasVideo && player.playbackState !== MediaPlayer.StoppedState
                                     && player.mediaStatus !== MediaPlayer.LoadingMedia

    function elegir(id) {
        camId = id
        var i = cam.camaras.findIndex(c => c.id === id)
        if (i >= 0) selector.currentIndex = i
        ver()
    }
    function ver() {
        player.stop()
        fallo = false
        if (!/^\d{4}-\d{2}-\d{2}$/.test(campoFecha.text) || !/^\d{1,2}:\d{2}$/.test(campoHora.text)) {
            error.text = "Escribe el día como 2026-10-30 y la hora como 14:30."
            return
        }
        error.text = ""
        fecha = campoFecha.text
        hora = campoHora.text.length === 4 ? "0" + campoHora.text : campoHora.text
        url = camId ? cam.urlGrabado(camId, fecha, hora) : ""
        player.source = url
        if (url !== "" && activa)
            player.play()
    }
    function saltar(minutos) {
        // se salta desde la hora que se está viendo, no desde la que se pidió
        var visto = cam.moverHora(fecha, hora, Math.floor(player.position / 60000))
        var nueva = cam.moverHora(visto[0], visto[1], minutos)
        campoFecha.text = nueva[0]
        campoHora.text = nueva[1]
        ver()
    }
    function horaVista() {
        var p = cam.moverHora(fecha, hora, 0)
        var base = new Date(p[0] + "T" + p[1] + ":00")
        return Qt.formatDateTime(new Date(base.getTime() + player.position), "dddd d 'de' MMMM, HH:mm:ss")
    }
    onActivaChanged: if (!activa) player.stop()
    Component.onCompleted: if (!camId && cam.camaras.length) camId = cam.camaras[0].id

    MediaPlayer {
        id: player
        videoOutput: salida
        audioOutput: AudioOutput { muted: !raiz.audio }
        onErrorOccurred: (e, texto) => raiz.fallo = true
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 24
        spacing: 16

        ColumnLayout {
            spacing: 3
            Text { text: "Grabaciones"; color: Tema.texto; font.pixelSize: 24; font.weight: Font.DemiBold }
            Text { text: "Lo que cada cámara guardó en su microSD."; color: Tema.suave; font.pixelSize: 14 }
        }

        RowLayout {
            Layout.fillWidth: true
            spacing: 12
            ColumnLayout {
                spacing: 6
                Text { text: "Cámara"; color: Tema.suave; font.pixelSize: 12; font.weight: Font.DemiBold }
                Selector {
                    id: selector
                    Layout.preferredWidth: 240
                    model: cam.camaras
                    textRole: "nombre"
                    currentIndex: Math.max(0, cam.camaras.findIndex(c => c.id === raiz.camId))
                    onActivated: (i) => { raiz.camId = cam.camaras[i].id; raiz.ver() }
                }
            }
            Entrada {
                id: campoFecha
                Layout.preferredWidth: 150
                etiqueta: "Día"
                text: raiz.fecha
                placeholderText: "2026-10-30"
                onAceptado: raiz.ver()
            }
            Entrada {
                id: campoHora
                Layout.preferredWidth: 110
                etiqueta: "Desde las"
                text: raiz.hora
                placeholderText: "14:30"
                onAceptado: raiz.ver()
            }
            Boton {
                Layout.alignment: Qt.AlignBottom
                icono: "play"
                text: "Ver"
                enabled: raiz.camId !== ""
                onClicked: raiz.ver()
            }
            Item { Layout.fillWidth: true }
        }
        Text { id: error; visible: text !== ""; color: Tema.peligro; font.pixelSize: 13 }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            radius: Tema.radioTarjeta
            color: "#05060c"
            clip: true
            VideoOutput {
                id: salida
                anchors.fill: parent
                fillMode: VideoOutput.PreserveAspectFit
            }
            Column {
                anchors.centerIn: parent
                spacing: 10
                visible: !raiz.conVideo
                Icono {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 34; height: 34
                    nombre: raiz.fallo ? "alerta" : raiz.url ? "historial" : "calendario"
                    color: raiz.fallo ? Tema.peligro : Tema.tenue
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: Math.min(implicitWidth, 420)
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.Wrap
                    text: !cam.camaras.length ? "Primero agrega una cámara."
                        : raiz.fallo ? "La cámara no mandó nada de esa hora. ¿Tiene microSD y estaba grabando?"
                        : raiz.url ? "Pidiéndole el video a la cámara…" : "Elige el día y la hora, y dale a Ver."
                    color: Tema.suave
                    font.pixelSize: 14
                }
            }
            Rectangle {
                visible: raiz.conVideo
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.margins: 14
                width: horaTexto.implicitWidth + 24
                height: 30
                radius: 15
                color: Tema.flotante
                Text {
                    id: horaTexto
                    anchors.centerIn: parent
                    text: raiz.conVideo ? raiz.horaVista() : ""
                    color: Tema.texto
                    font.pixelSize: 12
                    font.features: { "tnum": 1 }
                }
            }
        }

        RowLayout {
            Layout.alignment: Qt.AlignHCenter
            spacing: 6
            enabled: raiz.url !== ""
            Boton { tipo: "fantasma"; icono: "retroceder"; text: "10 min"; onClicked: raiz.saltar(-10) }
            Boton { tipo: "fantasma"; icono: "izquierda"; text: "1 min"; onClicked: raiz.saltar(-1) }
            BotonIcono {
                tam: 46
                relleno: true
                icono: player.playbackState === MediaPlayer.PlayingState ? "pausa" : "play"
                ayuda: player.playbackState === MediaPlayer.PlayingState ? "Pausa" : "Seguir"
                onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
            }
            Boton { tipo: "fantasma"; icono: "derecha"; text: "1 min"; onClicked: raiz.saltar(1) }
            Boton { tipo: "fantasma"; icono: "adelantar"; text: "10 min"; onClicked: raiz.saltar(10) }
            Item { implicitWidth: 12 }
            BotonIcono {
                icono: raiz.audio ? "volumen" : "silencio"
                activo: raiz.audio
                ayuda: raiz.audio ? "Silenciar" : "Escuchar"
                onClicked: raiz.audio = !raiz.audio
            }
        }
    }
}
