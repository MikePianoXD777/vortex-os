// La demo del paso 0: Tema.qml con la paleta real. Con Colores.captura (VORTEX_DEMO_CAPTURA) guarda una
// captura y se cierra, para probarla sin pantalla.
import QtQuick
import QtQuick.Window
import Vortex.Colores
import Vortex.Tema

Window {
    id: ventana
    width: 640
    height: 400
    visible: true
    title: "Vortexine · demo de Tema (Rust + cxx-qt)"
    color: Tema.fondo

    Column {
        anchors.centerIn: parent
        spacing: 16

        Text {
            text: "Vortexine en Rust: paso 0"
            color: Tema.texto
            font.pixelSize: 26
            font.bold: true
        }
        Text {
            text: "Los colores salen del fondo de pantalla (o de la paleta de respaldo)."
            color: Tema.suave
            font.pixelSize: 14
        }
        Row {
            spacing: 10
            Repeater {
                model: ["fondo", "barra", "tarjeta", "campo", "borde", "texto", "azul", "violeta", "peligro", "exito"]
                delegate: Column {
                    spacing: 4
                    Rectangle {
                        width: 48; height: 48; radius: 10
                        color: Tema[modelData]
                        border.color: Tema.borde
                    }
                    Text { text: modelData; color: Tema.tenue; font.pixelSize: 10 }
                }
            }
        }
        Rectangle {
            width: 200; height: 40; radius: 20
            color: Tema.azul
            Text { anchors.centerIn: parent; text: "Botón de acento"; color: Tema.sobreAcento; font.bold: true }
        }
    }

    Timer {
        interval: 800
        running: Colores.captura !== ""
        onTriggered: ventana.contentItem.grabToImage(function (r) {
            r.saveToFile(Colores.captura);
            Qt.quit();
        })
    }
}
