// Inicio: bienvenida, «Recomendadas para Vortexine's» (fila que se desplaza), «Apps de Vortexine's»
// (las nuestras que no vienen de fábrica) y «Populares» (cuadrícula).
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property bool cargando: tienda.inicioEstado === "cargando"
    readonly property bool conProblema: tienda.inicioEstado === "sinred" || tienda.inicioEstado === "error"

    Rectangle {   // bienvenida
        Layout.fillWidth: true
        implicitHeight: Math.max(148, bienvenida.implicitHeight + 56)
        radius: Tema.radioGrande
        border.width: 1
        border.color: Tema.borde
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: Tema.realce }
            GradientStop { position: 1; color: Tema.tarjeta }
        }
        Image {
            anchors.right: parent.right
            anchors.rightMargin: 28
            anchors.verticalCenter: parent.verticalCenter
            width: 104
            height: 104
            source: "vortex-os.svg"
            sourceSize.width: 208
            sourceSize.height: 208
            smooth: true
            visible: parent.width > 560
        }
        Column {
            id: bienvenida
            anchors.left: parent.left
            anchors.leftMargin: 30
            anchors.right: parent.right
            anchors.rightMargin: parent.width > 560 ? 160 : 30
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Text {
                text: "Vortexine's Shop"
                color: Tema.violeta
                font.pixelSize: 12
                font.weight: Font.DemiBold
                font.letterSpacing: 1.4
                font.capitalization: Font.AllUppercase
            }
            Text {
                width: parent.width
                text: "Todo lo que necesitas, a un clic"
                color: Tema.texto
                font.pixelSize: 28
                font.weight: Font.DemiBold
                wrapMode: Text.WordWrap
            }
            Text {
                width: parent.width
                text: "Miles de apps y juegos de Flathub, listos para instalar. Sin terminal y sin contraseñas."
                color: Tema.suave
                font.pixelSize: 15
                wrapMode: Text.WordWrap
            }
        }
    }

    EstadoPantalla {
        Layout.fillWidth: true
        visible: raiz.conProblema
        icono: tienda.inicioEstado === "sinred" ? "sinred" : "alerta"
        titulo: tienda.inicioEstado === "sinred" ? "Sin conexión a internet" : "No pudimos cargar las apps"
        texto: tienda.inicioEstado === "sinred" ? "Revisa tu red y vuelve a intentarlo."
                                                : "Algo salió mal al pedir el catálogo. Vuelve a intentarlo en un momento."
        boton: "Reintentar"
        onAccion: tienda.reintentarTodo()
    }

    Seccion {
        Layout.fillWidth: true
        visible: !raiz.conProblema && (raiz.cargando || tienda.recomendadas.length > 0)
        etiqueta: "Vortexine's"
        titulo: "Recomendadas para Vortexine's"
        subtitulo: "Las que elegimos para que empieces con buen pie"
        derecha: [
            Flecha { sentido: "izquierda"; etiqueta: "Anteriores"; activo: fila.contentX > 4; onClicked: fila.mover(-1) },
            Flecha { sentido: "derecha"; etiqueta: "Siguientes"; activo: fila.contentX < fila.contentWidth - fila.width - 4; onClicked: fila.mover(1) }
        ]
        ListView {
            id: fila
            Layout.fillWidth: true
            Layout.preferredHeight: 152
            orientation: ListView.Horizontal
            spacing: 16
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            model: raiz.cargando ? [{}, {}, {}, {}] : tienda.recomendadas
            delegate: Loader {
                required property var modelData
                width: 320
                height: 152
                sourceComponent: modelData.id ? tarjeta : esqueleto
                Component { id: tarjeta; Tarjeta { app: modelData } }
                Component { id: esqueleto; Esqueleto {} }
            }
            NumberAnimation { id: desliz; target: fila; property: "contentX"; duration: 260; easing.type: Easing.InOutCubic }
            function mover(sentido) {
                desliz.stop()
                desliz.to = Math.max(0, Math.min(contentWidth - width, contentX + sentido * (width * 0.8)))
                desliz.start()
            }
        }
    }

    // Las apps propias no dependen de Flathub: se ven aunque no haya internet.
    Seccion {
        Layout.fillWidth: true
        visible: tienda.propias.length > 0
        etiqueta: "Hecho en Vortexine's"
        titulo: "Apps de Vortexine's"
        subtitulo: "No vienen de fábrica: instálalas solo si las necesitas"
        Repeater {
            model: tienda.propias
            TarjetaPropia {
                required property var modelData
                Layout.fillWidth: true
                app: modelData
            }
        }
    }

    Seccion {
        Layout.fillWidth: true
        visible: !raiz.conProblema
        titulo: "Populares"
        subtitulo: "Lo que más instala la gente"
        Rejilla {
            Layout.fillWidth: true
            cargando: raiz.cargando
            huecos: 6
            apps: tienda.populares
        }
    }
}
