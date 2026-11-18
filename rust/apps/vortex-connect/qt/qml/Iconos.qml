pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "escudo": "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6z",
        "escudoBien": "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6zM8.5 12l2.5 2.5 4.5-5",
        "escudoRoto": "M12 3l8 3v6c0 5-3.5 8-8 9-4.5-1-8-4-8-9V6zM12 8v5M12 16.5h.01",
        "wifi": "M3 9a14 14 0 0 1 18 0M6.5 12.5a9 9 0 0 1 11 0M10 16a4 4 0 0 1 4 0M12 19h.01",
        "sinWifi": "M3 3l18 18M8.5 16a5 5 0 0 1 7 0M5 12.5a10 10 0 0 1 4-2.2M19 12.5a10 10 0 0 0-5-2.4M2 8.8a15 15 0 0 1 4-2.4M22 8.8A15 15 0 0 0 10 5.3M12 19.5h.01",
        "recargar": "M20 11a8 8 0 0 0-14.5-3.5L3 10M3 4v6h6M4 13a8 8 0 0 0 14.5 3.5L21 14M21 20v-6h-6",
        "abrir": "M14 4h6v6M20 4l-9 9M18 14v6H4V6h6",
        "viaje": "M10 17l-1 4 3-2 3 2-1-4M3 11l18-7-7 18-3-8z",
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
