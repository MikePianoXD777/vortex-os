pragma Singleton
// Íconos de trazo (24x24, esquinas redondeadas) como SVG en línea: no dependen del tema de íconos
// del sistema, así que se ven igual en cualquier equipo.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "inicio": "M3 11l9-8 9 8M5 9.5V20h5v-6h4v6h5V9.5",
        "categorias": "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
        "instaladas": "M21 8l-9-5-9 5v8l9 5 9-5zM3 8l9 5 9-5M12 13v8",
        "actualizaciones": "M20 11a8 8 0 1 0-1.8 6.5M20 4v7h-7",
        "buscar": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
        "atras": "M15 5l-7 7 7 7",
        "izquierda": "M15 5l-7 7 7 7",
        "derecha": "M9 5l7 7-7 7",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "descargar": "M12 3v12m0 0l-4-4m4 4l4-4M4 20h16",
        "abrir": "M14 4h6v6M20 4l-9 9M18 14v6H4V6h6",
        "papelera": "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
        "estrella": "M12 3l2.7 5.6 6.1.9-4.4 4.3 1 6.1L12 17l-5.4 2.9 1-6.1L3.2 9.5l6.1-.9z",
        "alerta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16h.01",
        "sinred": "M2 8.8a15 15 0 0 1 4-2.6M22 8.8A15 15 0 0 0 11 5.2M5 12.9a10 10 0 0 1 5-2.7M19 12.9a10 10 0 0 0-2.6-2.1M8.5 16.4a5 5 0 0 1 7 0M12 20h.01M3 3l18 18",
        "visto": "M5 12.5l4.5 4.5L19 7.5",
        "enlace": "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
        "info": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01",
        // categorías
        "juegos": "M7 11h4M9 9v4M15.5 12h.01M18 10h.01M17.3 5H6.7a4 4 0 0 0-3.9 3.2l-1.1 6A3 3 0 0 0 7 16.5l1.4-1.5h7.2l1.4 1.5a3 3 0 0 0 5.3-2.3l-1.1-6A4 4 0 0 0 17.3 5z",
        "productividad": "M3 8h18v12H3zM8 8V5h8v3M3 13.5h18",
        "graficos": "M4 5h16v14H4zM4 16l5-5 4 4 3-3 4 4M15 9h.01",
        "multimedia": "M9 18V6l11-2v12M9 18a3 3 0 1 1-3-3 3 3 0 0 1 3 3zM20 16a3 3 0 1 1-3-3 3 3 0 0 1 3 3z",
        "desarrollo": "M8 7l-5 5 5 5M16 7l5 5-5 5M14 4l-4 16",
        "educacion": "M2 9l10-5 10 5-10 5zM6 11.5V16c0 1.5 2.7 3 6 3s6-1.5 6-3v-4.5M22 9v6",
        "internet": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18",
        "utilidades": "M4 7h10M18 7h2M4 17h2M10 17h10M16 4v6M8 14v6",
        "sistema": "M3 5h18v12H3zM8 21h8M12 17v4",
        "comunicacion": "M4 5h16v11H10l-5 4v-4H4z",
        "ciencia": "M9 3h6M10 3v6l-5 9a2 2 0 0 0 1.8 3h10.4a2 2 0 0 0 1.8-3l-5-9V3M7.5 14h9"
    })

    // Nombre de ícono simbólico del tema («applications-games-symbolic») o nombre de categoría → ícono propio.
    function paraCategoria(nombre) {
        var n = (nombre || "").toLowerCase()
        var tabla = [
            [["game", "juego"], "juegos"],
            [["office", "productiv", "oficina"], "productividad"],
            [["graphic", "photo", "gráfic", "grafic", "imagen", "foto"], "graficos"],
            [["multimedia", "audio", "video", "vídeo", "music", "música"], "multimedia"],
            [["develop", "engineering", "code", "desarrollo", "program"], "desarrollo"],
            [["science", "education", "dictionary", "educa", "ciencia"], "educacion"],
            [["internet", "network", "web", "browser"], "internet"],
            [["chat", "mail", "comunic", "message", "social"], "comunicacion"],
            [["utilit", "accessor", "utilidad"], "utilidades"],
            [["system", "sistema", "settings"], "sistema"]
        ]
        for (var i = 0; i < tabla.length; i++)
            for (var j = 0; j < tabla[i][0].length; j++)
                if (n.indexOf(tabla[i][0][j]) >= 0)
                    return tabla[i][1]
        return "categorias"
    }

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    // «data:» con el SVG del ícono, del color y el grosor de trazo pedidos.
    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["categorias"]
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
