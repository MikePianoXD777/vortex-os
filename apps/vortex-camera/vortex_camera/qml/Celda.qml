// Un cuadro del mosaico: el video en vivo de una cámara (el subflujo, el ligero; el principal si es el
// único cuadro), con su nombre abajo y el punto rojo si se está grabando. Doble clic la abre en grande;
// clic derecho, el menú. Vacío, ofrece poner una cámara. Si se cae, se reintenta solo cada 8 segundos.
import QtQuick
import QtQuick.Controls.Basic
import QtMultimedia

Rectangle {
    id: raiz
    property string camId: ""
    property int pos: 0
    property bool principal: false
    property bool activo: true          // fuera de pantalla (vista grande, otra página) se suelta el video

    signal verGrande(string id)
    signal elegirCamara(int pos)
    signal quitarDeAqui(int pos)

    readonly property var info: { cam.camaras; return camId ? cam.camara(camId) : ({}) }
    readonly property string url: { cam.camaras; return camId !== "" && activo ? cam.urlVivo(camId, principal) : "" }
    readonly property string estado: cam.estados[camId] || ""
    readonly property bool grabando: cam.grabando[camId] === true
    readonly property bool conVideo: player.hasVideo && player.playbackState === MediaPlayer.PlayingState
                                     && player.mediaStatus !== MediaPlayer.LoadingMedia
                                     && player.mediaStatus !== MediaPlayer.StalledMedia
    property bool fallo: false

    color: "#05060c"
    border.width: zona.containsMouse ? 1 : 0
    border.color: Tema.bordeHover

    function reproducir() {
        player.stop()
        fallo = false
        if (url !== "") {
            player.source = url
            player.play()
        } else {
            player.source = ""
        }
    }
    onUrlChanged: reproducir()
    Component.onCompleted: reproducir()

    MediaPlayer {
        id: player
        videoOutput: salida
        audioOutput: AudioOutput { muted: true }
        onErrorOccurred: (error, texto) => { raiz.fallo = true; reintento.restart() }
        onMediaStatusChanged: {
            if (mediaStatus === MediaPlayer.EndOfMedia && raiz.url !== "")   // la cámara cortó el flujo
                reintento.restart()
        }
    }
    Timer { id: reintento; interval: 8000; onTriggered: raiz.reproducir() }

    VideoOutput {
        id: salida
        anchors.fill: parent
        anchors.margins: raiz.border.width
        fillMode: VideoOutput.PreserveAspectFit
    }

    // --- lo que se ve cuando no hay video ---
    Column {
        anchors.centerIn: parent
        spacing: 8
        visible: raiz.camId === "" || !raiz.conVideo
        Icono {
            anchors.horizontalCenter: parent.horizontalCenter
            width: raiz.camId === "" ? 30 : 26; height: width
            nombre: raiz.camId === "" ? "mas" : raiz.fallo || raiz.estado === "sin_conexion" || raiz.estado === "codigo" ? "alerta"
                  : raiz.estado === "sin_ip" ? "red" : "camara"
            color: raiz.fallo || raiz.estado === "sin_conexion" || raiz.estado === "codigo" ? Tema.peligro : Tema.tenue
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            visible: raiz.height > 90
            text: raiz.camId === "" ? (cam.camaras.length ? "Poner una cámara" : "")
                : raiz.estado === "sin_ip" ? "Sin IP · edítala"
                : raiz.estado === "codigo" ? "Código incorrecto · edítala"
                : raiz.fallo || raiz.estado === "sin_conexion" ? "Sin señal · reintentando"
                : "Conectando…"
            color: Tema.tenue
            font.pixelSize: raiz.width > 260 ? 13 : 11
        }
    }

    // --- nombre abajo, con un degradado para que se lea sobre cualquier imagen ---
    Rectangle {
        visible: raiz.camId !== ""
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: raiz.border.width
        height: Math.min(34, raiz.height / 3)
        gradient: Gradient {
            GradientStop { position: 0; color: "#00000000" }
            GradientStop { position: 1; color: "#b0000000" }
        }
        Text {
            anchors.left: parent.left
            anchors.leftMargin: 10
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 6
            text: raiz.info.nombre || ""
            color: "#ffffff"
            font.pixelSize: raiz.width > 260 ? 13 : 11
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            style: Text.Outline
            styleColor: "#60000000"
        }
    }

    Row {   // grabando
        visible: raiz.grabando
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 8
        spacing: 5
        Rectangle {
            width: 9; height: 9; radius: 5
            color: "#ff4d5e"
            anchors.verticalCenter: parent.verticalCenter
            SequentialAnimation on opacity {
                running: raiz.grabando
                loops: Animation.Infinite
                NumberAnimation { to: 0.25; duration: 700 }
                NumberAnimation { to: 1; duration: 700 }
            }
        }
        Text { text: "REC"; color: "#ffffff"; font.pixelSize: 11; font.weight: Font.Bold; style: Text.Outline; styleColor: "#80000000" }
    }

    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: raiz.camId === "" ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: (e) => {
            if (e.button === Qt.RightButton && raiz.camId !== "")
                menu.popup()
            else if (raiz.camId === "")
                raiz.elegirCamara(raiz.pos)
        }
        onDoubleClicked: (e) => { if (raiz.camId !== "" && e.button === Qt.LeftButton) raiz.verGrande(raiz.camId) }
    }

    MenuVortex {
        id: menu
        OpcionMenu { text: "Ver en grande"; icono: "pantalla"; onTriggered: raiz.verGrande(raiz.camId) }
        OpcionMenu { text: "Sacar foto"; icono: "foto"; onTriggered: cam.foto(raiz.camId) }
        OpcionMenu {
            text: raiz.grabando ? "Dejar de grabar" : "Grabar"
            icono: raiz.grabando ? "detener" : "grabar"
            peligro: raiz.grabando
            onTriggered: cam.alternarGrabar(raiz.camId)
        }
        OpcionMenu { text: "Cambiar de cámara…"; icono: "camara"; onTriggered: raiz.elegirCamara(raiz.pos) }
        OpcionMenu { text: "Volver a conectar"; icono: "girar"; onTriggered: raiz.reproducir() }
        OpcionMenu { text: "Quitar de este cuadro"; icono: "cerrar"; onTriggered: raiz.quitarDeAqui(raiz.pos) }
    }
}
