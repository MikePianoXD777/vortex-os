// El cuadro con el ícono de un tipo de archivo: degradado azul → violeta de Vortexine's y el trazo encima.
// Si hay `imagen` (el ícono real de una app instalada), se muestra esa en su lugar.
import QtQuick

Rectangle {
    id: raiz
    property string tipo: ""
    property string imagen: ""
    property int lado: 76
    implicitWidth: lado
    implicitHeight: lado
    width: lado
    height: lado
    radius: Math.round(lado * 0.29)
    gradient: Gradient {
        orientation: Gradient.Vertical
        GradientStop { position: 0; color: imagenReal.status === Image.Ready ? Tema.realce : Tema.azul }
        GradientStop { position: 1; color: imagenReal.status === Image.Ready ? Tema.realce : Tema.violeta }
    }
    Icono {
        anchors.centerIn: parent
        visible: imagenReal.status !== Image.Ready
        width: Math.round(raiz.lado * 0.5)
        height: width
        nombre: Iconos.paraTipo(raiz.tipo)
        color: Tema.sobreAcento
        trazo: 2.2
    }
    Image {
        id: imagenReal
        anchors.centerIn: parent
        width: Math.round(raiz.lado * 0.72)
        height: width
        sourceSize.width: width * 2
        sourceSize.height: height * 2
        fillMode: Image.PreserveAspectFit
        smooth: true
        asynchronous: true
        source: raiz.imagen === "" ? "" : (raiz.imagen.indexOf("://") > 0 ? raiz.imagen : "file://" + raiz.imagen)
    }
}
