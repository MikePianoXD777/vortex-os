// Cuadrícula de tarjetas que se reacomoda al ancho: de 1 a 4 columnas según cabe.
// Con `cargando` muestra esqueletos en lugar de las apps.
import QtQuick

Flow {
    id: raiz
    property var apps: []
    property bool cargando: false
    property int huecos: 6
    property int minAncho: 270
    spacing: 16

    readonly property int columnas: Math.max(1, Math.floor((width + spacing) / (minAncho + spacing)))
    readonly property real anchoTarjeta: Math.floor((width - spacing * (columnas - 1)) / columnas)

    Repeater {
        model: raiz.cargando ? raiz.huecos : 0
        Esqueleto { width: raiz.anchoTarjeta }
    }
    Repeater {
        model: raiz.cargando ? [] : raiz.apps
        Tarjeta {
            required property var modelData
            app: modelData
            width: raiz.anchoTarjeta
        }
    }
}
