pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "foto": "M4 8h3l2-3h6l2 3h3a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1zM12 10a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z",
        "video": "M3 7a1 1 0 0 1 1-1h11a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1zM16 10.5l5-3v9l-5-3z",
        "cambiar": "M4 12a8 8 0 0 1 13.7-5.6L20 9M20 4v5h-5M20 12a8 8 0 0 1-13.7 5.6L4 15M4 20v-5h5",
        "ajustes": "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1",
        "reloj": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 7.5V12l3 2",
        "pausa": "M8 5v14M16 5v14",
        "play": "M7 5l12 7-12 7z",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "carpeta": "M3 7a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v9a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z",
        "derecha": "M9 6l6 6-6 6",
        "sinCamara": "M3 3l18 18M9 5h6l2 3h3a1 1 0 0 1 1 1v8M18 20H4a1 1 0 0 1-1-1V9a1 1 0 0 1 1-1h3",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .8-1 1.5M12 17h.01"
    })
    function _hex(c) { return "" + c }
    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var relleno = nombre === "play" ? _hex(color) : "none"
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + relleno + '" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
