// La barra del reproductor, abajo: la canción con su calidad, los controles con la barra de tiempo y,
// a la derecha, por dónde suena (con el códec de los audífonos Bluetooth) y el volumen.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: raiz
    color: Tema.barra
    implicitHeight: 92

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
            Layout.preferredWidth: parent.width * 0.3
            Layout.maximumWidth: parent.width * 0.3
            spacing: 12
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
                    texto: rep.cancion.etiqueta || ""
                    nivel: rep.cancion.nivel || "comprimida"
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
