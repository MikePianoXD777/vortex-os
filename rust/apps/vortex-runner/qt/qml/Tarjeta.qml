// Un programa de la biblioteca: ícono grande, nombre, cuándo lo abriste y el botón Jugar (o Detener si
// está abierto). Clic derecho o «…»: configurar, lanzador, carpetas, Wine y quitar.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Rectangle {
    id: raiz
    required property var programa
    signal configurar()
    signal pedirQuitar()

    radius: Tema.radioTarjeta
    color: area.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
    border.width: 1
    border.color: programa.abierto ? Tema.azul : area.containsMouse ? Tema.bordeHover : Tema.borde
    Behavior on color { ColorAnimation { duration: 120 } }
    Behavior on border.color { ColorAnimation { duration: 120 } }

    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: (ev) => { if (ev.button === Qt.RightButton) menu.popup() }
        onDoubleClicked: if (!raiz.programa.abierto) runner.jugar(raiz.programa.id)
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 16
        spacing: 10

        // El ícono del .exe (o su inicial, si no trae)
        Rectangle {
            Layout.alignment: Qt.AlignHCenter
            Layout.topMargin: 6
            width: 84; height: 84
            radius: 22
            color: Tema.realce
            Image {
                id: icono
                anchors.centerIn: parent
                width: 64; height: 64
                sourceSize.width: 128
                sourceSize.height: 128
                fillMode: Image.PreserveAspectFit
                smooth: true
                mipmap: true
                asynchronous: true
                source: raiz.programa.icono
                visible: status === Image.Ready
            }
            Text {
                anchors.centerIn: parent
                visible: !icono.visible
                text: raiz.programa.nombre.charAt(0).toUpperCase()
                color: Tema.azul
                font.pixelSize: 36
                font.weight: Font.Black
            }
            Rectangle {   // puntito verde: está abierto
                visible: raiz.programa.abierto
                width: 16; height: 16; radius: 8
                color: Tema.exito
                border.width: 3
                border.color: Tema.tarjeta
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                anchors.margins: -3
            }
        }

        Text {
            Layout.fillWidth: true
            text: raiz.programa.nombre
            color: Tema.texto
            font.pixelSize: 15
            font.weight: Font.Bold
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
        }
        Text {
            Layout.fillWidth: true
            Layout.topMargin: -6
            text: raiz.programa.existe ? raiz.programa.subtitulo : "No encuentro su .exe"
            color: raiz.programa.existe ? (raiz.programa.abierto ? Tema.exito : Tema.tenue) : Tema.peligro
            font.pixelSize: 12
            horizontalAlignment: Text.AlignHCenter
            elide: Text.ElideRight
        }

        Item { Layout.fillHeight: true }

        RowLayout {
            Layout.fillWidth: true
            spacing: 6
            Boton {
                Layout.fillWidth: true
                tipo: raiz.programa.abierto ? "secundario" : "primario"
                icono: raiz.programa.abierto ? "detener" : "play"
                text: raiz.programa.abierto ? "Detener" : "Jugar"
                enabled: raiz.programa.existe || raiz.programa.abierto
                onClicked: raiz.programa.abierto ? runner.detener(raiz.programa.id) : runner.jugar(raiz.programa.id)
            }
            BotonIcono {
                icono: "mas_puntos"
                ayuda: "Más opciones"
                onClicked: menu.popup()
            }
        }
    }

    MenuVortex {
        id: menu
        OpcionMenu { text: "Configurar"; icono: "ajustes"; onTriggered: raiz.configurar() }
        OpcionMenu {
            text: raiz.programa.lanzador ? "Quitar del lanzador de apps" : "Agregar al lanzador de apps"
            icono: "lanzador"
            onTriggered: runner.lanzador(raiz.programa.id, !raiz.programa.lanzador)
        }
        OpcionMenu { text: "Abrir su carpeta"; icono: "carpeta"; onTriggered: runner.abrirCarpeta(raiz.programa.id) }
        OpcionMenu { text: "Abrir su disco C:"; icono: "disco"; onTriggered: runner.abrirDiscoC(raiz.programa.id) }
        OpcionMenu { text: "Configuración de Wine"; icono: "herramienta"; onTriggered: runner.configuracionWine(raiz.programa.id) }
        MenuSeparator {
            contentItem: Rectangle { implicitHeight: 1; color: Tema.borde }
        }
        OpcionMenu { text: "Quitar de la biblioteca"; icono: "basura"; peligro: true; onTriggered: raiz.pedirQuitar() }
    }
}
