// Modelos: el catálogo de Vortexine con «Te cabe / Va justo / No te cabe» según tu gráfica, para bajar
// (con barra de progreso y Cancelar), y abajo los que ya tienes, con su tamaño y Borrar.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Flickable {
    id: raiz
    property bool activa: false
    contentHeight: columna.implicitHeight + 56
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    onActivaChanged: if (activa) modelos.recargar()

    Dialogo { id: dialogo }

    readonly property int columnas: Math.max(1, Math.floor((width - 56 + 16) / 316))
    readonly property real anchoTarjeta: (width - 56 - (columnas - 1) * 16) / columnas

    ColumnLayout {
        id: columna
        x: 28
        y: 28
        width: raiz.width - 56
        spacing: 18

        // encabezado
        RowLayout {
            Layout.fillWidth: true
            spacing: 14
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                Text { text: "Modelos"; color: Tema.texto; font.pixelSize: 28; font.weight: Font.Bold }
                Text {
                    Layout.fillWidth: true
                    text: "Los modelos de IA corren en tu PC. Te digo cuáles le caben a tu tarjeta de video."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
            }
            Rectangle {
                visible: modelos.gpu !== ""
                implicitWidth: filaGpu.implicitWidth + 28
                implicitHeight: 40
                radius: 20
                color: Tema.tarjeta
                border.width: 1
                border.color: Tema.borde
                RowLayout {
                    id: filaGpu
                    anchors.centerIn: parent
                    spacing: 8
                    Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "gpu"; color: Tema.azul }
                    Text { text: modelos.gpu; color: Tema.texto; font.pixelSize: 13; font.weight: Font.DemiBold }
                }
            }
        }

        // filtro por uso
        Flow {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: modelos.tipos
                Chip {
                    required property var modelData
                    texto: modelData.nombre
                    elegido: modelos.filtro === modelData.id
                    onPulsado: modelos.filtro = modelData.id
                }
            }
        }

        // catálogo
        Flow {
            id: tarjetas
            Layout.fillWidth: true
            spacing: 16
            Repeater {
                model: modelos.lista
                delegate: Rectangle {
                    id: tarjeta
                    required property string ident
                    required property string nombre
                    required property string descripcion
                    required property string tipo
                    required property string tamano
                    required property real vram_gb
                    required property string cabe
                    required property bool instalado
                    required property bool bajando
                    required property real progreso
                    required property string estado
                    required property bool recomendado
                    required property string contexto
                    required property bool piensa
                    width: raiz.anchoTarjeta
                    height: Math.max(212, contenido.implicitHeight + 36)
                    radius: Tema.radioTarjeta
                    color: zonaTarjeta.hovered ? Tema.tarjetaHover : Tema.tarjeta
                    border.width: recomendado ? 2 : 1
                    border.color: recomendado ? Tema.bordeHover : Tema.borde
                    Behavior on color { ColorAnimation { duration: 120 } }
                    HoverHandler { id: zonaTarjeta }

                    ColumnLayout {
                        id: contenido
                        anchors.fill: parent
                        anchors.margins: 18
                        spacing: 10
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 8
                            Text {
                                Layout.fillWidth: true
                                text: tarjeta.nombre
                                color: Tema.texto
                                font.pixelSize: 17
                                font.weight: Font.Bold
                                elide: Text.ElideRight
                            }
                            // Te cabe ✓ / Va justo / No te cabe
                            Rectangle {
                                implicitWidth: textoCabe.implicitWidth + 18
                                implicitHeight: 24
                                radius: 12
                                color: tarjeta.cabe === "si" ? Qt.alpha(Tema.exito, 0.16)
                                    : tarjeta.cabe === "justo" ? Qt.alpha(Tema.violeta, 0.16) : Tema.peligroFondo
                                Text {
                                    id: textoCabe
                                    anchors.centerIn: parent
                                    text: tarjeta.cabe === "si" ? "Te cabe ✓" : tarjeta.cabe === "justo" ? "Va justo" : "No te cabe"
                                    color: tarjeta.cabe === "si" ? Tema.exito : tarjeta.cabe === "justo" ? Tema.violeta : Tema.peligro
                                    font.pixelSize: 11
                                    font.weight: Font.Bold
                                }
                            }
                        }
                        Text {
                            Layout.fillWidth: true
                            text: tarjeta.ident
                            color: Tema.tenue
                            font.pixelSize: 11
                            font.family: "monospace"
                        }
                        Text {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            text: tarjeta.descripcion
                            color: Tema.suave
                            font.pixelSize: 13
                            wrapMode: Text.Wrap
                            lineHeight: 1.15
                            maximumLineCount: 4
                            elide: Text.ElideRight
                        }
                        Flow {
                            Layout.fillWidth: true
                            spacing: 6
                            Chip { visible: tarjeta.recomendado; texto: "Recomendado para ti"; icono: "chispa"; tocable: false; tinta: Tema.violeta }
                            Chip { texto: tarjeta.tipo; tocable: false }
                            Chip { visible: tarjeta.piensa; texto: "Piensa antes"; icono: "pensar"; tocable: false }
                        }
                        Text {
                            Layout.fillWidth: true
                            text: "Descarga " + tarjeta.tamano + " · usa " + tarjeta.vram_gb.toFixed(1) + " GB de video"
                                  + (tarjeta.contexto !== "" ? " · memoria " + tarjeta.contexto : "")
                            color: Tema.tenue
                            font.pixelSize: 11
                            wrapMode: Text.Wrap
                        }
                        // Bajar / progreso / instalado
                        RowLayout {
                            Layout.fillWidth: true
                            spacing: 10
                            visible: !tarjeta.bajando
                            Boton {
                                visible: !tarjeta.instalado
                                tipo: tarjeta.cabe === "no" ? "secundario" : "primario"
                                icono: "bajar"
                                text: "Bajar"
                                onClicked: {
                                    if (tarjeta.cabe === "no")
                                        dialogo.confirmar("¿Bajar " + tarjeta.nombre + " de todos modos?",
                                            "No le cabe a tu tarjeta de video: va a correr en parte con el procesador y la RAM, y será lento.",
                                            "Bajar igual", false, function () { modelos.bajar(tarjeta.ident) })
                                    else
                                        modelos.bajar(tarjeta.ident)
                                }
                            }
                            RowLayout {
                                visible: tarjeta.instalado
                                spacing: 6
                                Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; nombre: "visto"; color: Tema.exito }
                                Text { text: "Instalado"; color: Tema.exito; font.pixelSize: 13; font.weight: Font.DemiBold }
                            }
                            Item { Layout.fillWidth: true }
                        }
                        ColumnLayout {
                            Layout.fillWidth: true
                            visible: tarjeta.bajando
                            spacing: 6
                            RowLayout {
                                Layout.fillWidth: true
                                Text {
                                    Layout.fillWidth: true
                                    text: tarjeta.estado
                                    color: Tema.suave
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                }
                                Text {
                                    visible: tarjeta.progreso >= 0
                                    text: Math.round(tarjeta.progreso * 100) + " %"
                                    color: Tema.texto
                                    font.pixelSize: 12
                                    font.weight: Font.DemiBold
                                    font.features: { "tnum": 1 }
                                }
                            }
                            RowLayout {
                                Layout.fillWidth: true
                                spacing: 10
                                Barrita { Layout.fillWidth: true; valor: tarjeta.progreso }
                                Boton { tipo: "fantasma"; text: "Cancelar"; onClicked: modelos.cancelar(tarjeta.ident) }
                            }
                        }
                    }
                }
            }
        }

        // instalados
        Text {
            Layout.topMargin: 12
            text: "Tus modelos"
            color: Tema.texto
            font.pixelSize: 20
            font.weight: Font.Bold
        }
        Rectangle {
            Layout.fillWidth: true
            implicitHeight: listaInstalados.implicitHeight + 12
            radius: Tema.radioTarjeta
            color: Tema.tarjeta
            border.width: 1
            border.color: Tema.borde
            ColumnLayout {
                id: listaInstalados
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 6
                spacing: 0
                Text {
                    visible: modelos.listaInstalados.count === 0
                    Layout.fillWidth: true
                    Layout.margins: 16
                    text: ia.estadoOllama === "corriendo" ? "Todavía no tienes modelos. Baja uno de arriba: el recomendado es un buen comienzo."
                        : "Cuando Ollama esté corriendo, aquí salen tus modelos."
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.Wrap
                }
                Repeater {
                    model: modelos.listaInstalados
                    delegate: Rectangle {
                        id: inst
                        required property int index
                        required property string nombre
                        required property string bonito
                        required property string tamano
                        required property string detalles
                        required property bool embeddings
                        Layout.fillWidth: true
                        implicitHeight: 58
                        radius: 16
                        color: zonaInst.hovered ? Tema.realce : "transparent"
                        HoverHandler { id: zonaInst }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 14
                            anchors.rightMargin: 10
                            spacing: 14
                            Icono {
                                Layout.preferredWidth: 20; Layout.preferredHeight: 20
                                nombre: inst.embeddings ? "memoria" : "modelos"
                                color: Tema.azul
                            }
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 2
                                Text {
                                    Layout.fillWidth: true
                                    text: inst.bonito !== "" ? inst.bonito + "  ·  " + inst.nombre : inst.nombre
                                    color: Tema.texto
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                    elide: Text.ElideRight
                                }
                                Text {
                                    Layout.fillWidth: true
                                    text: (inst.embeddings ? "Para la memoria del vault · " : "") + inst.detalles
                                    color: Tema.tenue
                                    font.pixelSize: 12
                                    elide: Text.ElideRight
                                }
                            }
                            Text { text: inst.tamano; color: Tema.suave; font.pixelSize: 13; font.features: { "tnum": 1 } }
                            Boton {
                                tipo: "peligro"
                                icono: "basura"
                                text: "Borrar"
                                onClicked: dialogo.confirmar("¿Borrar " + inst.nombre + "?",
                                    "Se libera " + inst.tamano + " de tu disco. Lo puedes volver a bajar cuando quieras.",
                                    "Borrar", true, function () { modelos.borrar(inst.nombre) })
                            }
                        }
                    }
                }
            }
        }
    }
}
