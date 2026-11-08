// Ajustes: Ollama (el motor que corre los modelos), la carpeta del vault y el modelo que usa la memoria.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import QtQuick.Layouts

Flickable {
    id: raiz
    property bool activa: false
    signal irA(string pagina)
    contentHeight: columna.implicitHeight + 56
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    onActivaChanged: if (activa) ia.revisarOllama(false)

    component Tarjeta: Rectangle {
        id: tarjeta
        property string titulo: ""
        property string icono: ""
        default property alias contenido: interior.data
        Layout.fillWidth: true
        implicitHeight: interior.implicitHeight + 72
        radius: Tema.radioTarjeta
        color: Tema.tarjeta
        border.width: 1
        border.color: Tema.borde
        RowLayout {
            x: 22; y: 20
            spacing: 10
            Icono { Layout.preferredWidth: 19; Layout.preferredHeight: 19; nombre: tarjeta.icono; color: Tema.azul }
            Text { text: tarjeta.titulo; color: Tema.texto; font.pixelSize: 16; font.weight: Font.Bold }
        }
        ColumnLayout {
            id: interior
            x: 22; y: 54
            width: parent.width - 44
            spacing: 12
        }
    }

    FolderDialog {
        id: dialogoCarpeta
        title: "Abrir otra carpeta como vault"
        onAccepted: ia.abrirCarpetaVault(selectedFolder)
    }

    ColumnLayout {
        id: columna
        x: 28
        y: 28
        width: Math.min(raiz.width - 56, 760)
        spacing: 16

        Text { text: "Ajustes"; color: Tema.texto; font.pixelSize: 28; font.weight: Font.Bold }

        Tarjeta {
            titulo: "Ollama"
            icono: "rayo"
            RowLayout {
                Layout.fillWidth: true
                spacing: 10
                Punto { estado: ia.estadoOllama }
                Text {
                    Layout.fillWidth: true
                    text: ia.estadoOllama === "corriendo" ? "Corriendo"
                        : ia.estadoOllama === "arrancando" ? "Arrancando…"
                        : ia.estadoOllama === "falta" ? "No está instalado" : "Detenido"
                    color: Tema.texto
                    font.pixelSize: 14
                    font.weight: Font.DemiBold
                }
                Boton {
                    visible: ia.estadoOllama === "detenido"
                    icono: "play"
                    text: "Arrancar"
                    onClicked: ia.arrancarOllama()
                }
            }
            Text {
                Layout.fillWidth: true
                text: ia.estadoOllama === "falta"
                    ? "Ollama es el motor que corre los modelos en tu PC. Se instala con «vortex actualizar»."
                    : "Ollama es el motor que corre los modelos en tu PC. Lo arranco solo cuando abres la app."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
            Text {
                Layout.fillWidth: true
                visible: ia.mensajeOllama !== ""
                text: ia.mensajeOllama
                color: Tema.peligro
                font.pixelSize: 12
                wrapMode: Text.Wrap
            }
            Text {
                Layout.fillWidth: true
                text: ia.urlOllama + (ia.gpu !== "" ? "  ·  " + ia.gpu : "")
                color: Tema.tenue
                font.pixelSize: 12
                elide: Text.ElideRight
            }
        }

        Tarjeta {
            titulo: "Carpeta del vault"
            icono: "vault"
            Text {
                Layout.fillWidth: true
                text: ia.carpetaVault
                color: Tema.texto
                font.pixelSize: 13
                font.family: "monospace"
                wrapMode: Text.WrapAnywhere
            }
            Text {
                Layout.fillWidth: true
                text: "Puedes usar un vault que ya tengas (por ejemplo el de Obsidian): no se rompe nada, son los mismos archivos."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
            RowLayout {
                spacing: 10
                Boton { icono: "carpeta"; text: "Abrir otra carpeta"; onClicked: dialogoCarpeta.open() }
                Boton { tipo: "fantasma"; text: "Volver al de siempre"; onClicked: ia.vaultPorDefecto() }
            }
        }

        Tarjeta {
            titulo: "Modelo para el vault"
            icono: "memoria"
            Text {
                Layout.fillWidth: true
                text: "Es un modelo chiquito que convierte tus notas en «memoria» para que la IA encuentre lo que importa (embeddings)."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
            Selector {
                id: selectorMemoria
                visible: modelos.nombresEmbeddings.length > 0
                Layout.preferredWidth: 320
                readonly property var opciones: ["Automático"].concat(modelos.nombresEmbeddings)
                model: opciones
                currentIndex: Math.max(0, opciones.indexOf(ia.modeloMemoria))
                onActivated: (i) => ia.modeloMemoria = i === 0 ? "" : opciones[i]
            }
            ColumnLayout {
                visible: modelos.nombresEmbeddings.length === 0
                Layout.fillWidth: true
                spacing: 10
                Text {
                    Layout.fillWidth: true
                    text: "No tienes ninguno: mientras, la IA busca en tus notas por palabras. Para que entienda mejor, baja «"
                          + ia.embeddingsRecomendado + "» (es pequeño)."
                    color: Tema.tenue
                    font.pixelSize: 12
                    wrapMode: Text.Wrap
                }
                Boton {
                    tipo: "secundario"
                    icono: "bajar"
                    text: "Bajarlo en Modelos"
                    onClicked: { modelos.filtro = "vault"; raiz.irA("modelos") }
                }
            }
        }

        Tarjeta {
            titulo: "Acerca de"
            icono: "info"
            Text {
                Layout.fillWidth: true
                text: "Vortexine's Local AI " + ia.version + " · todo corre en tu PC: tus chats y tus notas no salen a internet."
                color: Tema.suave
                font.pixelSize: 13
                wrapMode: Text.Wrap
            }
        }
    }
}
