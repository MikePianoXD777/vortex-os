// Detalle de una app: ícono grande, acciones, capturas, descripción y datos.
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property var d: tienda.detalle
    readonly property string estado: tienda.detalleEstado
    readonly property bool listo: estado === "listo"

    Boton {
        text: "Atrás"
        icono: "atras"
        tipo: "fantasma"
        onClicked: Nav.atras()
    }

    // Cabecera
    RowLayout {
        Layout.fillWidth: true
        spacing: 24
        IconoApp {
            Layout.alignment: Qt.AlignTop
            tam: raiz.width < 700 ? 88 : 120
            url: raiz.d.icono ? raiz.d.icono : ""
            nombre: raiz.d.nombre ? raiz.d.nombre : ""
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignTop
            spacing: 6
            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Text {
                    id: titulo
                    Layout.fillWidth: true
                    text: raiz.d.nombre ? raiz.d.nombre : ""
                    color: Tema.texto
                    font.pixelSize: 32
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }
            }
            RowLayout {
                spacing: 8
                Sello { visible: raiz.d.verificado === true; tam: 18 }
                Text {
                    text: (raiz.d.desarrollador ? raiz.d.desarrollador : "")
                    color: Tema.suave
                    font.pixelSize: 15
                }
                Text {
                    visible: raiz.d.verificado === true
                    text: "· Verificado"
                    color: Tema.azul
                    font.pixelSize: 14
                }
            }
            Text {
                Layout.fillWidth: true
                visible: text !== ""
                text: raiz.d.resumen ? raiz.d.resumen : ""
                color: Tema.suave
                font.pixelSize: 15
                wrapMode: Text.WordWrap
            }
            Text {
                visible: text !== ""
                text: raiz.d.descargasTexto ? raiz.d.descargasTexto : ""
                color: Tema.tenue
                font.pixelSize: 13
            }
            AccionApp {
                Layout.topMargin: 12
                Layout.preferredWidth: enCurso ? Math.min(400, raiz.width) : implicitWidth
                appId: raiz.d.id ? raiz.d.id : ""
                nombre: raiz.d.nombre ? raiz.d.nombre : ""
                modo: "detalle"
            }
        }
    }

    // Cargando el resto
    Rectangle {
        Layout.fillWidth: true
        visible: raiz.estado === "cargando"
        implicitHeight: 200
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde
        SequentialAnimation on opacity {
            running: raiz.estado === "cargando"
            loops: Animation.Infinite
            NumberAnimation { from: 1; to: 0.5; duration: 800 }
            NumberAnimation { from: 0.5; to: 1; duration: 800 }
        }
        Text {
            anchors.centerIn: parent
            text: "Cargando información…"
            color: Tema.tenue
            font.pixelSize: 14
        }
    }
    EstadoPantalla {
        Layout.fillWidth: true
        visible: raiz.estado === "sinred" || raiz.estado === "error"
        implicitHeight: 280
        icono: raiz.estado === "sinred" ? "sinred" : "alerta"
        titulo: raiz.estado === "sinred" ? "Sin conexión a internet" : "No pudimos cargar esta app"
        texto: raiz.estado === "sinred" ? "Revisa tu red y vuelve a intentarlo."
                                        : "Algo salió mal. Vuelve a intentarlo en un momento."
        boton: "Reintentar"
        onAccion: tienda.reintentarDetalle()
    }

    // Capturas
    Seccion {
        Layout.fillWidth: true
        visible: raiz.listo && raiz.d.capturas && raiz.d.capturas.length > 0
        titulo: "Capturas"
        Carrusel {
            Layout.fillWidth: true
            Layout.preferredHeight: altoCaptura
            imagenes: raiz.d.capturas ? raiz.d.capturas : []
        }
    }

    // Descripción
    Seccion {
        Layout.fillWidth: true
        visible: raiz.listo && raiz.d.descripcion !== ""
        titulo: "Acerca de esta app"
        Text {
            Layout.fillWidth: true
            Layout.maximumWidth: 780
            text: raiz.d.descripcion ? raiz.d.descripcion : ""
            textFormat: Text.PlainText
            color: Tema.suave
            font.pixelSize: 15
            lineHeight: 1.35
            wrapMode: Text.WordWrap
        }
    }

    // Datos
    Seccion {
        Layout.fillWidth: true
        visible: raiz.listo
        titulo: "Información"
        Flow {
            id: datos
            Layout.fillWidth: true
            spacing: 12
            readonly property int columnas: Math.max(1, Math.min(4, Math.floor((width + spacing) / (200 + spacing))))
            readonly property real ancho: Math.floor((width - spacing * (columnas - 1)) / columnas)

            Repeater {
                model: [
                    { "etiqueta": "Versión", "valor": raiz.d.version ? raiz.d.version : "", "enlace": false },
                    { "etiqueta": "Tamaño de la descarga", "valor": raiz.d.tamanoTexto ? raiz.d.tamanoTexto : "", "enlace": false },
                    { "etiqueta": "Licencia", "valor": raiz.d.licencia ? raiz.d.licencia : "", "enlace": false },
                    { "etiqueta": "Sitio web", "valor": raiz.d.web ? raiz.d.web : "", "enlace": true }
                ]
                Rectangle {
                    required property var modelData
                    visible: modelData.valor !== ""
                    width: datos.ancho
                    height: 76
                    radius: 16
                    color: Tema.tarjeta
                    border.width: 1
                    border.color: Tema.borde
                    Column {
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.margins: 16
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        Text {
                            width: parent.width
                            text: modelData.etiqueta
                            color: Tema.tenue
                            font.pixelSize: 12
                            elide: Text.ElideRight
                        }
                        Text {
                            id: valor
                            width: parent.width
                            text: modelData.enlace ? modelData.valor.replace(/^https?:\/\//, "").replace(/\/$/, "") : modelData.valor
                            color: modelData.enlace ? Tema.azul : Tema.texto
                            font.pixelSize: 15
                            font.weight: Font.DemiBold
                            font.underline: modelData.enlace && enlace.containsMouse
                            elide: Text.ElideRight
                            MouseArea {
                                id: enlace
                                anchors.fill: parent
                                enabled: modelData.enlace
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Qt.openUrlExternally(modelData.valor)
                            }
                        }
                    }
                }
            }
        }
    }
}
