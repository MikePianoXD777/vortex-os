// Ícono de una app, cargado de su URL. Mientras carga (o si falla) se ve la inicial en los colores del
// tema (en el diseño de fábrica de la 4.0, blanco sobre gris: nada de colores sueltos).
import QtQuick

Item {
    id: raiz
    property string url: ""
    property string nombre: ""
    property int tam: 56
    width: tam
    height: tam

    Rectangle {
        anchors.fill: parent
        radius: raiz.tam * 0.24
        color: Tema.realce
        border.width: 1
        border.color: Tema.borde
        visible: imagen.status !== Image.Ready
        Text {
            anchors.centerIn: parent
            text: raiz.nombre.length > 0 ? raiz.nombre.charAt(0).toUpperCase() : "?"
            color: Tema.texto
            font.pixelSize: raiz.tam * 0.44
            font.weight: Font.DemiBold
        }
    }
    Image {
        id: imagen
        anchors.fill: parent
        source: raiz.url
        asynchronous: true
        cache: true
        smooth: true
        mipmap: true
        fillMode: Image.PreserveAspectFit
        sourceSize.width: raiz.tam * 2
        sourceSize.height: raiz.tam * 2
    }
}
