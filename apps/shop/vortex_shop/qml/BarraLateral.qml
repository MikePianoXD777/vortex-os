// Barra lateral: logo y nombre, Inicio, Categorías (las que dé el catálogo) y, abajo,
// Instaladas y Actualizaciones (con su contador).
import QtQuick
import QtQuick.Controls.Basic

Rectangle {
    id: raiz
    property string seccion: "inicio"      // «inicio», «categoria», «instaladas», «actualizaciones»…
    property string categoriaId: ""
    color: Tema.barra
    implicitWidth: Tema.anchoLateral

    Rectangle {   // borde derecho
        anchors.right: parent.right
        width: 1
        height: parent.height
        color: Tema.borde
    }

    Row {
        id: marca
        x: 20
        y: 22
        spacing: 12
        Image {
            width: 38
            height: 38
            source: "vortex-os.svg"
            sourceSize.width: 76
            sourceSize.height: 76
            smooth: true
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Vortexine's\nShop"
            color: Tema.texto
            font.pixelSize: 16
            font.weight: Font.DemiBold
            lineHeight: 0.95
        }
    }

    ItemLateral {
        id: inicio
        anchors.top: marca.bottom
        anchors.topMargin: 22
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 13
        icono: "inicio"
        texto: "Inicio"
        seleccionado: raiz.seccion === "inicio"
        onClicked: Nav.seccion("inicio", "")
    }

    Text {
        id: etiquetaCategorias
        anchors.top: inicio.bottom
        anchors.topMargin: 18
        x: 26
        text: "Categorías"
        color: Tema.tenue
        font.pixelSize: 12
        font.weight: Font.DemiBold
        font.letterSpacing: 1.4
        font.capitalization: Font.AllUppercase
    }

    Flickable {
        id: desplazable
        anchors.top: etiquetaCategorias.bottom
        anchors.topMargin: 8
        anchors.bottom: abajo.top
        anchors.bottomMargin: 8
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 13
        contentHeight: listaCategorias.implicitHeight
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: listaCategorias
            width: parent.width
            spacing: 2
            Repeater {
                model: tienda.categorias
                ItemLateral {
                    required property var modelData
                    width: listaCategorias.width
                    icono: Iconos.paraCategoria(modelData.icono + " " + modelData.nombre)
                    texto: modelData.nombre
                    seleccionado: raiz.seccion === "categoria" && raiz.categoriaId === modelData.id
                    onClicked: Nav.seccion("categoria", modelData.id)
                }
            }
        }
    }

    Column {
        id: abajo
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 16
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: 12
        anchors.rightMargin: 13
        spacing: 2
        Rectangle {
            width: parent.width
            height: 1
            color: Tema.borde
        }
        Item { width: 1; height: 8 }
        ItemLateral {
            width: parent.width
            icono: "instaladas"
            texto: "Instaladas"
            seleccionado: raiz.seccion === "instaladas"
            onClicked: Nav.seccion("instaladas", "")
        }
        ItemLateral {
            width: parent.width
            icono: "actualizaciones"
            texto: "Actualizaciones"
            contador: tienda.numActualizaciones
            seleccionado: raiz.seccion === "actualizaciones"
            onClicked: Nav.seccion("actualizaciones", "")
        }
    }
}
