// Portada de una canción (la sacan vp.portada / el hilo de ffmpeg); sin portada, una nota musical.
import QtQuick

Rectangle {
    id: raiz
    property string ruta: ""
    property string url: ruta ? vp.portada(ruta) : ""
    property int radio: 6
    radius: radio
    color: Tema.tarjeta
    clip: true
    onRutaChanged: url = ruta ? vp.portada(ruta) : ""
    Connections {
        target: vp
        function onPortadaLista(r) { if (r === raiz.ruta) raiz.url = vp.portada(raiz.ruta) }
    }
    Image {
        id: img
        anchors.fill: parent
        source: raiz.url
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: true
        sourceSize.width: Math.ceil(raiz.width * 2)
        sourceSize.height: Math.ceil(raiz.height * 2)
        visible: status === Image.Ready
    }
    Icono {
        visible: !img.visible
        anchors.centerIn: parent
        width: Math.round(raiz.width * 0.42)
        height: width
        nombre: "musica"
        color: Tema.tenue
    }
}
