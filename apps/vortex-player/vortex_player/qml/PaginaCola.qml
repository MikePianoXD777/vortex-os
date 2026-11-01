// 2.4: la cola, como «A continuación» de Spotify: lo que suena y lo que viene. Clic en una salta ahí y la X
// la quita de la cola. Se llena con «Tocar después» y «Agregar a la cola» (clic derecho en una canción).
// 1.4: al final, las de la radio sin fin (parecidas a lo que escuchabas, suenan sin bajarlas).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: raiz
    signal cerrar()
    color: Tema.fondo
    readonly property var siguientes: rep.cola.slice(rep.indice + 1)

    function tiempo(s) {
        s = Math.max(0, Math.floor(s || 0))
        var m = Math.floor(s / 60), r = s % 60
        return m + ":" + (r < 10 ? "0" : "") + r
    }

    component Fila: Rectangle {
        id: fila
        property var cancion: ({})
        property int numero: 0
        property bool actual: false
        signal tocar()
        signal quitar()
        Layout.fillWidth: true
        implicitHeight: 58
        radius: 8
        color: zona.containsMouse ? Tema.tarjetaHover : "transparent"
        Behavior on color { ColorAnimation { duration: 120 } }
        MouseArea { id: zona; anchors.fill: parent; hoverEnabled: true; cursorShape: fila.actual ? Qt.ArrowCursor : Qt.PointingHandCursor; onClicked: fila.tocar() }
        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 14
            Text {
                Layout.preferredWidth: 26
                horizontalAlignment: Text.AlignRight
                text: fila.actual ? "♪" : fila.numero
                color: fila.actual ? Tema.azul : Tema.tenue
                font.pixelSize: 13
                font.features: { "tnum": 1 }
            }
            Portada { Layout.preferredWidth: 42; Layout.preferredHeight: 42; ruta: fila.cancion.ruta || "" }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Text { Layout.fillWidth: true; text: fila.cancion.titulo || ""; color: fila.actual ? Tema.azul : Tema.texto; font.pixelSize: 14; font.weight: Font.Medium; elide: Text.ElideRight }
                Text { Layout.fillWidth: true; text: fila.cancion.artista || "Artista desconocido"; color: Tema.tenue; font.pixelSize: 12; elide: Text.ElideRight }
            }
            Text { text: raiz.tiempo(fila.cancion.duracion); color: Tema.tenue; font.pixelSize: 13; font.features: { "tnum": 1 } }
            BotonIcono {
                visible: !fila.actual
                opacity: zona.containsMouse || hovered ? 1 : 0
                tam: 30; icono: "cerrar"; ayuda: "Quitar de la cola"
                onClicked: fila.quitar()
            }
        }
    }

    BotonIcono {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 18
        z: 2
        icono: "cerrar"
        ayuda: "Cerrar la cola"
        onClicked: raiz.cerrar()
    }

    Flickable {
        anchors.fill: parent
        contentHeight: columna.implicitHeight + 56
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {}

        ColumnLayout {
            id: columna
            x: 28
            y: 28
            width: Math.min(parent.width - 56, 980)
            spacing: 4

            Text { text: "A continuación"; color: Tema.texto; font.pixelSize: 32; font.weight: Font.Bold; Layout.bottomMargin: 14 }

            Text { visible: !!rep.cancion.ruta; text: "Sonando ahora"; color: Tema.texto; font.pixelSize: 16; font.weight: Font.DemiBold; Layout.bottomMargin: 4 }
            Fila { visible: !!rep.cancion.ruta; cancion: rep.cancion; actual: true }

            RowLayout {
                Layout.fillWidth: true
                Layout.topMargin: 24
                Layout.bottomMargin: 4
                visible: raiz.siguientes.length > 0
                Text { text: "Lo que sigue"; color: Tema.texto; font.pixelSize: 16; font.weight: Font.DemiBold }
                Text { Layout.fillWidth: true; text: rep.origen ? "· de " + rep.origen : ""; color: Tema.tenue; font.pixelSize: 13; elide: Text.ElideRight }
                Text { text: raiz.siguientes.length + (raiz.siguientes.length === 1 ? " canción" : " canciones"); color: Tema.tenue; font.pixelSize: 13 }
            }
            Repeater {
                model: raiz.siguientes.slice(0, 300)   // con miles, las primeras 300 bastan para ver qué viene
                ColumnLayout {
                    required property var modelData
                    required property int index
                    Layout.fillWidth: true
                    spacing: 4
                    RowLayout {   // 1.4: donde empieza la radio
                        visible: !!modelData.radio && (index === 0 || !raiz.siguientes[index - 1].radio)
                        Layout.fillWidth: true
                        Layout.topMargin: index === 0 ? 0 : 20
                        Layout.bottomMargin: 4
                        spacing: 8
                        Icono { Layout.preferredWidth: 16; Layout.preferredHeight: 16; nombre: "radio"; color: Tema.azul }
                        Text { text: "Radio"; color: Tema.texto; font.pixelSize: 16; font.weight: Font.DemiBold }
                        Text { Layout.fillWidth: true; text: "· parecidas a lo que escuchabas, suenan sin bajarlas"; color: Tema.tenue; font.pixelSize: 13; elide: Text.ElideRight }
                    }
                    Fila {
                        cancion: modelData
                        numero: index + 1
                        onTocar: rep.saltarA(rep.indice + 1 + index)
                        onQuitar: rep.quitarDeColaEn(rep.indice + 1 + index)
                    }
                }
            }

            ColumnLayout {
                visible: raiz.siguientes.length === 0
                Layout.alignment: Qt.AlignHCenter
                Layout.topMargin: 60
                spacing: 10
                Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 46; Layout.preferredHeight: 46; nombre: "cola"; color: Tema.tenue; trazo: 1.5 }
                Text { Layout.alignment: Qt.AlignHCenter; text: "No viene nada después"; color: Tema.texto; font.pixelSize: 18; font.weight: Font.DemiBold }
                Text {
                    Layout.alignment: Qt.AlignHCenter
                    text: vp.ajustes.radio === false ? "Clic derecho en una canción → «Tocar después» o «Agregar a la cola»."
                        : "Al acabarse, la radio sigue con canciones parecidas."
                    color: Tema.tenue
                    font.pixelSize: 13
                }
            }
        }
    }
}
