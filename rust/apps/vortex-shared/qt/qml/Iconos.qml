pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "archivo": "M6 3h8l4 4v14H6zM14 3v4h4",
        "carpeta": "M3 7a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "copiar": "M9 9h11v11H9zM5 15H4V4h11v1",
        "compu": "M3 5h18v11H3zM8 20h8M12 16v4",
        "enviar": "M21 3L10 14M21 3l-7 18-4-7-7-4z",
        "subir": "M12 16V4M7 9l5-5 5 5M4 20h16",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 17h.01"
    })
    function _hex(c) { return "" + c }
    function uri(nombre, color, trazo) {
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="'
                + (trazos[nombre] || trazos["pregunta"]) + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
