pragma Singleton
// Íconos de trazo (24x24) como SVG en línea: se ven igual en cualquier equipo. Los de Vortexine's
// (los mismos de App Installer) más los del reproductor.
import QtQuick

QtObject {
    readonly property var trazos: ({
        "play": "M7 4.5v15l12.5-7.5z",
        "pausa": "M8 5v14M16 5v14",
        "siguiente": "M5 5.5v13l9.5-6.5zM18.5 5v14",
        "anterior": "M19 5.5v13L9.5 12zM5.5 5v14",
        "aleatorio": "M16 3h5v5M4 20L21 3M21 16v5h-5M15 15l6 6M4 4l5 5",
        "repetir": "M17 2l4 4-4 4M3 11V9a3 3 0 0 1 3-3h15M7 22l-4-4 4-4M21 13v2a3 3 0 0 1-3 3H3",
        "volumen": "M11 5L6 9H2v6h4l5 4zM15.5 8.5a5 5 0 0 1 0 7M19 5a10 10 0 0 1 0 14",
        "silencio": "M11 5L6 9H2v6h4l5 4zM22 9l-6 6M16 9l6 6",
        "musica": "M9 18V5l12-2v13M9 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0zM21 16a3 3 0 1 1-6 0 3 3 0 0 1 6 0z",
        "playlist": "M4 6h12M4 11h12M4 16h7M15 20a2 2 0 1 0 0-.01M17 20V12l4-1",
        "mas": "M12 5v14M5 12h14",
        "ecualizador": "M6 4v16M12 4v16M18 4v16M3.5 8h5M9.5 15h5M15.5 10h5",
        "ajustes": "M4 6h9M17 6h3M4 12h3M11 12h9M4 18h11M19 18h1M15 4v4M9 10v4M17 16v4",
        "audifonos": "M3.5 18v-5.5a8.5 8.5 0 0 1 17 0V18M20.5 18a2.5 2.5 0 0 1-2.5 2.5h-1v-6h3.5zM3.5 18A2.5 2.5 0 0 0 6 20.5h1v-6H3.5z",
        "bocina": "M6 3h12v18H6zM12 13.5a2.5 2.5 0 1 0 0 .01M12 7h.01",
        "aleatorioPlay": "M7 4.5v15l12.5-7.5z",
        "mas_playlist": "M4 6h12M4 11h12M4 16h7M18 13v8M14 17h8",
        "derecha": "M9 5l7 7-7 7",
        "buscar": "M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4",
        "cerrar": "M6 6l12 12M18 6L6 18",
        "descargar": "M12 3v12m0 0l-4-4m4 4l4-4M4 20h16",
        "abrir": "M14 4h6v6M20 4l-9 9M18 14v6H4V6h6",
        "papelera": "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13M10 11v6M14 11v6",
        "alerta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 8v5M12 16h.01",
        "visto": "M5 12.5l4.5 4.5L19 7.5",
        "enlace": "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
        "info": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM12 11v6M12 7.5h.01",
        "carpeta": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
        "archivo": "M6 3h8l5 5v13H6zM14 3v5h5",
        "girar": "M20 12a8 8 0 1 1-2.3-5.7",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01",
        "mas_puntos": "M5 12h.01M12 12h.01M19 12h.01",
        "editar": "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4"
    })

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var relleno = (nombre === "play" || nombre === "siguiente" || nombre === "anterior") ? _hex(color) : "none"
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="' + relleno + '" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
