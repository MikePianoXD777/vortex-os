// Botón redondo con una flecha (carruseles y filas que se desplazan).
import QtQuick

Rectangle {
    id: raiz
    property string sentido: "derecha"     // «izquierda» o «derecha»
    property bool activo: true
    property string etiqueta: ""
    signal clicked()
    width: 36
    height: 36
    radius: 18
    opacity: activo ? 1 : 0.4
    color: zona.containsMouse && activo ? Tema.realceFuerte : Tema.realce
    border.width: 1
    border.color: Tema.bordeCampo
    Icono {
        anchors.centerIn: parent
        width: 16
        height: 16
        nombre: raiz.sentido
        color: Tema.texto
    }
    MouseArea {
        id: zona
        anchors.fill: parent
        hoverEnabled: true
        enabled: raiz.activo
        cursorShape: Qt.PointingHandCursor
        onClicked: raiz.clicked()
    }
    Accessible.role: Accessible.Button
    Accessible.name: etiqueta
}
