pragma Singleton
// Íconos de trazo (24x24, esquinas redondeadas) como SVG en línea: no dependen del tema de íconos
// del sistema, así que se ven igual en cualquier equipo. Los mismos de Vortexine's Shop, más los de
// los tipos de archivo.
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
        // tipos de archivo y acciones del instalador
        "caja": "M3 4h18v5H3zM5 9v11h14V9M10 13h4",
        "carpeta": "M3 6.5A1.5 1.5 0 0 1 4.5 5H9l2 2h8.5A1.5 1.5 0 0 1 21 8.5v9a1.5 1.5 0 0 1-1.5 1.5h-15A1.5 1.5 0 0 1 3 17.5z",
        "archivo": "M6 3h8l5 5v13H6zM14 3v5h5",
        "programa": "M3 5h18v14H3zM3 9h18M6.5 7h.01M9.5 7h.01",
        "terminal": "M4 5h16v14H4zM8 10l3 2.5L8 15M13 15h3",
        "windows": "M4 6l7-1v6.5H4zM13 4.7l7-1.2v8H13zM4 13h7v6.5l-7-1zM13 13h7v7.5l-7-1.2z",
        "paquete": "M21 8l-9-5-9 5v8l9 5 9-5zM3 8l9 5 9-5M12 13v8M7.5 5.5l9 5",
        "cubo": "M12 3l8 4.5v9L12 21l-8-4.5v-9zM4 7.5l8 4.5 8-4.5M12 12v9",
        "java": "M6 10h11v4a5 5 0 0 1-5 5h-1a5 5 0 0 1-5-5zM17 11h1.5a2 2 0 0 1 0 4H17M9.5 3c-1 1.5 1 2.5 0 4.5M13.5 3c-1 1.5 1 2.5 0 4.5",
        "pregunta": "M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6V14M12 17h.01",
        "copiar": "M9 9h11v11H9zM5 15H4V4h11v1",
        "escudo": "M12 3l8 3v6c0 4.5-3.4 8.2-8 9-4.6-.8-8-4.5-8-9V6zM12 8v5M12 16h.01",
        "candado": "M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4",
        "girar": "M20 12a8 8 0 1 1-2.3-5.7"
    })

    // El ícono de cada tipo de archivo (los tipos de detectar.py).
    function paraTipo(tipo) {
        var tabla = { "comprimido": "caja", "programa": "programa", "appimage": "programa", "script": "terminal",
                      "deb": "paquete", "rpm": "paquete", "flatpakref": "paquete", "flatpak": "paquete",
                      "windows": "windows", "mod": "cubo", "jar": "java", "carpeta": "carpeta",
                      "descomprimido": "carpeta", "falta": "alerta" }
        return tabla[tipo] || "pregunta"
    }

    function _hex(c) {
        function h(v) { var s = Math.round(v * 255).toString(16); return s.length < 2 ? "0" + s : s }
        return "#" + h(c.r) + h(c.g) + h(c.b)
    }

    // «data:» con el SVG del ícono, del color y el grosor de trazo pedidos.
    function uri(nombre, color, trazo) {
        var d = trazos[nombre] || trazos["pregunta"]
        var svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="' + _hex(color)
                + '" stroke-width="' + (trazo || 2) + '" stroke-linecap="round" stroke-linejoin="round"><path d="' + d + '"/></svg>'
        return "data:image/svg+xml;utf8," + encodeURIComponent(svg)
    }
}
