// Una cámara en grande, con el flujo principal (la resolución completa) y su audio: foto, grabar, ver lo
// grabado y, si gira, las flechas (se mueve mientras las tengas apretadas). Esc la cierra.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

Rectangle {
    id: raiz
    property string camId: ""
    property bool activo: visible
    property bool audio: false

    signal cerrar()
    signal verGrabaciones(string id)

    readonly property var info: { cam.camaras; return camId ? cam.camara(camId) : ({}) }
    readonly property string url: { cam.camaras; return camId !== "" && activo ? cam.urlVivo(camId, true) : "" }
    readonly property bool grabando: cam.grabando[camId] === true
    readonly property string estado: cam.estados[camId] || ""
    readonly property bool conVideo: player.hasVideo && player.playbackState === MediaPlayer.PlayingState
                                     && player.mediaStatus !== MediaPlayer.LoadingMedia
                                     && player.mediaStatus !== MediaPlayer.StalledMedia
    property bool fallo: false

    color: "#05060c"

    function reproducir() {
        player.stop()
        fallo = false
        player.source = url
        if (url !== "")
            player.play()
    }
    onUrlChanged: reproducir()
    onCamIdChanged: audio = false

    MediaPlayer {
        id: player
        videoOutput: salida
        audioOutput: AudioOutput { muted: !raiz.audio }
        onErrorOccurred: (error, texto) => { raiz.fallo = true; reintento.restart() }
        onMediaStatusChanged: if (mediaStatus === MediaPlayer.EndOfMedia && raiz.url !== "") reintento.restart()
    }
    Timer { id: reintento; interval: 8000; onTriggered: raiz.reproducir() }

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
            nombre: raiz.fallo || raiz.estado === "sin_conexion" || raiz.estado === "codigo" ? "alerta" : "camara"
            color: raiz.fallo || raiz.estado === "sin_conexion" || raiz.estado === "codigo" ? Tema.peligro : Tema.tenue
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: raiz.estado === "sin_ip" ? "Esta cámara no tiene IP: edítala para escribirla."
                : raiz.estado === "codigo" ? "La cámara no aceptó el código de seguridad: edítala para corregirlo."
                : raiz.fallo || raiz.estado === "sin_conexion" ? "Sin señal. Reintentando…" : "Conectando…"
            color: Tema.suave
            font.pixelSize: 14
        }
    }

    MouseArea {   // doble clic también cierra, como en Imou
        anchors.fill: parent
        onDoubleClicked: raiz.cerrar()
    }

    // --- arriba: nombre y cerrar ---
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        height: 64
        gradient: Gradient {
            GradientStop { position: 0; color: "#b0000000" }
            GradientStop { position: 1; color: "#00000000" }
        }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 20
            anchors.rightMargin: 14
            anchors.bottomMargin: 16
            spacing: 10
            Text {
                text: raiz.info.nombre || ""
                color: "#ffffff"
                font.pixelSize: 17
                font.weight: Font.DemiBold
                style: Text.Outline
                styleColor: "#60000000"
            }
            Rectangle {
                visible: raiz.grabando
                implicitWidth: rec.implicitWidth + 16
                implicitHeight: 22
                radius: 11
                color: "#d9ff4d5e"
                Text { id: rec; anchors.centerIn: parent; text: "● REC"; color: "#ffffff"; font.pixelSize: 11; font.weight: Font.Bold }
            }
            Item { Layout.fillWidth: true }
            BotonIcono { icono: "cerrar"; ayuda: "Volver al mosaico (Esc)"; onClicked: raiz.cerrar() }
        }
    }

    // --- abajo: los controles ---
    Rectangle {
        id: barra
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 20
        width: controles.implicitWidth + 24
        height: 56
        radius: 28
        color: Tema.flotante
        border.width: 1
        border.color: Tema.borde
        RowLayout {
            id: controles
            anchors.centerIn: parent
            spacing: 6
            BotonIcono {
                icono: raiz.audio ? "volumen" : "silencio"
                activo: raiz.audio
                ayuda: raiz.audio ? "Silenciar" : "Escuchar la cámara"
                onClicked: raiz.audio = !raiz.audio
            }
            BotonIcono { icono: "foto"; ayuda: "Sacar foto"; onClicked: cam.foto(raiz.camId) }
            BotonIcono {
                icono: raiz.grabando ? "detener" : "grabar"
                activo: raiz.grabando
                ayuda: raiz.grabando ? "Dejar de grabar" : "Grabar"
                onClicked: cam.alternarGrabar(raiz.camId)
            }
            Boton { tipo: "fantasma"; icono: "historial"; text: "Grabaciones"; onClicked: raiz.verGrabaciones(raiz.camId) }
            Boton { tipo: "fantasma"; icono: "girar"; text: "Reconectar"; onClicked: raiz.reproducir() }
        }
    }

    // --- las flechas de las cámaras que giran ---
    Item {
        id: pad
        visible: raiz.info.ptz !== "no" && raiz.camId !== ""
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 24
        width: 150
        height: 150

        component Flecha: Rectangle {
            id: f
            property string dir: ""
            property string icono: dir
            width: 46; height: 46; radius: 23
            color: zonaF.pressed ? Tema.azul : zonaF.containsMouse ? Tema.realceFuerte : Tema.flotante
            border.width: 1
            border.color: Tema.borde
            Icono { anchors.centerIn: parent; width: 22; height: 22; nombre: f.icono; color: zonaF.pressed ? Tema.sobreAcento : Tema.texto }
            MouseArea {
                id: zonaF
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onPressed: cam.moverPtz(raiz.camId, f.dir)
                onReleased: cam.pararPtz(raiz.camId, f.dir)
                onCanceled: cam.pararPtz(raiz.camId, f.dir)
            }
        }
        Rectangle { anchors.centerIn: parent; width: 150; height: 150; radius: 75; color: "#55000000" }
        Flecha { dir: "arriba"; anchors.horizontalCenter: parent.horizontalCenter; anchors.top: parent.top; anchors.topMargin: 4 }
        Flecha { dir: "abajo"; anchors.horizontalCenter: parent.horizontalCenter; anchors.bottom: parent.bottom; anchors.bottomMargin: 4 }
        Flecha { dir: "izquierda"; anchors.verticalCenter: parent.verticalCenter; anchors.left: parent.left; anchors.leftMargin: 4 }
        Flecha { dir: "derecha"; anchors.verticalCenter: parent.verticalCenter; anchors.right: parent.right; anchors.rightMargin: 4 }
    }
    Boton {
        visible: raiz.info.ptz === "no"
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: 28
        tipo: "fantasma"
        icono: "girar"
        text: "¿Sí gira? Probar las flechas"
        onClicked: cam.probarPtz(raiz.camId)
    }
}
