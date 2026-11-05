// La ventana de Vortexine Video, como la Cámara de Windows: la imagen a toda la ventana y a la derecha foto o
// video, el disparador y la última captura. Espacio toma la foto o empieza y detiene el video.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtMultimedia

ApplicationWindow {
    id: ventana
    width: 1100
    height: 680
    minimumWidth: 640
    minimumHeight: 460
    visible: true
    title: "Vortexine Video"
    color: "#000000"

    property string modo: vv.ajuste("modo")          // foto | video
    property int temporizador: vv.ajuste("temporizador")   // 0, 3, 5 o 10 segundos
    property bool microfono: vv.ajuste("microfono")
    property bool cuadricula: vv.ajuste("cuadricula")
    property string nombreCamara: vv.ajuste("camara")
    property string resolucionPreferida: vv.ajuste("resolucion")
    property int cuenta: 0                           // lo que falta del temporizador
    property string errorCamara: ""
    property bool cerrarAlTerminar: false
    property bool panelAbierto: false
    property var formatosPlanos: []

    readonly property var camaras: vv.sinCamara ? [] : dispositivos.videoInputs
    readonly property bool hayCamara: camaras.length > 0
    readonly property var camaraActual: {
        for (var i = 0; i < camaras.length; i++)
            if (camaras[i].description === nombreCamara) return camaras[i]
        return hayCamara ? (dispositivos.defaultVideoInput.isNull ? camaras[0] : dispositivos.defaultVideoInput) : null
    }
    readonly property bool grabando: grabadora.recorderState !== MediaRecorder.StoppedState
    readonly property bool enPausa: grabadora.recorderState === MediaRecorder.PausedState
    readonly property bool lista: hayCamara && camara.active && errorCamara === ""
    readonly property var resoluciones: vv.resoluciones(formatosPlanos)

    onCamaraActualChanged: { errorCamara = ""; aplicarFormato() }
    onResolucionPreferidaChanged: aplicarFormato()
    Component.onCompleted: aplicarFormato()

    // Qt da los formatos como objetos suyos; el motor (que se prueba sin cámara) elige sobre datos simples.
    function aplicarFormato() {
        if (!camaraActual) { formatosPlanos = []; return }
        var formatos = camaraActual.videoFormats, planos = []
        for (var i = 0; i < formatos.length; i++)
            planos.push({ancho: formatos[i].resolution.width, alto: formatos[i].resolution.height, fps: formatos[i].maxFrameRate})
        formatosPlanos = planos
        var elegido = vv.elegirFormato(planos, resolucionPreferida)
        if (elegido >= 0 && camara.cameraFormat !== formatos[elegido]) camara.cameraFormat = formatos[elegido]
    }

    function aviso(texto) { avisoTexto.text = texto; avisoCaja.opacity = 1; avisoReloj.restart() }

    function disparar() {
        if (grabando) { grabadora.stop(); return }
        if (cuenta > 0) { cuenta = 0; relojCuenta.stop(); return }   // volver a apretar cancela
        if (!lista) return
        if (temporizador > 0) { cuenta = temporizador; relojCuenta.start(); return }
        capturar()
    }
    function capturar() {
        if (modo === "foto") {
            var ruta = vv.rutaFoto()
            if (ruta === "") { aviso("No se pudo crear la carpeta de fotos"); return }
            fotoCaptura.captureToFile(ruta)
            destello.restart()
        } else {
            var url = vv.urlVideo()
            if (url === "") { aviso("No se pudo crear la carpeta de videos"); return }
            grabadora.outputLocation = url
            grabadora.record()
        }
    }
    function cambiarCamara() {
        var i = camaras.findIndex((c) => c.description === camaraActual.description)
        nombreCamara = camaras[(i + 1) % camaras.length].description
        vv.guardar("camara", nombreCamara)
    }
    function textoError(texto) {
        var t = (texto || "").toLowerCase()
        return t.indexOf("busy") >= 0 || t.indexOf("in use") >= 0 ? "Otra app está usando tu cámara" : "No se pudo abrir tu cámara"
    }

    Timer {
        id: relojCuenta
        interval: 1000
        repeat: true
        onTriggered: {
            ventana.cuenta--
            if (ventana.cuenta <= 0) { stop(); ventana.capturar() }
        }
    }

    MediaDevices { id: dispositivos }

    CaptureSession {
        id: sesion
        videoOutput: salida
        camera: Camera {
            id: camara
            cameraDevice: ventana.camaraActual ? ventana.camaraActual : dispositivos.defaultVideoInput
            // Minimizada no ocupa la cámara ni el procesador; grabando sigue.
            active: ventana.hayCamara && ventana.errorCamara === ""
                    && (ventana.visibility !== Window.Minimized && ventana.visibility !== Window.Hidden || ventana.grabando)
            onErrorOccurred: (error, texto) => { ventana.errorCamara = ventana.textoError(texto) }
        }
        imageCapture: ImageCapture {
            id: fotoCaptura
            onImageSaved: (id, ruta) => vv.registrar(ruta)
            onErrorOccurred: (id, error, texto) => ventana.aviso("No se pudo guardar la foto")
        }
        recorder: MediaRecorder {
            id: grabadora
            quality: MediaRecorder.HighQuality
            mediaFormat { fileFormat: MediaFormat.MPEG4; videoCodec: MediaFormat.VideoCodec.H264; audioCodec: MediaFormat.AudioCodec.AAC }
            onRecorderStateChanged: {
                if (recorderState === MediaRecorder.StoppedState) {
                    vv.registrar(actualLocation.toString())
                    if (ventana.cerrarAlTerminar) Qt.quit()
                }
            }
            onErrorOccurred: (error, texto) => ventana.aviso("No se pudo grabar el video")
        }
        audioInput: ventana.microfono && ventana.modo === "video" ? entrada : null
    }
    AudioInput { id: entrada }

    // Termina de escribir el video antes de cerrar: si no, queda un archivo que no abre.
    onClosing: (cierre) => {
        if (grabando) { cierre.accepted = false; cerrarAlTerminar = true; grabadora.stop() }
    }

    VideoOutput {
        id: salida
        anchors.fill: parent
        fillMode: VideoOutput.PreserveAspectFit
    }

    // Cuadrícula: los tercios, sobre la imagen y no sobre las franjas negras.
    Item {
        visible: ventana.cuadricula && ventana.lista
        x: salida.contentRect.x
        y: salida.contentRect.y
        width: salida.contentRect.width
        height: salida.contentRect.height
        Repeater {
            model: 2
            Rectangle { required property int index; x: parent.width * (index + 1) / 3; width: 1; height: parent.height; color: Qt.rgba(1, 1, 1, 0.4) }
        }
        Repeater {
            model: 2
            Rectangle { required property int index; y: parent.height * (index + 1) / 3; height: 1; width: parent.width; color: Qt.rgba(1, 1, 1, 0.4) }
        }
    }

    Rectangle {   // el destello de la foto
        anchors.fill: parent
        color: "#ffffff"
        opacity: 0
        SequentialAnimation on opacity {
            id: destello
            running: false
            NumberAnimation { to: 0.75; duration: 40 }
            NumberAnimation { to: 0; duration: 180 }
        }
    }

    // Sin cámara o con error: qué pasa y qué hacer. Si la conectas, aparece sola.
    ColumnLayout {
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: -48
        visible: !ventana.hayCamara || ventana.errorCamara !== ""
        spacing: 12
        Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 56; Layout.preferredHeight: 56; nombre: "sinCamara"; color: "#c9ccd8"; trazo: 1.5 }
        Text {
            objectName: "mensajeSinCamara"
            Layout.alignment: Qt.AlignHCenter
            text: ventana.errorCamara !== "" ? ventana.errorCamara : "No encuentro tu cámara"
            color: "#ffffff"
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: ventana.errorCamara !== "" ? "Ciérrala o revisa el cable, y reintenta." : "Conéctala y aparece aquí sola."
            color: "#c9ccd8"
            font.pixelSize: 14
        }
        AbstractButton {
            Layout.alignment: Qt.AlignHCenter
            visible: ventana.errorCamara !== ""
            implicitWidth: 130; implicitHeight: 40
            hoverEnabled: true
            onClicked: ventana.errorCamara = ""
            background: Rectangle { radius: 20; color: parent.hovered ? Tema.azulHover : Tema.azul }
            contentItem: Text { text: "Reintentar"; color: Tema.sobreAcento; font.pixelSize: 14; font.weight: Font.DemiBold; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
        }
    }

    Text {   // la cuenta del temporizador
        anchors.centerIn: parent
        anchors.horizontalCenterOffset: -48
        visible: ventana.cuenta > 0
        text: ventana.cuenta
        color: "#ffffff"
        font.pixelSize: 120
        font.weight: Font.Bold
        style: Text.Outline
        styleColor: Qt.rgba(0, 0, 0, 0.5)
    }

    // Arriba a la izquierda: ajustes y temporizador
    Row {
        anchors { left: parent.left; top: parent.top; margins: 16 }
        spacing: 10
        BotonRedondo {
            icono: "ajustes"
            ayuda: "Ajustes"
            enabled: !ventana.grabando
            onClicked: ventana.panelAbierto = true
        }
        BotonRedondo {
            icono: "reloj"
            etiqueta: ventana.temporizador > 0 ? ventana.temporizador + " s" : ""
            ayuda: ventana.temporizador > 0 ? "Temporizador: " + ventana.temporizador + " segundos" : "Temporizador apagado"
            elegido: ventana.temporizador > 0
            enabled: !ventana.grabando && ventana.cuenta === 0
            onClicked: {
                var siguientes = {0: 3, 3: 5, 5: 10, 10: 0}
                ventana.temporizador = siguientes[ventana.temporizador]
                vv.guardar("temporizador", ventana.temporizador)
            }
        }
    }

    // Arriba al centro: el tiempo de grabación
    Rectangle {
        anchors.top: parent.top
        anchors.topMargin: 20
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.horizontalCenterOffset: -48
        visible: ventana.grabando
        width: filaTiempo.implicitWidth + 28
        height: 36
        radius: 18
        color: Qt.rgba(0, 0, 0, 0.6)
        Row {
            id: filaTiempo
            anchors.centerIn: parent
            spacing: 8
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 12; height: 12; radius: 6
                color: "#ff4d5e"
                SequentialAnimation on opacity {
                    running: ventana.grabando && !ventana.enPausa
                    loops: Animation.Infinite
                    onRunningChanged: if (!running) parent.opacity = 1
                    NumberAnimation { to: 0.3; duration: 600 }
                    NumberAnimation { to: 1; duration: 600 }
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: vv.duracion(grabadora.duration)
                color: "#ffffff"
                font.pixelSize: 15
                font.features: { "tnum": 1 }
            }
        }
    }

    // A la derecha: como en la Cámara de Windows
    Rectangle {
        id: riel
        anchors { right: parent.right; top: parent.top; bottom: parent.bottom }
        width: 104
        color: Qt.rgba(0, 0, 0, 0.45)

        BotonRedondo {
            anchors { top: parent.top; horizontalCenter: parent.horizontalCenter; topMargin: 16 }
            visible: ventana.camaras.length > 1
            icono: "cambiar"
            ayuda: "Cambiar de cámara"
            enabled: !ventana.grabando
            onClicked: ventana.cambiarCamara()
        }

        Column {
            anchors.centerIn: parent
            spacing: 16
            BotonRedondo {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: !ventana.grabando
                icono: "foto"
                ayuda: "Foto"
                elegido: ventana.modo === "foto"
                onClicked: { ventana.modo = "foto"; vv.guardar("modo", "foto") }
            }
            BotonRedondo {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: !ventana.grabando
                icono: "video"
                ayuda: "Video"
                elegido: ventana.modo === "video"
                onClicked: { ventana.modo = "video"; vv.guardar("modo", "video") }
            }
            BotonRedondo {
                anchors.horizontalCenter: parent.horizontalCenter
                visible: ventana.grabando
                icono: ventana.enPausa ? "play" : "pausa"
                ayuda: ventana.enPausa ? "Seguir grabando" : "Pausar"
                onClicked: ventana.enPausa ? grabadora.record() : grabadora.pause()
            }
            Disparador {
                objectName: "disparador"
                anchors.horizontalCenter: parent.horizontalCenter
                video: ventana.modo === "video"
                grabando: ventana.grabando
                enabled: ventana.lista
                onClicked: ventana.disparar()
            }
        }

        // La última captura: al picarle, se abre
        AbstractButton {
            anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter; bottomMargin: 18 }
            visible: vv.ultima !== ""
            implicitWidth: 60; implicitHeight: 60
            hoverEnabled: true
            focusPolicy: Qt.NoFocus
            Accessible.name: "Abrir la última captura"
            onClicked: vv.abrirUltima()
            background: Rectangle {
                radius: 12
                color: "#20232e"
                border.width: 2
                border.color: parent.hovered ? "#ffffff" : Qt.rgba(1, 1, 1, 0.5)
                clip: true
                Image {
                    anchors.fill: parent
                    anchors.margins: 2
                    source: vv.miniatura
                    sourceSize.width: 120
                    sourceSize.height: 120
                    fillMode: Image.PreserveAspectCrop
                    asynchronous: true
                    cache: false
                }
                Icono {
                    anchors.centerIn: parent
                    visible: vv.ultimaEsVideo
                    width: 22; height: 22
                    nombre: "play"
                    color: "#ffffff"
                }
            }
        }
    }

    Rectangle {   // avisos cortos
        id: avisoCaja
        anchors { bottom: parent.bottom; horizontalCenter: parent.horizontalCenter; horizontalCenterOffset: -48; bottomMargin: 24 }
        width: avisoTexto.implicitWidth + 32
        height: 40
        radius: 20
        color: Qt.rgba(0, 0, 0, 0.75)
        opacity: 0
        Behavior on opacity { NumberAnimation { duration: 180 } }
        Text { id: avisoTexto; anchors.centerIn: parent; color: "#ffffff"; font.pixelSize: 14 }
        Timer { id: avisoReloj; interval: 3500; onTriggered: avisoCaja.opacity = 0 }
    }

    MouseArea {   // picar fuera cierra el panel
        anchors.fill: parent
        visible: ventana.panelAbierto
        onClicked: ventana.panelAbierto = false
    }
    PanelAjustes {
        objectName: "panelAjustes"
        width: Math.min(360, ventana.width - 120)
        anchors { top: parent.top; bottom: parent.bottom }
        x: ventana.panelAbierto ? 0 : -width - 2
        visible: x > -width
        Behavior on x { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
        modo: ventana.modo
        camaras: ventana.camaras.map((c) => c.description)
        camaraIndice: ventana.camaraActual ? ventana.camaras.findIndex((c) => c.description === ventana.camaraActual.description) : -1
        resoluciones: ventana.resoluciones
        resolucionIndice: {
            var i = ventana.formatosPlanos.length ? vv.elegirFormato(ventana.formatosPlanos, ventana.resolucionPreferida) : -1
            if (i < 0) return -1
            var f = ventana.formatosPlanos[i]
            return ventana.resoluciones.indexOf(f.ancho + "x" + f.alto)
        }
        microfono: ventana.microfono
        cuadricula: ventana.cuadricula
        onCerrar: ventana.panelAbierto = false
        onElegirCamara: (i) => { ventana.nombreCamara = ventana.camaras[i].description; vv.guardar("camara", ventana.nombreCamara) }
        onElegirResolucion: (i) => { ventana.resolucionPreferida = ventana.resoluciones[i]; vv.guardar("resolucion", ventana.resolucionPreferida) }
        onCambiarMicrofono: (si) => { ventana.microfono = si; vv.guardar("microfono", si) }
        onCambiarCuadricula: (si) => { ventana.cuadricula = si; vv.guardar("cuadricula", si) }
    }

    Shortcut { sequences: ["Space", "Return"]; enabled: !ventana.panelAbierto; onActivated: ventana.disparar() }
    Shortcut {
        sequence: "Escape"
        onActivated: {
            if (ventana.panelAbierto) ventana.panelAbierto = false
            else if (ventana.cuenta > 0) ventana.disparar()
        }
    }
    Shortcut { sequence: "F11"; onActivated: ventana.visibility = ventana.visibility === Window.FullScreen ? Window.Windowed : Window.FullScreen }
    Shortcut { sequence: "Ctrl+W"; onActivated: ventana.close() }
    Connections {
        target: vv
        function onMostrarVentana() { ventana.show(); ventana.raise(); ventana.requestActivate() }
    }
}
