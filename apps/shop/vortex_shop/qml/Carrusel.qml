// Carrusel de capturas de pantalla: se arrastra o se avanza con las flechas.
import QtQuick

Item {
    id: raiz
    property var imagenes: []
    property int altoCaptura: width < 700 ? 200 : 270
    readonly property real anchoCaptura: Math.round(altoCaptura * 16 / 9)
    implicitHeight: altoCaptura

    ListView {
        id: lista
        anchors.fill: parent
        orientation: ListView.Horizontal
        spacing: 14
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: raiz.imagenes
        delegate: Item {
            id: celda
            required property string modelData
            width: raiz.anchoCaptura
            height: raiz.altoCaptura
            Rectangle {
                anchors.fill: parent
                radius: 14
                color: Tema.tarjeta
                border.width: 1
                border.color: Tema.borde
            }
            Image {
                id: foto
                anchors.fill: parent
                anchors.margins: 1
                source: celda.modelData
                asynchronous: true
                fillMode: Image.PreserveAspectCrop
                sourceSize.width: raiz.anchoCaptura * 2
                sourceSize.height: raiz.altoCaptura * 2
            }
            Esquinas { anchors.fill: foto; radio: 13; relleno: Tema.fondo }
            Rectangle {   // marco fino por encima, para que el borde no lo tape la imagen
                anchors.fill: parent
                radius: 14
                color: "transparent"
                border.width: 1
                border.color: Tema.borde
            }
            Text {
                anchors.centerIn: parent
                visible: foto.status === Image.Loading
                text: "Cargando…"
                color: Tema.tenue
                font.pixelSize: 13
            }
        }
    }

    NumberAnimation { id: mover; target: lista; property: "contentX"; duration: 260; easing.type: Easing.InOutCubic }
    function avanzar(sentido) {
        mover.stop()
        mover.to = Math.max(0, Math.min(lista.contentWidth - lista.width, lista.contentX + sentido * (anchoCaptura + lista.spacing)))
        mover.start()
    }

    Repeater {
        model: [-1, 1]
        Rectangle {
            required property int modelData
            visible: modelData < 0 ? lista.contentX > 4 : lista.contentX < lista.contentWidth - lista.width - 4
            width: 40
            height: 40
            radius: 20
            y: (raiz.height - height) / 2
            x: modelData < 0 ? 10 : raiz.width - width - 10
            color: flecha.containsMouse ? Tema.realceFuerte : Tema.flotante
            border.width: 1
            border.color: Tema.bordeCampo
            Icono {
                anchors.centerIn: parent
                width: 18
                height: 18
                nombre: parent.modelData < 0 ? "izquierda" : "derecha"
                color: Tema.texto
            }
            MouseArea {
                id: flecha
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: raiz.avanzar(parent.modelData)
            }
        }
    }
}
