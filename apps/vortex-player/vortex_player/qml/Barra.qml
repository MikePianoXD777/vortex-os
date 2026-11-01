// La barra del reproductor, abajo: la canción con su calidad, los controles con la barra de tiempo y,
// a la derecha, por dónde suena (con el códec de los audífonos Bluetooth) y el volumen.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Effects

Rectangle {
    id: raiz
    color: Tema.barra
    implicitHeight: 92
    property bool letraAbierta: false
    property bool colaAbierta: false
    property bool miniAbierto: false
    signal alternarLetra()
    signal alternarCola()
    signal alternarMini()

    // 2.4: al cambiar de canción, la portada y el título entran deslizándose con desenfoque
    Connections {
        target: rep
        function onCambioCancion() { cambioCancion.restart() }
    }

    function tiempo(s) {
        s = Math.max(0, Math.floor(s || 0))
        var m = Math.floor(s / 60), r = s % 60
        return m + ":" + (r < 10 ? "0" : "") + r
    }

    function textoSalida() {
        var s = vp.salida
        if (!s || !s.nombre) return ""
        var partes = [s.nombre]
        if (s.bluetooth && s.codecTexto) partes.push(s.codecTexto + (s.codec === "ldac" && vp.ajustes.ldac_hq ? " 990" : ""))
        if (s.hz) partes.push((s.hz / 1000).toLocaleString(Qt.locale(), "f", s.hz % 1000 ? 1 : 0) + " kHz")
        return partes.join(" · ")
    }

    Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: Tema.borde; opacity: 0.6 }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 16
        anchors.rightMargin: 20
        spacing: 16

        // --- la canción ---
        RowLayout {
            id: laCancion
            Layout.preferredWidth: parent.width * 0.3
            Layout.maximumWidth: parent.width * 0.3
            spacing: 12
            property real desenfoque: 0
            layer.enabled: cambioCancion.running
            layer.effect: MultiEffect { blurEnabled: true; blurMax: 24; blur: laCancion.desenfoque }
            transform: Translate { id: corrimiento }
            ParallelAnimation {
                id: cambioCancion
                NumberAnimation { target: laCancion; property: "opacity"; from: 0; to: 1; duration: 380; easing.type: Easing.OutCubic }
                NumberAnimation { target: corrimiento; property: "x"; from: 18; to: 0; duration: 420; easing.type: Easing.OutCubic }
                NumberAnimation { target: laCancion; property: "desenfoque"; from: 1; to: 0; duration: 380; easing.type: Easing.OutCubic }
            }
            Portada {
                Layout.preferredWidth: 60
                Layout.preferredHeight: 60
                ruta: rep.cancion.ruta || ""
                radio: 8
            }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 3
                Text {
                    Layout.fillWidth: true
                    text: rep.cancion.titulo || "Nada sonando"
                    color: Tema.texto
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }
                Text {
                    Layout.fillWidth: true
                    text: rep.cancion.artista || (rep.cancion.ruta ? "Artista desconocido" : "Elige una canción")
                    color: Tema.tenue
                    font.pixelSize: 12
                    elide: Text.ElideRight
                }
                Etiqueta {
                    texto: rep.cancion.radio ? "Radio" : (rep.cancion.etiqueta || "")   // 1.4: suena de YouTube Music
                    nivel: rep.cancion.nivel || "comprimida"
                }
            }
            BotonIcono {   // 1.4: la de la radio todavía no es tuya: se baja (y luego ya le puedes dar ❤️)
                visible: !!rep.cancion.radio
                tam: 32
                icono: vp.bajandoRadio.indexOf(rep.cancion.ruta || "") >= 0 ? "visto" : "descargar"
                ayuda: "Bajar a tu biblioteca"
                onClicked: vp.bajarDeLaRadio(rep.cancion.ruta)
            }
            BotonIcono {   // 2.4: ❤️ como en Spotify, junto a la canción
                id: corazon
                readonly property bool gusta: vp.meGusta.indexOf(rep.cancion.ruta || "") >= 0
                visible: !!rep.cancion.ruta && !rep.cancion.radio
                tam: 32
                icono: gusta ? "corazonLleno" : "corazon"
                activo: gusta
                ayuda: gusta ? "Quitar de Tus me gusta" : "Guardar en Tus me gusta"
                onClicked: { vp.alternarMeGusta(rep.cancion.ruta); latido.restart() }
                SequentialAnimation {
                    id: latido
                    NumberAnimation { target: corazon; property: "scale"; to: 1.3; duration: 120; easing.type: Easing.OutQuad }
                    NumberAnimation { target: corazon; property: "scale"; to: 1; duration: 260; easing.type: Easing.OutBack; easing.overshoot: 3 }
                }
            }
        }

        // --- controles ---
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 2
            RowLayout {
                Layout.alignment: Qt.AlignHCenter
                spacing: 10
                BotonIcono { icono: "aleatorio"; activo: rep.aleatorio; tam: 34; ayuda: "Aleatorio"; onClicked: rep.alternarAleatorio() }
                BotonIcono { icono: "anterior"; tam: 36; enabled: rep.hayAnterior; ayuda: "Anterior"; onClicked: rep.anterior() }
                BotonIcono {
                    icono: rep.pausado ? "play" : "pausa"
                    relleno: true
                    tam: 42
                    ayuda: rep.pausado ? "Reproducir (espacio)" : "Pausa (espacio)"
                    onClicked: rep.cancion.ruta ? rep.alternar() : vp.tocar(0)
                }
                BotonIcono { icono: "siguiente"; tam: 36; enabled: rep.haySiguiente; ayuda: "Siguiente"; onClicked: rep.siguiente() }
                BotonIcono {
                    icono: "repetir"
                    activo: rep.repetir !== "no"
                    tam: 34
                    ayuda: rep.repetir === "una" ? "Repetir esta canción" : rep.repetir === "todo" ? "Repetir todo" : "Repetir"
                    onClicked: rep.alternarRepetir()
                    Text {
                        visible: rep.repetir === "una"
                        text: "1"
                        color: Tema.azul
                        font.pixelSize: 9
                        font.weight: Font.Bold
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.rightMargin: 4
                    }
                }
            }
            RowLayout {
                Layout.fillWidth: true
                Layout.maximumWidth: 640
                Layout.alignment: Qt.AlignHCenter
                spacing: 10
                Text { text: raiz.tiempo(tiempoBarra.pressed ? tiempoBarra.value : rep.posicion); color: Tema.tenue; font.pixelSize: 11; font.features: { "tnum": 1 } }
                Slider {
                    id: tiempoBarra
                    Layout.fillWidth: true
                    from: 0
                    to: Math.max(1, rep.duracion)
                    enabled: !!rep.cancion.ruta
                    hoverEnabled: true
                    onMoved: {}
                    onPressedChanged: if (!pressed) rep.buscar(value)
                    Binding on value { value: rep.posicion; when: !tiempoBarra.pressed }
                    background: Rectangle {
                        x: tiempoBarra.leftPadding
                        y: tiempoBarra.topPadding + tiempoBarra.availableHeight / 2 - height / 2
                        width: tiempoBarra.availableWidth
                        height: 4
                        radius: 2
                        color: Tema.borde
                        Rectangle {
                            width: tiempoBarra.visualPosition * parent.width
                            height: parent.height
                            radius: 2
                            color: tiempoBarra.hovered || tiempoBarra.pressed ? Tema.azul : Tema.texto
                        }
                    }
                    handle: Rectangle {
                        x: tiempoBarra.leftPadding + tiempoBarra.visualPosition * (tiempoBarra.availableWidth - width)
                        y: tiempoBarra.topPadding + tiempoBarra.availableHeight / 2 - height / 2
                        width: 12; height: 12; radius: 6
                        color: Tema.texto
                        visible: tiempoBarra.hovered || tiempoBarra.pressed
                    }
                }
                Text { text: raiz.tiempo(rep.duracion); color: Tema.tenue; font.pixelSize: 11; font.features: { "tnum": 1 } }
            }
        }

        // --- salida y volumen ---
        RowLayout {
            Layout.preferredWidth: parent.width * 0.3
            Layout.maximumWidth: parent.width * 0.3
            spacing: 8
            Item { Layout.fillWidth: true }
            BotonIcono { icono: "letra"; tam: 32; activo: raiz.letraAbierta; ayuda: "Letra (Ctrl+Y)"; enabled: !!rep.cancion.ruta; onClicked: raiz.alternarLetra() }
            BotonIcono { icono: "cola"; tam: 32; activo: raiz.colaAbierta; ayuda: "A continuación (Ctrl+Q)"; onClicked: raiz.alternarCola() }
            BotonIcono {   // 2.4: temporizador para dormir
                id: botonDormir
                icono: "luna"; tam: 32; activo: rep.dormirActivo
                ayuda: rep.dormirActivo ? "Se pausa en " + rep.dormirTexto : "Temporizador para dormir"
                onClicked: menuDormir.popup(botonDormir, 0, -menuDormir.implicitHeight - 6)
                Text {
                    visible: rep.dormirActivo && rep.dormirTexto.indexOf(":") > 0
                    text: rep.dormirTexto.split(":")[0]
                    color: Tema.azul
                    font.pixelSize: 9
                    font.weight: Font.Bold
                    anchors.right: parent.right
                    anchors.top: parent.top
                }
                Menu {
                    id: menuDormir
                    MenuItem { enabled: false; text: rep.dormirActivo ? "Se pausa en " + rep.dormirTexto : "Pausar la música en…" }
                    Repeater {
                        model: [5, 15, 30, 45, 60, 90]
                        MenuItem { required property int modelData; text: modelData + " minutos"; onTriggered: rep.dormir(modelData) }
                    }
                    MenuItem { text: "Al terminar la canción"; onTriggered: rep.dormir(-1) }
                    MenuItem { visible: rep.dormirActivo; height: visible ? implicitHeight : 0; text: "Apagar el temporizador"; onTriggered: rep.dormir(0) }
                }
            }
            BotonIcono { icono: "mini"; tam: 32; activo: raiz.miniAbierto; ayuda: "Mini reproductor (Ctrl+M)"; onClicked: raiz.alternarMini() }
            Rectangle {
                id: chip
                visible: raiz.textoSalida() !== ""
                Layout.maximumWidth: 260
                implicitHeight: 28
                implicitWidth: filaChip.implicitWidth + 20
                radius: 14
                color: vp.salida.codec === "ldac" ? Qt.rgba(0.95, 0.78, 0.42, 0.14) : Tema.realce
                border.width: 1
                border.color: vp.salida.codec === "ldac" ? Qt.rgba(0.95, 0.78, 0.42, 0.45) : "transparent"
                RowLayout {
                    id: filaChip
                    anchors.centerIn: parent
                    width: Math.min(implicitWidth, chip.width - 20)
                    spacing: 6
                    Icono {
                        Layout.preferredWidth: 15
                        Layout.preferredHeight: 15
                        nombre: vp.salida.bluetooth ? "audifonos" : "bocina"
                        color: vp.salida.codec === "ldac" ? "#f2c86b" : Tema.suave
                    }
                    Text {
                        Layout.fillWidth: true
                        text: raiz.textoSalida()
                        color: vp.salida.codec === "ldac" ? "#f2c86b" : Tema.suave
                        font.pixelSize: 12
                        elide: Text.ElideRight
                    }
                }
                ToolTip.visible: zonaChip.containsMouse
                ToolTip.delay: 500
                ToolTip.text: vp.salida.bluetooth
                    ? (vp.salida.codec === "ldac" ? "Suena en LDAC: lo más alto que da el Bluetooth (hasta 24 bits / 96 kHz)"
                       : vp.salida.ldac ? "Tus audífonos aceptan LDAC: clic para cambiar a LDAC"
                       : "Códec Bluetooth: " + vp.salida.codecTexto)
                    : "Por aquí está sonando"
                MouseArea {
                    id: zonaChip
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: vp.salida.ldac && vp.salida.codec !== "ldac" ? Qt.PointingHandCursor : Qt.ArrowCursor
                    onClicked: if (vp.salida.ldac && vp.salida.codec !== "ldac") vp.forzarLdac()
                }
            }
            BotonIcono {
                icono: volumen.value === 0 ? "silencio" : "volumen"
                tam: 32
                property int antes: 80
                onClicked: {
                    if (volumen.value > 0) { antes = volumen.value; vp.guardarVolumen(0) }
                    else vp.guardarVolumen(antes || 80)
                }
            }
            Slider {
                id: volumen
                Layout.preferredWidth: 110
                from: 0; to: 100; stepSize: 1
                hoverEnabled: true
                value: rep.volumen
                onMoved: vp.guardarVolumen(value)
                background: Rectangle {
                    x: volumen.leftPadding
                    y: volumen.topPadding + volumen.availableHeight / 2 - height / 2
                    width: volumen.availableWidth
                    height: 4; radius: 2
                    color: Tema.borde
                    Rectangle {
                        width: volumen.visualPosition * parent.width
                        height: parent.height; radius: 2
                        color: volumen.hovered || volumen.pressed ? Tema.azul : Tema.texto
                    }
                }
                handle: Rectangle {
                    x: volumen.leftPadding + volumen.visualPosition * (volumen.availableWidth - width)
                    y: volumen.topPadding + volumen.availableHeight / 2 - height / 2
                    width: 12; height: 12; radius: 6
                    color: Tema.texto
                    visible: volumen.hovered || volumen.pressed
                }
                WheelHandler { onWheel: (e) => vp.guardarVolumen(Math.max(0, Math.min(100, volumen.value + (e.angleDelta.y > 0 ? 5 : -5)))) }
            }
        }
    }
}
