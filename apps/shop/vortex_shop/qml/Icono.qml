// Ícono de trazo (SVG en línea, de la colección de Iconos.qml) del color que se pida.
import QtQuick

Image {
    id: raiz
    property string nombre: ""
    property color color: Tema.texto
    property real trazo: 2
    width: 20
    height: 20
    sourceSize.width: Math.ceil(width * 2)    // nítido en pantallas con escala
    sourceSize.height: Math.ceil(height * 2)
    fillMode: Image.PreserveAspectFit
    smooth: true
    source: nombre === "" ? "" : Iconos.uri(nombre, color, trazo)
}
