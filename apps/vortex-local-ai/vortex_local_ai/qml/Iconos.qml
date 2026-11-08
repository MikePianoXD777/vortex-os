pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo. Los de Vortexine's
// (los mismos de Vortex Player y del Task Manager) más los de Local AI. Todos dibujados para este proyecto.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "chat": "M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v9a1.5 1.5 0 0 1-1.5 1.5H10l-4.5 4v-4h0A1.5 1.5 0 0 1 4 14.5zM8 9h8M8 12h5",
        "modelos": "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
        "vault": "M5 4.5A1.5 1.5 0 0 1 6.5 3H19v16H6.5A1.5 1.5 0 0 0 5 20.5zM5 20.5A1.5 1.5 0 0 0 6.5 22H19v-3M9 7h6M9 10.5h4",
        "ajustes": "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1",
        "chispa": "M12 3c.9 5 3.3 7.4 8.3 8.3-5 .9-7.4 3.3-8.3 8.3-.9-5-3.3-7.4-8.3-8.3C8.7 10.4 11.1 8 12 3z",
        "enviar": "M4 12l16-8-6 16-2.5-6.5zM11.5 13.5L20 4",
        "detener": "M6.5 6.5h11v11h-11z",
        "girar": "M20 12a8 8 0 1 1-2.3-5.7M20.5 3.5v4h-4",
        "copiar": "M9 9h11v11H9zM5 15H4V4h11v1",
        "basura": "M4 7h16M9 7V4.5h6V7M6.5 7l1 13h9l1-13M10 11v5M14 11v5",
        "bajar": "M12 4v11M7 10.5l5 5 5-5M5 20h14",
        "carpeta": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
        "carpetaNueva": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5zM12 10v6M9 13h6",
        "nota": "M6 3h8.5L19 7.5V21H6zM14 3v5h5M9 12h6M9 15.5h6",
        "notaNueva": "M6 3h8.5L19 7.5V21H6zM14 3v5h5M12.5 11v7M9 14.5h7",
        "grafo": "M6 6a2 2 0 1 0 0 .01M18 7a2 2 0 1 0 0 .01M12 18a2 2 0 1 0 0 .01M7.6 7.2l3.2 9M16.6 8.6l-3.6 7.8M8 6.2l8 .6",
        "lapiz": "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4",
        "ojo": "M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12zM12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6z",
        "columnas": "M4 5h16v14H4zM12 5v14",
        "visto": "M5 12.5l4.5 4.5L19 7.5",
        "alerta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16h.01",
        "info": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01",
        "pensar": "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.5 10.9c.6.5 1 1.2 1 2V16h5v-.1c0-.8.4-1.5 1-2A6 6 0 0 0 12 3z",
        "enlace": "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
        "etiqueta": "M3.5 12.5V4.5A1 1 0 0 1 4.5 3.5h8l8 8-9 9zM8 8h.01",
        "gpu": "M2 7h18v10H2zM20 10h2M20 14h2M5 17v3M8.5 17v3M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM16.5 10.5v3M5.5 10.5v3",
        "guardar": "M5 4h11l3 3v13H5zM8 4v5h7V4M8 20v-6h8v6",
        "mas": "M12 5v14M5 12h14",
        "buscar": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "derecha": "M9 5l7 7-7 7",
        "mas_puntos": "M5 12h.01M12 12h.01M19 12h.01",
        "play": "M7 4.5v15l12.5-7.5z",
        "rayo": "M13 2L4.5 13.5H12L11 22l8.5-11.5H12z",
        "memoria": "M12 4a8 8 0 1 0 8 8M12 8a4 4 0 1 0 4 4M12 12h.01M15 4.5l1 3 3 1",
        "codigo": "M8.5 7L3.5 12l5 5M15.5 7l5 5-5 5M13.5 4.5l-3 15",
        "foto": "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M15.5 8.5h.01",
        "usuario": "M12 4a4 4 0 1 0 0 8 4 4 0 0 0 0-8zM4.5 20.5a7.5 7.5 0 0 1 15 0"
    })

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var relleno = (nombre === "play" || nombre === "rayo" || nombre === "chispa") ? _hex(color) : "none"
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + relleno + '" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
