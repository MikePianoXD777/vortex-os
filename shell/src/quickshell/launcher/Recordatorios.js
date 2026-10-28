.pragma library
// Vortex 1.10: los recordatorios del lanzador. Entiende frases como «recuérdame en 20 min sacar la
// pizza», «recuérdame sacar la pizza en 20 min», «recuérdame mañana a las 9 pagar la luz», «avísame a
// las 6 llamar a mamá» o «remind me at 6pm to call mom». Sin nada de QML, para probarlo con node
// (pruebas/recordatorios.sh). Los guarda y los hace sonar singletons/system/Recordatorios.qml.

// «é» → «e» sin cambiar el largo del texto (las posiciones sirven para cortar el original)
function sinAcentos(s) {
    return s.normalize("NFC").toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "");
}

var INICIO = /^\s*(?:recuerdame|recordar|recordatorio:?|avisame|remind me|reminder:?)\s+/;

var NUMEROS = { "un": 1, "una": 1, "uno": 1, "a": 1, "an": 1, "dos": 2, "two": 2, "tres": 3, "three": 3,
                "cinco": 5, "five": 5, "diez": 10, "ten": 10, "quince": 15, "fifteen": 15, "veinte": 20,
                "twenty": 20, "treinta": 30, "thirty": 30, "media": 0.5, "half": 0.5 };

var UNIDADES = [
    [/^(?:s|seg|segs|segundos?|secs?|seconds?)$/, 1000],
    [/^(?:m|min|mins|minutos?|minutes?)$/, 60000],
    [/^(?:h|hr|hrs|horas?|hours?)$/, 3600000],
    [/^(?:d|dias?|days?)$/, 86400000]
];

// «en 20 min», «en una hora y media», «en media hora», «in 2 hours», «dentro de 10 minutos»
var RELATIVO = /(?:^|\s)(?:en|dentro de|in)\s+(\d+(?:[.,]\d+)?|un|una|uno|a|an|dos|two|tres|three|cinco|five|diez|ten|quince|fifteen|veinte|twenty|treinta|thirty|media|half)\s*(?:an?\s+)?(s|seg|segs|segundos?|secs?|seconds?|m|min|mins|minutos?|minutes?|h|hr|hrs|horas?|hours?|d|dias?|days?)\b(\s+y\s+media|\s+and\s+a\s+half)?/;

// «a las 6», «a las 18:30», «a la 1 de la tarde», «at 6pm», con «mañana» / «hoy» / «tomorrow» antes o después
var DIA = "(manana|pasado manana|hoy|tomorrow|today)";
var HORA = "(?:a las|a la|at)\\s+(\\d{1,2})(?:[:.](\\d{2}))?\\s*(a\\.?\\s?m\\.?|p\\.?\\s?m\\.?|de la manana|de la tarde|de la noche|de la madrugada|in the morning|in the afternoon|in the evening|at night)?";
var ABSOLUTO = new RegExp("(?:^|\\s)(?:" + DIA + "\\s+)?" + HORA + "(?:\\s+" + DIA + ")?(?=\\s|$)");
var SOLO_DIA = new RegExp("(?:^|\\s)" + DIA + "(?=\\s|$)");

function numero(s) {
    if (NUMEROS[s] !== undefined) return NUMEROS[s];
    return parseFloat(s.replace(",", "."));
}

// La hora en 24 h según «pm», «p. m.», «de la tarde»…; -1 si no dice
function horaDe(h, sufijo) {
    var s = sufijo.replace(/[\s.]/g, "");
    if (!s) return -1;
    if (/^(?:p|delatarde|delanoche|intheafternoon|intheevening|atnight)/.test(s)) return h % 12 + 12;
    return h % 12; // am, de la mañana, de la madrugada
}

