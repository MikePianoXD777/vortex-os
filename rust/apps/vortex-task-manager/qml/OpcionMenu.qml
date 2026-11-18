// Una opción de MenuVortex: ícono, texto, atajo y, si es un submenú, la flechita.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

MenuItem {
    id: raiz
    property string icono: ""
    property string atajo: ""
    property bool peligro: false
    implicitHeight: visible ? 36 : 0
    hoverEnabled: true
    readonly property color tinta: !enabled ? Tema.tenue : peligro ? Tema.peligro : checked ? Tema.azul : Tema.texto
    arrow: Icono {
        x: raiz.width - width - 10
        y: (raiz.height - height) / 2
        visible: raiz.subMenu !== null
        width: 14; height: 14
        nombre: "derecha"
        color: Tema.tenue
    }
    indicator: Item {}
    contentItem: RowLayout {
        spacing: 10
        Icono {
            Layout.preferredWidth: 17; Layout.preferredHeight: 17
            Layout.leftMargin: 4
            nombre: raiz.checked ? "visto" : raiz.icono
            color: raiz.tinta
            opacity: nombre === "" ? 0 : 1
        }
        Text {
            Layout.fillWidth: true
            text: raiz.text
            color: raiz.tinta
            font.pixelSize: 13
            font.weight: raiz.checked ? Font.DemiBold : Font.Normal
            elide: Text.ElideRight
        }
        Text {
            visible: raiz.atajo !== ""
            text: raiz.atajo
            color: Tema.tenue
            font.pixelSize: 11
            Layout.rightMargin: 6
        }
    }
    background: Rectangle {
        radius: 10
        color: raiz.highlighted && raiz.enabled ? (raiz.peligro ? Tema.peligroFondo : Tema.realce) : "transparent"
    }
}
