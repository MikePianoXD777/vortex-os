// La calidad real de la canción: «Hi-Res 24/96» (dorada), «CD 16/44,1» (azul) o «Opus 160 kbps» (gris).
import QtQuick

Rectangle {
    id: raiz
    property string texto: ""
    property string nivel: "comprimida"
    readonly property color tinta: nivel === "hires" ? "#f2c86b" : nivel === "cd" ? Tema.azul : Tema.tenue
    visible: texto !== ""
    implicitWidth: t.implicitWidth + 14
    implicitHeight: 20
    radius: 6
    color: Qt.rgba(tinta.r, tinta.g, tinta.b, nivel === "comprimida" ? 0.10 : 0.16)
    border.width: 1
    border.color: Qt.rgba(tinta.r, tinta.g, tinta.b, 0.45)
    Text {
        id: t
        anchors.centerIn: parent
        text: raiz.texto
        color: raiz.tinta
        font.pixelSize: 11
        font.weight: Font.DemiBold
        font.letterSpacing: 0.3
    }
}
