// Apps instaladas: lista con Abrir y Quitar.
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property int cuantas: tienda.instaladas.length

    Seccion {
        Layout.fillWidth: true
        etiqueta: "Tu equipo"
        titulo: "Instaladas"
        tamTitulo: 28
        subtitulo: raiz.cuantas > 0 ? raiz.cuantas + (raiz.cuantas === 1 ? " app instalada desde la tienda" : " apps instaladas desde la tienda") : ""

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 10
            Repeater {
                model: tienda.instaladas
                FilaApp {
                    required property var modelData
                    Layout.fillWidth: true
                    app: modelData
                    modo: "instalada"
                }
            }
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.cuantas === 0
            icono: "instaladas"
            titulo: "Todavía no instalaste ninguna app"
            texto: "Cuando instales algo desde la tienda, aparecerá aquí para que lo abras o lo quites."
            boton: "Explorar apps"
            onAccion: Nav.seccion("inicio", "")
        }
    }
}
