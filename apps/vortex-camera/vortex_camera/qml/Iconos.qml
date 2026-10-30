pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo. Los de Vortexine's
// (los mismos de Vortex Player y Task Manager) más los de las cámaras.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "camara": "M3 7.5A1.5 1.5 0 0 1 4.5 6h9A1.5 1.5 0 0 1 15 7.5v9a1.5 1.5 0 0 1-1.5 1.5h-9A1.5 1.5 0 0 1 3 16.5zM15 10.5l6-3.5v10l-6-3.5",
        "vivo": "M12 12h.01M8.5 15.5a5 5 0 0 1 0-7M15.5 8.5a5 5 0 0 1 0 7M5.5 18.5a9.5 9.5 0 0 1 0-13M18.5 5.5a9.5 9.5 0 0 1 0 13",
        "historial": "M4 12a8 8 0 1 0 2.3-5.7M3.5 3.5v4h4M12 8v4.5l3 2",
        "mosaico": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "mas": "M12 5v14M5 12h14",
        "foto": "M4 8h3l2-3h6l2 3h3v11H4zM12 10.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7z",
        "grabar": "M12 6a6 6 0 1 0 0 12 6 6 0 0 0 0-12z",
        "detener": "M6.5 6.5h11v11h-11z",
        "pantalla": "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5",
        "salirPantalla": "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5",
        "volumen": "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11",
        "silencio": "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM16 9.5l5 5M21 9.5l-5 5",
        "editar": "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
        "basura": "M4 7h16M9 7V4.5h6V7M6.5 7l1 13h9l1-13M10 11v5M14 11v5",
        "arriba": "M6 15l6-6 6 6",
        "abajo": "M6 9l6 6 6-6",
        "izquierda": "M15 6l-6 6 6 6",
        "derecha": "M9 6l6 6-6 6",
        "retroceder": "M11 6l-6 6 6 6M19 6l-6 6 6 6",
        "adelantar": "M13 6l6 6-6 6M5 6l6 6-6 6",
        "calendario": "M4 6h16v14H4zM4 10h16M8 3.5v4M16 3.5v4",
        "casa": "M4 11l8-7 8 7v9H4zM10 20v-5h4v5",
        "ojo": "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
        "ojoNo": "M3 3l18 18M10.6 5.1A10.6 10.6 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3 3.9M6.6 6.6C3.7 8.4 2 12 2 12s3.5 7 10 7c1.9 0 3.5-.6 4.9-1.4M9.9 9.9a3 3 0 0 0 4.2 4.2",
        "red": "M12 20h.01M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 14 0M1.5 9.5a15 15 0 0 1 21 0",
        "buscar": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "mas_puntos": "M5 12h.01M12 12h.01M19 12h.01",
        "play": "M7 4.5v15l12.5-7.5z",
        "pausa": "M8 5v14M16 5v14",
        "girar": "M20 12a8 8 0 1 1-2.3-5.7M20.5 3.5v4h-4",
        "alerta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16h.01",
        "visto": "M5 12.5l4.5 4.5L19 7.5",
        "carpeta": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
        "info": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01"
    })

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var relleno = (nombre === "play" || nombre === "grabar" || nombre === "detener") ? _hex(color) : "none"
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + relleno + '" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
