// Pantalla de estado centrada: sin conexión, sin resultados, lista vacía, error…
import QtQuick
import QtQuick.Layouts

Item {
    id: raiz
    property string icono: "info"
    property string titulo: ""
    property string texto: ""
    property string boton: ""
    property string boton2: ""
    signal accion()
    signal accion2()
    implicitHeight: 330

    Column {
        anchors.centerIn: parent
        width: Math.min(parent.width, 460)
        spacing: 14
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 76
            height: 76
            radius: 38
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            Icono {
                anchors.centerIn: parent
                width: 34
                height: 34
                nombre: raiz.icono
                color: Tema.azul
                trazo: 1.8
            }
        }
        Text {
            width: parent.width
            text: raiz.titulo
            color: Tema.texto
            font.pixelSize: 20
            font.weight: Font.DemiBold
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }
        Text {
            width: parent.width
            visible: raiz.texto !== ""
            text: raiz.texto
            color: Tema.suave
            font.pixelSize: 14
            lineHeight: 1.25
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 10
            topPadding: 6
            Boton {
                visible: raiz.boton !== ""
                text: raiz.boton
                grande: true
                onClicked: raiz.accion()
            }
            Boton {
                visible: raiz.boton2 !== ""
                text: raiz.boton2
                tipo: "secundario"
                grande: true
                onClicked: raiz.accion2()
            }
        }
    }
}
