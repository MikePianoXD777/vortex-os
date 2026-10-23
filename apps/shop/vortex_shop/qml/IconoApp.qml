// Ícono de una app, cargado de su URL. Mientras carga (o si falla) se ve la inicial sobre un color.
import QtQuick

Item {
    id: raiz
    property string url: ""
    property string nombre: ""
    property int tam: 56
    width: tam
    height: tam

    readonly property var paleta: [["#25306e", "#9ec5ff"], ["#352570", "#c6a8ff"], ["#1c4558", "#8fdcf0"],
                                   ["#44255a", "#f0a8e6"], ["#244a3f", "#8fe3b4"], ["#58402a", "#f5c88f"]]
    readonly property int tono: {
        var h = 0
        for (var i = 0; i < nombre.length; i++)
            h = (h * 31 + nombre.charCodeAt(i)) % 9973
        return h % paleta.length
    }

    Rectangle {
        anchors.fill: parent
        radius: raiz.tam * 0.24
        color: raiz.paleta[raiz.tono][0]
        visible: imagen.status !== Image.Ready
        Text {
            anchors.centerIn: parent
            text: raiz.nombre.length > 0 ? raiz.nombre.charAt(0).toUpperCase() : "?"
            color: raiz.paleta[raiz.tono][1]
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
