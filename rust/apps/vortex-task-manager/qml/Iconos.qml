pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo. Los de Vortexine's
// (los mismos de Vortex Player) más los del administrador de tareas.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "procesos": "M4 5h16M4 10h16M4 15h10M4 20h7M18 15v6M15 18h6",
        "rendimiento": "M3 12h4l2.5-6 4 12 2.5-6H21",
        "servicios": "M12 8.5a3.5 3.5 0 1 0 0 7 3.5 3.5 0 0 0 0-7zM12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1",
        "cpu": "M7 7h10v10H7zM10 10h4v4h-4zM9.5 3v4M14.5 3v4M9.5 17v4M14.5 17v4M3 9.5h4M3 14.5h4M17 9.5h4M17 14.5h4",
        "memoria": "M3 7h18v9H3zM3 16v3M21 16v3M7 10v3M11 10v3M15 10v3M7 19v-3M12 19v-3M17 19v-3",
        "disco": "M4 6.5C4 5 7.6 4 12 4s8 1 8 2.5v11c0 1.5-3.6 2.5-8 2.5s-8-1-8-2.5zM4 6.5C4 8 7.6 9 12 9s8-1 8-2.5M4 12c0 1.5 3.6 2.5 8 2.5s8-1 8-2.5",
        "red": "M12 20h.01M8.5 16.5a5 5 0 0 1 7 0M5 13a10 10 0 0 1 14 0M1.5 9.5a15 15 0 0 1 21 0",
        "ethernet": "M5 4h14v11H5zM9 15v5h6v-5M8.5 8v3M11.5 8v3M14.5 8v3",
        "gpu": "M2 7h18v10H2zM20 10h2M20 14h2M5 17v3M8.5 17v3M12 9.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5zM16.5 10.5v3M5.5 10.5v3",
        "app": "M4 5h16v14H4zM4 9h16M7 7h.01M9.5 7h.01",
        "fondo": "M12 3l8 4.5v9L12 21l-8-4.5v-9zM12 12l8-4.5M12 12v9M12 12L4 7.5",
        "escudo": "M12 3l7.5 3v5.5c0 4.5-3.2 8-7.5 9.5-4.3-1.5-7.5-5-7.5-9.5V6z",
        "buscar": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "derecha": "M9 5l7 7-7 7",
        "mas_puntos": "M5 12h.01M12 12h.01M19 12h.01",
        "detener": "M6.5 6.5h11v11h-11z",
        "play": "M7 4.5v15l12.5-7.5z",
        "pausa": "M8 5v14M16 5v14",
        "girar": "M20 12a8 8 0 1 1-2.3-5.7M20.5 3.5v4h-4",
        "alerta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16h.01",
        "visto": "M5 12.5l4.5 4.5L19 7.5",
        "carpeta": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
        "copiar": "M9 9h11v11H9zM5 15H4V4h11v1",
        "prioridad": "M12 20V5M6 11l6-6 6 6",
        "ventana": "M4 5h16v14H4zM4 9h16M15 13.5l-4 4M11 13.5l4 4",
        "rayo": "M13 2L4.5 13.5H12L11 22l8.5-11.5H12z",
        "info": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01",
        "nucleos": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "grafica": "M4 19V5M4 19h16M7 15l4-4 3 3 5-6"
    })

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var relleno = (nombre === "play" || nombre === "rayo") ? _hex(color) : "none"
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + relleno + '" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
