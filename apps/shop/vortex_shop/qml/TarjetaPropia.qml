// Tarjeta de una app de Vortexine's que no viene de fábrica (Vortexine's Camera…): ícono, nombre con el
// sello de Vortexine's, de qué se trata y sus botones. No tiene página de detalle: todo está aquí.
import QtQuick
import QtQuick.Layouts

Rectangle {
    id: raiz
    property var app: ({})
    readonly property string fase: tienda.propiasEstados[app.id] || ""
    readonly property bool ocupada: fase === "instalando" || fase === "quitando"

    implicitHeight: Math.max(132, fila.implicitHeight + 36)
    radius: Tema.radioTarjeta
    border.width: 1
    border.color: Tema.bordeHover
    gradient: Gradient {
        orientation: Gradient.Horizontal
        GradientStop { position: 0; color: Tema.realce }
        GradientStop { position: 1; color: Tema.tarjeta }
    }

    RowLayout {
        id: fila
        anchors.fill: parent
        anchors.margins: 18
        spacing: 18
        IconoApp {
            Layout.alignment: Qt.AlignTop
            tam: 72
            url: raiz.app.icono || ""
            nombre: raiz.app.nombre || ""
        }
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 4
            RowLayout {
                spacing: 8
                Text {
                    text: raiz.app.nombre || ""
                    color: Tema.texto
                    font.pixelSize: 18
                    font.weight: Font.DemiBold
                }
                Rectangle {
                    implicitWidth: sello.implicitWidth + 16
                    implicitHeight: 20
                    radius: 10
                    color: Tema.realceFuerte
                    Text { id: sello; anchors.centerIn: parent; text: "De Vortexine's"; color: Tema.violeta; font.pixelSize: 11; font.weight: Font.DemiBold }
                }
            }
            Text {
                Layout.fillWidth: true
                text: raiz.app.resumen || ""
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.WordWrap
                maximumLineCount: 3
                elide: Text.ElideRight
            }
            Text {
                visible: raiz.ocupada
                text: raiz.fase === "instalando" ? "Instalando…" : "Quitando…"
                color: Tema.azul
                font.pixelSize: 12
                font.weight: Font.DemiBold
            }
        }
        Column {
            Layout.alignment: Qt.AlignVCenter
            spacing: 8
            Boton {
                visible: !raiz.app.instalada
                enabled: !raiz.ocupada
                text: raiz.fase === "instalando" ? "Instalando…" : "Instalar"
                icono: "descargar"
                onClicked: tienda.instalarPropia(raiz.app.id)
            }
            Boton {
                visible: raiz.app.instalada === true
                enabled: !raiz.ocupada && raiz.fase !== "abriendo"
                text: raiz.fase === "abriendo" ? "Abriendo…" : "Abrir"
                icono: "abrir"
                onClicked: tienda.abrirPropia(raiz.app.id)
            }
            Boton {
                visible: raiz.app.instalada === true
                enabled: !raiz.ocupada
                tipo: "peligro"
                text: raiz.fase === "quitando" ? "Quitando…" : "Quitar"
                icono: "papelera"
                onClicked: tienda.quitarPropia(raiz.app.id)
            }
        }
    }
}
