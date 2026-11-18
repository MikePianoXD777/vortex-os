// Pestaña «Instalado»: todo lo que instaló o descomprimió la app (AppImages, programas de ~/.local/opt,
// Flatpaks, mods y carpetas), con su botón para abrirlo y otro para deshacerlo (a la papelera).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: raiz
    readonly property var t: instalador.t
    readonly property int margen: width < 640 ? 20 : 32

    // Sin nada instalado
    Column {
        visible: instalador.instalados.length === 0
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 420)
        spacing: 14
        Mosaico {
            anchors.horizontalCenter: parent.horizontalCenter
            tipo: "flatpak"
            lado: 84
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: raiz.t.instalado_vacio_titulo
            color: Tema.texto
            font.pixelSize: 21
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: raiz.t.instalado_vacio_texto
            color: Tema.suave
            font.pixelSize: 15
            lineHeight: 1.25
            wrapMode: Text.WordWrap
        }
    }

    ListView {
        id: lista
        visible: instalador.instalados.length > 0
        anchors.fill: parent
        anchors.leftMargin: raiz.margen
        anchors.rightMargin: raiz.margen
        spacing: 10
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: instalador.instalados
        ScrollBar.vertical: Barra {}
        header: Text {
            width: lista.width
            text: raiz.t.instalado_ayuda
            color: Tema.tenue
            font.pixelSize: 13
            topPadding: 8
            bottomPadding: 14
            leftPadding: 4
            wrapMode: Text.WordWrap
        }
        footer: Item { width: 1; height: 24 }
        delegate: Rectangle {
            required property var modelData
            width: lista.width
            height: fila.implicitHeight + 28
            radius: Tema.radioTarjeta
            color: zona.containsMouse ? Tema.tarjetaHover : Tema.tarjeta
            border.width: 1
            border.color: zona.containsMouse ? Tema.bordeHover : Tema.borde
            Behavior on color { ColorAnimation { duration: 120 } }
            MouseArea { id: zona; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
            RowLayout {
                id: fila
                x: 16
                y: 14
                width: parent.width - 32
                spacing: 14
                Mosaico {
                    lado: 52
                    tipo: modelData.tipo === "appimage" || modelData.tipo === "programa" ? "programa"
                        : modelData.tipo === "flatpak" ? "flatpak" : modelData.tipo === "mod" ? "mod" : "descomprimido"
                    imagen: modelData.icono
                }
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 3
                    Text {
                        Layout.fillWidth: true
                        text: modelData.nombre
                        color: Tema.texto
                        font.pixelSize: 15
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }
                    Text {
                        Layout.fillWidth: true
                        text: [modelData.tipoTexto, modelData.fecha, modelData.detalle]
                              .filter(function (x) { return x }).join("  ·  ")
                        color: Tema.tenue
                        font.pixelSize: 12
                        elide: Text.ElideRight
                    }
                }
                Boton {
                    visible: raiz.width > 520
                    text: raiz.t.boton_abrir_instalado
                    icono: modelData.tipo === "appimage" || modelData.tipo === "programa" || modelData.tipo === "flatpak"
                           ? "abrir" : "carpeta"
                    tipo: "secundario"
                    onClicked: instalador.abrirInstalado(modelData.id)
                }
                Boton {
                    text: modelData.boton
                    icono: "papelera"
                    tipo: "peligro"
                    onClicked: ventana.pedirQuitar(modelData)
                }
            }
        }
    }
}
