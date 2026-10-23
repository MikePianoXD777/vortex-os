// Actualizaciones pendientes, con «Actualizar todo».
import QtQuick
import QtQuick.Layouts

Pagina {
    id: raiz
    readonly property int cuantas: tienda.numActualizaciones
    // «Actualizar todo» se apaga cuando ya está todo en marcha o en cola.
    readonly property bool todoEnMarcha: {
        var lista = tienda.actualizables
        for (var i = 0; i < lista.length; i++)
            if (tienda.estados[lista[i].id] === undefined)
                return false
        return true
    }

    Seccion {
        Layout.fillWidth: true
        etiqueta: "Mantén tus apps al día"
        titulo: "Actualizaciones"
        tamTitulo: 28
        subtitulo: tienda.revisandoActualizaciones && raiz.cuantas === 0 ? "Buscando actualizaciones…"
                   : raiz.cuantas === 0 ? "Todo está al día"
                   : raiz.cuantas + (raiz.cuantas === 1 ? " app tiene una versión nueva" : " apps tienen una versión nueva")
        derecha: [
            Boton {
                text: "Revisar ahora"
                icono: "actualizaciones"
                tipo: "secundario"
                enabled: !tienda.revisandoActualizaciones
                onClicked: tienda.buscarActualizaciones()
            },
            Boton {
                visible: raiz.cuantas > 0
                text: "Actualizar todo"
                icono: "descargar"
                enabled: !raiz.todoEnMarcha
                onClicked: tienda.actualizarTodo()
            }
        ]

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 10
            Repeater {
                model: tienda.actualizables
                FilaApp {
                    required property var modelData
                    Layout.fillWidth: true
                    app: modelData
                    modo: "actualizacion"
                }
            }
        }
        EstadoPantalla {
            Layout.fillWidth: true
            visible: raiz.cuantas === 0 && !tienda.revisandoActualizaciones
            icono: "visto"
            titulo: "Todo está al día"
            texto: "No hay actualizaciones pendientes. Revisamos de vez en cuando por ti."
        }
    }
}
