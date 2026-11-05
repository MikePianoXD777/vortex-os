// El panel de ajustes: sale de la izquierda, sobre la imagen.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: raiz
    property var camaras: []          // nombres de las cámaras
    property int camaraIndice: -1
    property var resoluciones: []     // «1920x1080»…
    property int resolucionIndice: -1
    property bool microfono: true
    property bool cuadricula: false
    property string modo: "foto"
    signal cerrar()
    signal elegirCamara(int indice)
    signal elegirResolucion(int indice)
    signal cambiarMicrofono(bool si)
    signal cambiarCuadricula(bool si)

    color: Tema.fondo
    MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons; onWheel: (w) => w.accepted = true }

    component Fila: RowLayout {
        property alias texto: etiqueta.text
        Layout.fillWidth: true
        spacing: 12
        Text { id: etiqueta; Layout.fillWidth: true; color: Tema.texto; font.pixelSize: 14; wrapMode: Text.WordWrap }
    }
    component Titulo: Text { color: Tema.tenue; font.pixelSize: 12; font.weight: Font.DemiBold; Layout.topMargin: 8 }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 10
        RowLayout {
            Layout.fillWidth: true
            Text { text: "Ajustes"; color: Tema.texto; font.pixelSize: 20; font.weight: Font.DemiBold; Layout.fillWidth: true }
            AbstractButton {
                implicitWidth: 34; implicitHeight: 34
                hoverEnabled: true
                Accessible.name: "Cerrar ajustes"
                onClicked: raiz.cerrar()
                background: Rectangle { radius: 17; color: parent.hovered ? Tema.realce : "transparent" }
                contentItem: Item { Icono { anchors.centerIn: parent; width: 18; height: 18; nombre: "cerrar"; color: Tema.suave } }
            }
        }
        Titulo { text: "Cámara" }
        Selector {
            Layout.fillWidth: true
            model: raiz.camaras
            currentIndex: raiz.camaraIndice
            enabled: raiz.camaras.length > 1
            onActivated: (i) => raiz.elegirCamara(i)
        }
        Titulo { text: "Calidad" }
        Selector {
            Layout.fillWidth: true
            model: raiz.resoluciones
            currentIndex: raiz.resolucionIndice
            enabled: raiz.resoluciones.length > 1
            textoDe: (r) => vv.etiquetaResolucion(r)
            displayText: currentIndex >= 0 && currentIndex < raiz.resoluciones.length ? vv.etiquetaResolucion(raiz.resoluciones[currentIndex]) : "—"
            onActivated: (i) => raiz.elegirResolucion(i)
        }
        Fila {
            Layout.topMargin: 8
            texto: "Grabar sonido en los videos"
            Interruptor { checked: raiz.microfono; onToggled: raiz.cambiarMicrofono(checked) }
        }
        Fila {
            texto: "Cuadrícula para encuadrar"
            Interruptor { checked: raiz.cuadricula; onToggled: raiz.cambiarCuadricula(checked) }
        }
        Item { Layout.fillHeight: true }
        AbstractButton {
            Layout.fillWidth: true
            implicitHeight: 42
            hoverEnabled: true
            onClicked: vv.abrirCarpeta(raiz.modo)
            background: Rectangle { radius: 21; color: parent.hovered ? Tema.realceFuerte : Tema.campo; border.width: 1; border.color: Tema.bordeCampo }
            contentItem: Row {
                spacing: 8
                leftPadding: 16
                Icono { anchors.verticalCenter: parent.verticalCenter; width: 18; height: 18; nombre: "carpeta"; color: Tema.suave }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: raiz.modo === "video" ? "Abrir la carpeta de videos" : "Abrir la carpeta de fotos"
                    color: Tema.texto
                    font.pixelSize: 14
                }
            }
        }
    }
}
