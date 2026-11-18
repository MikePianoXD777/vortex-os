// Ícono de trazo (SVG en línea, de la colección de Iconos.qml) del color que se pida.
// Es un Item con la imagen dentro (no una Image suelta): así su tamaño propio es fijo y, dentro de un
// Layout, el ancho no cambia la imagen ni la imagen el ancho (eso hacía un bucle). En un Layout se usa
// Layout.preferredWidth para cambiarle el tamaño.
import QtQuick

Item {
    id: raiz
    property string nombre: ""
    property color color: Tema.texto
    property real trazo: 2
    width: 20
    height: 20
    implicitWidth: 20
    implicitHeight: 20
    Image {
        anchors.fill: parent
        sourceSize.width: Math.ceil(raiz.width * 2)    // nítido en pantallas con escala
        sourceSize.height: Math.ceil(raiz.height * 2)
        fillMode: Image.PreserveAspectFit
        smooth: true
        source: raiz.nombre === "" ? "" : Iconos.uri(raiz.nombre, raiz.color, raiz.trazo)
    }
}
