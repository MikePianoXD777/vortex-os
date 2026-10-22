/* Diapositivas de Vortex OS mientras se instala (Calamares, slideshowAPI 2). */
import QtQuick 2.15
import calamares.slideshow 1.0

Presentation {
    id: presentation
    property var textos: [
        ["Las funciones de los grandes, fáciles de usar", "La capacidad sí, la complejidad no."],
        ["Hecho para jugar", "Kernel de CachyOS, planificador scx_lavd, modo juego automático y Proton listo."],
        ["Ligero de verdad", "En reposo usa casi nada de CPU: tu PC queda libre para lo que importa."],
        ["Tus colores, tu fondo", "Cambia el fondo y todo el escritorio toma sus colores."],
        ["Claude integrado", "Super+A y le hablas con el micrófono. Solo mira tu pantalla si se lo pides."]
    ]

    Timer {
        interval: 7000
        running: presentation.activatedInCalamares
        repeat: true
        onTriggered: presentation.goToNextSlide()
    }

    Repeater {
        model: presentation.textos
        Slide {
            Rectangle {
                anchors.fill: parent
                color: "#0a0c18"
                Column {
                    anchors.centerIn: parent
                    width: parent.width * 0.8
                    spacing: 18
                    Text {
                        width: parent.width
                        text: modelData[0]
                        color: "#9ec5ff"
                        font.pixelSize: 30
                        font.bold: true
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }
                    Text {
                        width: parent.width
                        text: modelData[1]
                        color: "#e6e9ff"
                        font.pixelSize: 18
                        wrapMode: Text.WordWrap
                        horizontalAlignment: Text.AlignHCenter
                    }
                }
            }
        }
    }

    function onActivate() { presentation.currentSlide = 0; }
    function onLeave() { }
}
