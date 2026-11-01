// 2.4: la letra de lo que suena, como en Spotify: del color de la portada, la línea que suena en blanco,
// las que pasaron tenues y las que vienen oscuras. Se recorre sola al ritmo de la canción; clic en una
// línea salta ahí. Sin tiempos (la de YouTube Music) se muestra completa para leerla.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: raiz
    signal cerrar()
    property string colorPortada: ""
    readonly property var letra: vp.letra
    readonly property int actual: letra.sincronizada ? vp.lineaActual(rep.posicion) : -1
    color: colorPortada || Tema.realceFuerte
    Behavior on color { ColorAnimation { duration: 600 } }

    function refrescarColor() { colorPortada = vp.colorDe(rep.cancion.ruta || "") }
    Component.onCompleted: refrescarColor()
    Connections {
        target: rep
        function onCambioCancion() { raiz.refrescarColor() }
    }
    Connections {
        target: vp
        function onPortadaLista(r) { if (r === rep.cancion.ruta) raiz.refrescarColor() }
    }

    Rectangle { anchors.fill: parent; color: "black"; opacity: 0.12 }   // un poco más oscuro, para el texto blanco

    BotonIcono {
        anchors.top: parent.top
        anchors.right: parent.right
        anchors.margins: 18
        z: 2
        icono: "cerrar"
        ayuda: "Cerrar la letra"
        onClicked: raiz.cerrar()
    }

    ListView {
        id: lista
        anchors.fill: parent
        anchors.leftMargin: Math.max(40, (parent.width - 900) / 2)
        anchors.rightMargin: anchors.leftMargin
        topMargin: 60
        bottomMargin: parent.height * 0.5
        clip: true
        spacing: 14
        model: raiz.letra.lineas
        currentIndex: raiz.actual
        // La línea que suena se queda a un tercio de la altura; el recorrido va animado
        highlightRangeMode: raiz.letra.sincronizada && raiz.actual >= 0 ? ListView.ApplyRange : ListView.NoHighlightRange
        preferredHighlightBegin: height * 0.28
        preferredHighlightEnd: height * 0.28 + 60
        highlightMoveDuration: 550
        highlightMoveVelocity: -1
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        delegate: Text {
            id: linea
            required property var modelData
            required property int index
            width: ListView.view.width
            text: modelData.texto || (raiz.letra.sincronizada ? "♪" : "")
            wrapMode: Text.WordWrap
            font.pixelSize: 30
            font.weight: Font.Bold
            color: !raiz.letra.sincronizada || index === raiz.actual ? "white"
                : index < raiz.actual ? Qt.rgba(1, 1, 1, 0.55) : Qt.rgba(0, 0, 0, 0.55)
            Behavior on color { ColorAnimation { duration: 350 } }
            scale: index === raiz.actual ? 1.03 : 1
            transformOrigin: Item.Left
            Behavior on scale { NumberAnimation { duration: 300; easing.type: Easing.OutBack } }
            MouseArea {
                anchors.fill: parent
                enabled: raiz.letra.sincronizada && linea.modelData.ms >= 0
                cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
                hoverEnabled: true
                onClicked: rep.buscar(linea.modelData.ms / 1000)
            }
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        visible: !raiz.letra.lineas || raiz.letra.lineas.length === 0
        spacing: 10
        Icono { Layout.alignment: Qt.AlignHCenter; Layout.preferredWidth: 46; Layout.preferredHeight: 46; nombre: "letra"; color: "white"; trazo: 1.5 }
        Text {
            Layout.alignment: Qt.AlignHCenter
            text: !rep.cancion.ruta ? "Toca una canción para ver su letra"
                : raiz.letra.buscando ? "Buscando la letra…"
                : raiz.letra.error ? raiz.letra.error
                : "No encontré la letra de esta canción"
            color: "white"
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }
    }
}