// Devuelve { cuando: ms, texto: "sacar la pizza" } o null si la frase no es un recordatorio
function interpretar(frase, ahora) {
    var original = String(frase).normalize("NFC");
    var bajo = sinAcentos(original);
    var inicio = bajo.match(INICIO);
    if (!inicio) return null;
    var resto = original.slice(inicio[0].length), r = bajo.slice(inicio[0].length);
    var cuando = -1, m, quitar = null;

    if ((m = r.match(RELATIVO))) {
        var cantidad = numero(m[1]);
        var unidad = 0;
        for (var k = 0; k < UNIDADES.length; k++) if (UNIDADES[k][0].test(m[2])) unidad = UNIDADES[k][1];
        if (m[3]) cantidad += 0.5;
        if (!(cantidad > 0) || unidad === 0) return null;
        cuando = ahora + Math.round(cantidad * unidad);
        quitar = m;
    } else if ((m = r.match(ABSOLUTO))) {
        var h = parseInt(m[2], 10), min = m[3] ? parseInt(m[3], 10) : 0;
        if (h > 23 || min > 59) return null;
        var dia = m[1] || m[5] || "";
        var base = new Date(ahora);
        base.setSeconds(0, 0);
        var conSufijo = horaDe(h, m[4] || "");
        var opciones;
        if (conSufijo >= 0) opciones = [conSufijo];
        else if (h >= 13 || h === 0) opciones = [h];
        else if (dia === "manana" || dia === "tomorrow" || dia === "pasado manana") opciones = [h <= 6 ? h + 12 : h];
        else opciones = h === 12 ? [12] : [h, h + 12]; // «a las 6» = la próxima 6
        var dias = dia === "manana" || dia === "tomorrow" ? 1 : dia === "pasado manana" ? 2 : 0;
        var mejor = -1;
        for (var d = dias; d <= dias + 1 && mejor < 0; d++) {
            for (var o = 0; o < opciones.length; o++) {
                var c = new Date(base.getTime());
                c.setDate(c.getDate() + d);
                c.setHours(opciones[o], min, 0, 0);
                if (c.getTime() > ahora && (mejor < 0 || c.getTime() < mejor)) mejor = c.getTime();
            }
            if (dias > 0 && mejor >= 0) break;
        }
        if (mejor < 0) return null;
        cuando = mejor;
        quitar = m;
    } else {
        return null;
    }

    // La tarea es lo que queda sin la hora, sin «que», «de», «to»… del principio
    var antes = resto.slice(0, quitar.index), despues = resto.slice(quitar.index + quitar[0].length);
    var texto = (antes + " " + despues).replace(/\s+/g, " ").trim();
    texto = texto.replace(/^(?:que|de|para|to|about|el|la)\s+/i, "").replace(/[\s,.;:]+$/, "").trim();
    return { cuando: cuando, texto: texto };
}

// «hoy a las 18:42», «mañana a las 9:00», «el 3 de nov a las 7:00»; en, si es inglés
function cuandoTexto(cuando, ahora, ingles) {
    var c = new Date(cuando), a = new Date(ahora);
    var hh = String(c.getHours()).padStart(2, "0") + ":" + String(c.getMinutes()).padStart(2, "0");
    var hoy = new Date(a.getFullYear(), a.getMonth(), a.getDate()).getTime();
    var diaC = new Date(c.getFullYear(), c.getMonth(), c.getDate()).getTime();
    var dif = Math.round((diaC - hoy) / 86400000);
    if (ingles) {
        var meses = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
        if (dif === 0) return "today at " + hh;
        if (dif === 1) return "tomorrow at " + hh;
        return meses[c.getMonth()] + " " + c.getDate() + " at " + hh;
    }
    var meses2 = ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sep", "oct", "nov", "dic"];
    if (dif === 0) return "hoy a las " + hh;
    if (dif === 1) return "mañana a las " + hh;
    if (dif === 2) return "pasado mañana a las " + hh;
    return "el " + c.getDate() + " de " + meses2[c.getMonth()] + " a las " + hh;
}

// «en 20 min», «en 1 h 30 min», «in 2 h»
function faltaTexto(cuando, ahora, ingles) {
    var min = Math.max(0, Math.round((cuando - ahora) / 60000));
    var pre = ingles ? "in " : "en ";
    if (min < 1) return ingles ? "in less than a minute" : "en menos de un minuto";
    if (min < 60) return pre + min + " min";
    var h = Math.floor(min / 60), m = min % 60;
    if (h < 48) return pre + h + " h" + (m ? " " + m + " min" : "");
    return pre + Math.round(h / 24) + (ingles ? " days" : " días");
}

// La frase para pedir la lista: «recordatorios», «mis recordatorios», «reminders»
function esLista(frase) {
    return /^\s*(?:mis\s+)?(?:recordatorios|reminders)\s*$/.test(sinAcentos(String(frase)));
}
