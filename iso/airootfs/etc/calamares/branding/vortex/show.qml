/* Diapositivas de Vortexine's OS mientras se instala (Calamares, slideshowAPI 2).
   Diseño: la pantalla «3 · Instalando» del lienzo «Instalador de Vortexine's OS» (Claude Design):
   una tarjeta con el logo, el número de diapositiva, el título, el texto y los puntos. */
import QtQuick 2.15
import calamares.slideshow 1.0

Presentation {
    id: presentation
    property var textos: [
        ["Las funciones de los grandes, fáciles de usar", "La capacidad sí, la complejidad no."],
        ["Hecho para jugar", "Kernel de CachyOS, planificador scx_lavd, modo juego automático y Proton listo."],
        ["Ligero de verdad", "En reposo usa casi nada de CPU: tu PC queda libre para lo que importa."],
        ["Tus colores, tu fondo", "Cambia el fondo y todo el escritorio toma sus colores."],
        ["Tus pantallas a toda velocidad", "Cada monitor a su frecuencia más alta, hasta 240 Hz, sin configurar nada."]
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
                id: fondo
                // El número de esta diapositiva (dentro del Repeater de los puntos, «index» es el del punto).
                property int indice: index
                anchors.fill: parent
                color: "#0b0d1a"
                Rectangle {
                    anchors.fill: parent
                    anchors.margins: 24
                    radius: 24
                    color: "#141833"
                    border.color: "#232850"
                    border.width: 1
                    Column {
                        anchors.centerIn: parent
                        width: parent.width * 0.78
                        spacing: 20
                        Image {
                            anchors.horizontalCenter: parent.horizontalCenter
                            source: "vortex-os.svg"
                            sourceSize.width: 112
                            sourceSize.height: 112
                            width: 112
                            height: 112
                        }
                        Text {
                            width: parent.width
                            text: (index + 1) + " de " + presentation.textos.length
                            color: "#c6a8ff"
                            font.family: "Adwaita Sans"
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            font.letterSpacing: 1.6
                            font.capitalization: Font.AllUppercase
                            horizontalAlignment: Text.AlignHCenter
                        }
                        Text {
                            width: parent.width
                            text: modelData[0]
                            color: "#e8ebff"
                            font.family: "Adwaita Sans"
                            font.pixelSize: 34
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                        }
                        Text {
                            width: parent.width
                            text: modelData[1]
                            color: "#c3c8ea"
                            font.family: "Adwaita Sans"
                            font.pixelSize: 17
                            lineHeight: 1.3
                            wrapMode: Text.WordWrap
                            horizontalAlignment: Text.AlignHCenter
                        }
                        Row {
                            anchors.horizontalCenter: parent.horizontalCenter
                            spacing: 8
                            Repeater {
                                model: presentation.textos.length
                                Rectangle {
                                    width: index === fondo.indice ? 24 : 8
                                    height: 8
                                    radius: 4
                                    color: index === fondo.indice ? "#9ec5ff" : "#3a4278"
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    function onActivate() { presentation.currentSlide = 0; }
    function onLeave() { }
}
