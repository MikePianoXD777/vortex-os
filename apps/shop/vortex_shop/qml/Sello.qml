// Sello de «verificado» (el desarrollador es quien publica la app en Flathub).
import QtQuick

Rectangle {
    id: raiz
    property int tam: 16
    width: tam
    height: tam
    radius: tam / 2
    color: Tema.azul
    Icono {
        anchors.centerIn: parent
        width: raiz.tam * 0.72
        height: width
        nombre: "visto"
        color: Tema.sobreAcento
        trazo: 3.2
    }
}
