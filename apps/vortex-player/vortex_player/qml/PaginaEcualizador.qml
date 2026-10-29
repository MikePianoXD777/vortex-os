// El ecualizador: es el mismo del panel de música del escritorio (vp.eq lo lee del mismo archivo y lo
// cambia con el mismo script). Mueves una banda aquí y se mueve allá, y al revés.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts

Item {
    id: raiz

    function frecuencia(f) {
        return f >= 1000 ? (f / 1000).toLocaleString(Qt.locale(), "f", f % 1000 ? 1 : 0) + "k" : Math.round(f) + ""
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 28
        spacing: 16

        Text { text: "Ecualizador"; color: Tema.texto; font.pixelSize: 32; font.weight: Font.Bold }
        RowLayout {
            spacing: 8
            Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; nombre: "info"; color: Tema.azul }
            Text {
                Layout.fillWidth: true
                text: "Es el mismo del panel de música del escritorio y suena en todo el sistema: lo que muevas aquí se mueve allá."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
        }

        Flow {
            Layout.fillWidth: true
            spacing: 8
            Repeater {
                model: vp.presets
                delegate: Boton {
                    required property string modelData
                    text: ({ "Flat": "Plano", "Bass": "Graves", "Treble": "Agudos", "Vocal": "Voces", "Pop": "Pop",
                             "Rock": "Rock", "Jazz": "Jazz", "Classic": "Clásica" })[modelData] || modelData
                    tipo: vp.eq.preset === modelData ? "primario" : "secundario"
                    onClicked: vp.ponerPreset(modelData)
                }
            }
            Boton { visible: vp.eq.preset === "Custom"; text: "Personalizado"; tipo: "primario"; enabled: false }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.maximumHeight: 420
            radius: Tema.radioTarjeta
            color: Tema.tarjeta

            RowLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 4
                // La escala en dB
                ColumnLayout {
                    Layout.fillHeight: true
                    Layout.topMargin: 22
                    Layout.bottomMargin: 30
                    Text { text: "+12"; color: Tema.tenue; font.pixelSize: 11 }
                    Item { Layout.fillHeight: true }
                    Text { text: "0"; color: Tema.tenue; font.pixelSize: 11 }
                    Item { Layout.fillHeight: true }
                    Text { text: "-12"; color: Tema.tenue; font.pixelSize: 11 }
                }
                // 2.1: el modelo es el número de bandas, no la lista. Con la lista, cada movimiento (que cambia
                // vp.eq) volvía a crear todas las barras y la que arrastrabas se cambiaba bajo el mouse: daba
                // brincos y se soltaba. Así solo se rehacen si cambia cuántas bandas hay.
                Repeater {
                    model: vp.eq.bandas.length
                    delegate: ColumnLayout {
                        id: banda
                        required property int index
                        readonly property var modelData: vp.eq.bandas[index] || { "f": 0, "g": 0 }
                        Layout.fillWidth: true
                        Layout.fillHeight: true
                        spacing: 6
                        Text {
                            Layout.alignment: Qt.AlignHCenter
                            text: (barra.value > 0 ? "+" : "") + barra.value.toLocaleString(Qt.locale(), "f", 1)
                            color: barra.value === 0 ? Tema.tenue : Tema.texto
                            font.pixelSize: 11
                            font.features: { "tnum": 1 }
                        }
                        Slider {
                            id: barra
                            Layout.alignment: Qt.AlignHCenter
                            Layout.fillHeight: true
                            orientation: Qt.Vertical
                            from: -12; to: 12; stepSize: 0.5
                            hoverEnabled: true
                            // Sigue al estado (el panel del escritorio, un preset) solo mientras no la arrastras
                            Binding on value { value: banda.modelData.g; when: !barra.pressed }
                            onMoved: vp.ponerBanda(banda.index, value)
                            background: Rectangle {
                                x: barra.leftPadding + barra.availableWidth / 2 - width / 2
                                y: barra.topPadding
                                width: 6
                                height: barra.availableHeight
                                radius: 3
                                color: Tema.borde
                                Rectangle {   // la línea del 0
                                    width: 14; height: 2; radius: 1
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    y: parent.height / 2 - 1
                                    color: Tema.tenue
                                    opacity: 0.6
                                }
                                Rectangle {   // del 0 hasta el valor
                                    width: parent.width
                                    radius: 3
                                    color: barra.value >= 0 ? Tema.azul : Tema.violeta
                                    y: barra.value >= 0 ? barra.visualPosition * parent.height : parent.height / 2
                                    height: Math.abs(barra.visualPosition - 0.5) * parent.height
                                }
                            }
                            handle: Rectangle {
                                x: barra.leftPadding + barra.availableWidth / 2 - width / 2
                                y: barra.topPadding + barra.visualPosition * (barra.availableHeight - height)
                                width: 18; height: 18; radius: 9
                                color: barra.pressed ? Tema.azulPresion : Tema.texto
                                border.width: 3
                                border.color: Tema.tarjeta
                            }
                            WheelHandler { onWheel: (e) => vp.ponerBanda(banda.index, Math.max(-12, Math.min(12, barra.value + (e.angleDelta.y > 0 ? 0.5 : -0.5)))) }
                        }
                        Text {
                            Layout.alignment: Qt.AlignHCenter
                            text: raiz.frecuencia(banda.modelData.f)
                            color: Tema.suave
                            font.pixelSize: 12
                            font.weight: Font.DemiBold
                        }
                    }
                }
            }
        }
        Text {
            Layout.fillWidth: true
            text: "Para cambiar las frecuencias o agregar bandas (de 2 a 16), usa el panel de música del escritorio: aquí aparecen solas."
            color: Tema.tenue
            font.pixelSize: 12
            wrapMode: Text.WordWrap
        }
        Item { Layout.fillHeight: true }
    }
}
