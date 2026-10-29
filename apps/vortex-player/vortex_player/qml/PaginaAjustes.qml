// Ajustes: las carpetas de música, YouTube Music Premium y el audio (LDAC a 990 kbps, frecuencia nativa).
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import QtQuick.Dialogs

Flickable {
    id: raiz
    contentHeight: columna.implicitHeight + 56
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar {}

    component Tarjeta: Rectangle {
        default property alias contenido: interior.data
        property string titulo: ""
        Layout.fillWidth: true
        implicitHeight: interior.implicitHeight + 40
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        ColumnLayout {
            id: interior
            anchors.fill: parent
            anchors.margins: 20
            spacing: 12
            Text { text: parent.parent.titulo; color: Tema.texto; font.pixelSize: 17; font.weight: Font.DemiBold }
        }
    }

    component Explicacion: Text {
        Layout.fillWidth: true
        color: Tema.suave
        font.pixelSize: 13
        wrapMode: Text.WordWrap
    }

    ColumnLayout {
        id: columna
        x: 28
        y: 28
        width: Math.min(raiz.width - 56, 860)
        spacing: 18

        Text { text: "Ajustes"; color: Tema.texto; font.pixelSize: 32; font.weight: Font.Bold }

        Tarjeta {
            titulo: "Audio"
            RowLayout {
                Layout.fillWidth: true
                spacing: 12
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4
                    Text { text: "LDAC a 990 kbps"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.Medium }
                    Explicacion { text: "Con audífonos LDAC (como los SOUNDPEATS Air6 HS) siempre en la calidad más alta, en vez de «automático», que baja hasta 330 kbps cuando hay poca señal. Si se corta seguido, apágalo." }
                }
                Switch {
                    id: ldac
                    checked: !!vp.ajustes.ldac_hq
                    onToggled: vp.ponerLdacHq(checked)
                }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Tema.borde; opacity: 0.6 }
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 4
                RowLayout {
                    spacing: 8
                    Text { text: "Frecuencia nativa"; color: Tema.texto; font.pixelSize: 14; font.weight: Font.Medium }
                    Etiqueta { texto: "Siempre activa"; nivel: "cd" }
                }
                Explicacion { text: "Cada canción suena a su frecuencia original (44,1, 88,2, 96, 176,4 o 192 kHz) sin pasar todo a 48 kHz, y sin tocar el volumen ni normalizarlo." }
            }
            RowLayout {
                visible: !!vp.ajustes.audioPendiente
                Layout.fillWidth: true
                spacing: 12
                Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "info"; color: Tema.azul }
                Explicacion { text: "Esto se nota al volver a entrar a tu sesión, o ahora mismo (el sonido se corta un segundo)." }
                Boton { text: "Aplicar ahora"; icono: "girar"; onClicked: vp.aplicarAudio() }
            }
            Rectangle { Layout.fillWidth: true; height: 1; color: Tema.borde; opacity: 0.6 }
            RowLayout {
                spacing: 8
                Icono { Layout.preferredWidth: 17; Layout.preferredHeight: 17; nombre: vp.salida.bluetooth ? "audifonos" : "bocina"; color: Tema.suave }
                Text {
                    Layout.fillWidth: true
                    text: !vp.salida.nombre ? "No encuentro la salida de audio"
                        : "Ahora suena por " + vp.salida.nombre
                          + (vp.salida.bluetooth ? " con " + vp.salida.codecTexto : "")
                          + (vp.salida.hz ? " a " + (vp.salida.hz / 1000).toLocaleString(Qt.locale(), "f", vp.salida.hz % 1000 ? 1 : 0) + " kHz" : "")
                    color: Tema.suave
                    font.pixelSize: 13
                    wrapMode: Text.WordWrap
                }
                Boton {
                    visible: !!vp.salida.ldac && vp.salida.codec !== "ldac"
                    text: "Usar LDAC"
                    tipo: "secundario"
                    onClicked: vp.forzarLdac()
                }
            }
        }

        Tarjeta {
            titulo: "YouTube Music Premium"
            Explicacion { text: "Sin Premium se baja en Opus a ~160 kbps. Con Premium (también el familiar), AAC a 256 kbps: entra a music.youtube.com con tu cuenta en tu navegador y elígelo aquí. Vortex Player usa esa sesión solo para bajar." }
            Selector {
                id: navegador
                Layout.preferredWidth: 260
                model: vp.ajustes.navegadores || []
                textoDe: (n) => n === "" ? "No tengo Premium" : "Mi sesión de " + n.charAt(0).toUpperCase() + n.slice(1)
                displayText: textoDe(currentText)
                currentIndex: Math.max(0, (vp.ajustes.navegadores || []).indexOf(vp.ajustes.navegador_premium || ""))
                onActivated: (i) => vp.ponerNavegador(model[i])
            }
        }

        Tarjeta {
            titulo: "Carpetas de música"
            Explicacion { text: "Lo que pongas en estas carpetas aparece solo en tu biblioteca. Lo que bajas queda en " + vp.ajustes.descargas + "." }
            RowLayout {
                spacing: 10
                Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "carpeta"; color: Tema.azul }
                Text { Layout.fillWidth: true; text: vp.ajustes.musica; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideMiddle }
                Text { text: "Música"; color: Tema.tenue; font.pixelSize: 12 }
            }
            Repeater {
                model: vp.ajustes.carpetas || []
                delegate: RowLayout {
                    required property string modelData
                    spacing: 10
                    Icono { Layout.preferredWidth: 18; Layout.preferredHeight: 18; nombre: "carpeta"; color: Tema.azul }
                    Text { Layout.fillWidth: true; text: modelData; color: Tema.texto; font.pixelSize: 13; elide: Text.ElideMiddle }
                    Boton { text: "Quitar"; tipo: "peligro"; onClicked: vp.quitarCarpeta(modelData) }
                }
            }
            RowLayout {
                spacing: 10
                Boton { text: "Agregar una carpeta"; icono: "carpeta"; tipo: "secundario"; onClicked: dialogoCarpeta.open() }
                Boton { text: vp.escaneando ? "Buscando…" : "Buscar música nueva"; icono: "girar"; tipo: "fantasma"; enabled: !vp.escaneando; onClicked: vp.escanear() }
            }
        }

        Text {
            Layout.fillWidth: true
            text: "Vortex Player 1.0 · para uso personal. Lo que bajas de YouTube es para escucharlo tú."
            color: Tema.tenue
            font.pixelSize: 12
        }
    }

    FolderDialog {
        id: dialogoCarpeta
        title: "Agregar una carpeta de música"
        onAccepted: vp.agregarCarpeta(selectedFolder)
    }
}
