// Aviso de error que aparece abajo y se cierra solo (o con la X).
import QtQuick

Rectangle {
    id: raiz
    property string texto: ""
    function mostrar(mensaje) {
        texto = mensaje
        abierto = true
        reloj.restart()
    }
    property bool abierto: false

    width: Math.min(parent.width - Tema.anchoLateral - 48, 520)
    height: fila.implicitHeight + 28
    x: Tema.anchoLateral + (parent.width - Tema.anchoLateral - width) / 2   // centrado sobre el contenido
    y: abierto ? parent.height - height - 24 : parent.height + 8
    radius: 18
    color: Tema.peligroFondo
    border.width: 1
    border.color: Tema.peligroBorde
    opacity: abierto ? 1 : 0
    visible: opacity > 0
    z: 90
    Behavior on y { NumberAnimation { duration: 200; easing.type: Easing.OutCubic } }
    Behavior on opacity { NumberAnimation { duration: 200 } }

    Timer { id: reloj; interval: 9000; onTriggered: raiz.abierto = false }

    Row {
        id: fila
        x: 16
        y: 14
        width: parent.width - 32
        spacing: 12
        Icono {
            anchors.verticalCenter: parent.verticalCenter
            width: 22
            height: 22
            nombre: "alerta"
            color: Tema.peligro
        }
        Text {
            width: parent.width - 22 - 34 - parent.spacing * 2
            anchors.verticalCenter: parent.verticalCenter
            text: raiz.texto
            color: Tema.texto
            font.pixelSize: 14
            wrapMode: Text.WordWrap
        }
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: 34
            height: 34
            radius: 17
            color: cerrar.containsMouse ? Tema.realce : "transparent"
            Icono {
                anchors.centerIn: parent
                width: 16
                height: 16
                nombre: "cerrar"
                color: Tema.suave
            }
            MouseArea {
                id: cerrar
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: raiz.abierto = false
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Cerrar aviso"
        }
    }
}
