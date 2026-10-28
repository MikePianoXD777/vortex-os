// Vortex 1.10: widgets en HTML sencillo. Lo comparten el widget (HtmlFace.qml) y el panel que los edita
// (htmlwidgets/HtmlWidgetsPopup.qml). El HTML lo dibuja el texto enriquecido de Qt (QTextDocument): sin
// JavaScript ni navegador, casi no gasta memoria. Sirven <h1>…<h6>, <p>, <b>, <i>, <u>, <s>, <br>, <hr>,
// <font color size>, <span style="color; font-size; background-color">, <div align>, <table>, <ul>, <ol>,
// <img src="/ruta/imagen.png" width>, <a href> y un <style> con CSS sencillo.
//
// Los datos entre llaves se llenan solos y se actualizan cada segundo:
//   {hora} 18:30   {hora12} 6:30 p. m.   {segundos} 18:30:05   {fecha} 27/10/2026   {dia} martes
//   {fechaLarga} martes, 27 de octubre   {mes} octubre   {año} 2026   {usuario} mike   {equipo} cachyos

var DATOS = ["hora", "hora12", "segundos", "fecha", "dia", "fechaLarga", "mes", "año", "usuario", "equipo"];

var DIAS = ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"];
var MESES = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre",
             "octubre", "noviembre", "diciembre"];

function dosCifras(n) { return n < 10 ? "0" + n : "" + n; }

function valores(ahora, usuario, equipo) {
    var h = ahora.getHours();
    var h12 = h % 12 === 0 ? 12 : h % 12;
    return {
        "hora": dosCifras(h) + ":" + dosCifras(ahora.getMinutes()),
        "hora12": h12 + ":" + dosCifras(ahora.getMinutes()) + (h < 12 ? " a. m." : " p. m."),
        "segundos": dosCifras(h) + ":" + dosCifras(ahora.getMinutes()) + ":" + dosCifras(ahora.getSeconds()),
        "fecha": dosCifras(ahora.getDate()) + "/" + dosCifras(ahora.getMonth() + 1) + "/" + ahora.getFullYear(),
        "dia": DIAS[ahora.getDay()],
        "fechaLarga": DIAS[ahora.getDay()] + ", " + ahora.getDate() + " de " + MESES[ahora.getMonth()],
        "mes": MESES[ahora.getMonth()],
        "año": "" + ahora.getFullYear(),
        "usuario": usuario || "",
        "equipo": equipo || ""
    };
}

// ¿Tiene datos que cambian solos? (si no, el widget no necesita su reloj de cada segundo)
function tieneDatos(html) {
    for (var i = 0; i < DATOS.length; i++) {
        if (html.indexOf("{" + DATOS[i] + "}") !== -1) return true;
    }
    return false;
}

function llenar(html, ahora, usuario, equipo) {
    var v = valores(ahora, usuario, equipo);
    return html.replace(/\{([a-zA-Zñ0-9]+)\}/g, function(todo, nombre) {
        return v.hasOwnProperty(nombre) ? v[nombre] : todo;
    });
}

var PLANTILLA =
    "<!-- Tu widget en HTML sencillo. Guarda (Ctrl+S) y se ve al instante en el escritorio. -->\n" +
    "<h2 style=\"color: #cba6f7\">¡Hola, {usuario}!</h2>\n" +
    "<p>Son las <b>{hora}</b> del {fechaLarga}.</p>\n" +
    "<p><font color=\"#a6e3a1\">Escribe aquí lo que quieras:</font> notas, una frase, tu horario…</p>\n";
