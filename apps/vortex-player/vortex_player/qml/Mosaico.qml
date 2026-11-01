// 2.4: la portada de una playlist: con 4 canciones o más, un mosaico de las primeras 4 (como Spotify);
// con menos, la portada de la primera; vacía, el ícono de playlist. 2.4: «Tus me gusta» lleva su propia
// portada, un degradado con el corazón, como en Spotify.
import QtQuick

Rectangle {
    id: raiz
    property var rutas: []
    property int radio: 6
    property bool meGusta: false
    radius: radio
    color: Tema.realceFuerte
    clip: true
    Rectangle {
        visible: raiz.meGusta
        anchors.fill: parent
        z: 2
        radius: raiz.radio
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop { position: 0; color: Qt.darker(Tema.azul, 1.6) }
            GradientStop { position: 1; color: Qt.lighter(Tema.violeta, 1.15) }
        }
        Icono { anchors.centerIn: parent; width: Math.round(raiz.width * 0.42); height: width; nombre: "corazonLleno"; color: "white" }
    }
    Icono {
        visible: raiz.rutas.length === 0
        anchors.centerIn: parent
        width: Math.round(raiz.width * 0.42); height: width
        nombre: "playlist"
        color: Tema.tenue
    }
    Portada {
        visible: raiz.rutas.length > 0 && raiz.rutas.length < 4
        anchors.fill: parent
        ruta: raiz.rutas.length ? raiz.rutas[0] : ""
        radio: raiz.radio
    }
    Grid {
        visible: raiz.rutas.length >= 4
        anchors.fill: parent
        columns: 2
        Repeater {
            model: raiz.rutas.length >= 4 ? 4 : 0
            Portada { required property int index; width: raiz.width / 2; height: raiz.height / 2; radio: 0; ruta: raiz.rutas[index] }
        }
    }
}
