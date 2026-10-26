// Vortex 1.9: lo que la calculadora del lanzador (Launcher.qml, evaluateMath) no entendía:
//
//   15% de 230        → 34.5            230 + 15%  → 264.5         230 - 15%  → 195.5
//   5 km a millas     → 3.1069 mi       100 f a c  → 37.7778 °C    2 gb en mb → 2000 MB
//   100 usd a mxn     → con las tasas de divisas.sh (base USD)
//
// calcular(texto, tasas) devuelve null si no es nada de eso, o {mostrar, valor, divisa, faltanTasas}:
// «mostrar» va en la lista, «valor» es lo que Enter copia (vacío si faltan las tasas). Acepta español
// e inglés, con o sin acentos, y la coma decimal («1,5 kg a lb»). Sin .pragma library para que
// pruebas/calculadora.js lo cargue con node.

var UNIDADES = [
    // [categoría, símbolo, factor a la unidad base, alias...]
    ["longitud", "mm", 0.001, "milimetro", "milimetros", "millimeter", "millimeters"],
    ["longitud", "cm", 0.01, "centimetro", "centimetros", "centimeter", "centimeters"],
    ["longitud", "m", 1, "metro", "metros", "meter", "meters", "metre", "metres"],
    ["longitud", "km", 1000, "kilometro", "kilometros", "kilometer", "kilometers", "kms"],
    ["longitud", "in", 0.0254, "pulgada", "pulgadas", "inch", "inches", "\""],
    ["longitud", "ft", 0.3048, "pie", "pies", "foot", "feet", "'"],
    ["longitud", "yd", 0.9144, "yarda", "yardas", "yard", "yards"],
    ["longitud", "mi", 1609.344, "milla", "millas", "mile", "miles"],
    ["masa", "mg", 0.000001, "miligramo", "miligramos", "milligram", "milligrams"],
    ["masa", "g", 0.001, "gramo", "gramos", "gram", "grams", "gr"],
    ["masa", "kg", 1, "kilo", "kilos", "kilogramo", "kilogramos", "kilogram", "kilograms", "kgs"],
    ["masa", "t", 1000, "tonelada", "toneladas", "ton", "tons", "tonne", "tonnes"],
    ["masa", "oz", 0.028349523125, "onza", "onzas", "ounce", "ounces"],
    ["masa", "lb", 0.45359237, "lbs", "libra", "libras", "pound", "pounds"],
    ["volumen", "ml", 0.001, "mililitro", "mililitros", "milliliter", "milliliters"],
    ["volumen", "l", 1, "litro", "litros", "liter", "liters", "litre", "litres", "lt"],
    ["volumen", "gal", 3.785411784, "galon", "galones", "gallon", "gallons"],
    ["volumen", "fl oz", 0.0295735295625, "floz", "onza liquida", "onzas liquidas", "fluid ounce", "fluid ounces"],
    ["volumen", "taza", 0.2365882365, "tazas", "cup", "cups"],
    ["velocidad", "km/h", 1 / 3.6, "kmh", "kph", "kilometros por hora"],
    ["velocidad", "m/s", 1, "mps", "metros por segundo"],
    ["velocidad", "mph", 0.44704, "millas por hora", "miles per hour"],
    ["velocidad", "nudos", 0.514444, "nudo", "knot", "knots", "kn"],
    ["datos", "B", 1, "b", "byte", "bytes"],
    ["datos", "KB", 1e3, "kb", "kilobyte", "kilobytes"],
    ["datos", "MB", 1e6, "mb", "megabyte", "megabytes", "megas"],
    ["datos", "GB", 1e9, "gb", "gigabyte", "gigabytes", "gigas"],
    ["datos", "TB", 1e12, "tb", "terabyte", "terabytes", "teras"],
    ["datos", "KiB", 1024, "kib"],
    ["datos", "MiB", 1048576, "mib"],
    ["datos", "GiB", 1073741824, "gib"],
    ["datos", "TiB", 1099511627776, "tib"],
    ["tiempo", "ms", 0.001, "milisegundo", "milisegundos", "millisecond", "milliseconds"],
    ["tiempo", "s", 1, "seg", "segs", "segundo", "segundos", "sec", "secs", "second", "seconds"],
    ["tiempo", "min", 60, "mins", "minuto", "minutos", "minute", "minutes"],
    ["tiempo", "h", 3600, "hr", "hrs", "hora", "horas", "hour", "hours"],
    ["tiempo", "días", 86400, "d", "dia", "dias", "day", "days"],
    ["tiempo", "semanas", 604800, "semana", "week", "weeks"],
    ["tiempo", "años", 31557600, "ano", "anos", "year", "years"],
    ["area", "m²", 1, "m2", "metro cuadrado", "metros cuadrados", "square meter", "square meters"],
    ["area", "km²", 1e6, "km2", "kilometro cuadrado", "kilometros cuadrados"],
    ["area", "ha", 1e4, "hectarea", "hectareas", "hectare", "hectares"],
    ["area", "acres", 4046.8564224, "acre"],
    ["area", "ft²", 0.09290304, "ft2", "pie cuadrado", "pies cuadrados", "square feet"],
    ["temperatura", "°C", 0, "c", "celsius", "centigrados", "grados", "grados celsius"],
    ["temperatura", "°F", 0, "f", "fahrenheit", "grados fahrenheit"],
    ["temperatura", "K", 0, "k", "kelvin", "kelvins"]
];

var DIVISAS = {
    "dolar": "USD", "dolares": "USD", "dollar": "USD", "dollars": "USD", "$": "USD", "us$": "USD",
    "euro": "EUR", "euros": "EUR", "€": "EUR",
    "peso": "MXN", "pesos": "MXN", "pesos mexicanos": "MXN", "peso mexicano": "MXN",
    "libra esterlina": "GBP", "libras esterlinas": "GBP", "£": "GBP",
    "yen": "JPY", "yenes": "JPY", "¥": "JPY",
    "real": "BRL", "reales": "BRL",
    "peso argentino": "ARS", "pesos argentinos": "ARS",
    "peso colombiano": "COP", "pesos colombianos": "COP",
    "peso chileno": "CLP", "pesos chilenos": "CLP",
    "sol": "PEN", "soles": "PEN",
    "bitcoin": "BTC"
};

var CODIGOS = ["USD", "EUR", "MXN", "GBP", "JPY", "CAD", "AUD", "CHF", "CNY", "BRL", "ARS", "COP", "CLP",
               "PEN", "UYU", "BOB", "PYG", "VES", "GTQ", "HNL", "NIO", "CRC", "PAB", "DOP", "CUP", "KRW",
               "INR", "RUB", "TRY", "SEK", "NOK", "DKK", "PLN", "CZK", "HUF", "NZD", "SGD", "HKD", "BTC"];

function sinAcentos(t) {
    return t.replace(/[áàä]/g, "a").replace(/[éèë]/g, "e").replace(/[íìï]/g, "i")
            .replace(/[óòö]/g, "o").replace(/[úùü]/g, "u");
}

// «1,5» → 1.5; «1,000» y «1.000,50» como miles; «12.5» tal cual.
function numero(t) {
    t = t.replace(/\s/g, "");
    if (!/^-?[\d.,]+$/.test(t) || !/\d/.test(t)) return NaN;
    var coma = t.lastIndexOf(","), punto = t.lastIndexOf(".");
    if (coma >= 0 && punto >= 0) {
        t = coma > punto ? t.replace(/\./g, "").replace(",", ".") : t.replace(/,/g, "");
    } else if (coma >= 0) {
        t = (/^-?\d{1,3}(,\d{3})+$/.test(t)) ? t.replace(/,/g, "") : t.replace(",", ".");
    }
    if ((t.match(/\./g) || []).length > 1) return NaN;
    return parseFloat(t);
}

function formatear(n, decimales) {
    if (!isFinite(n)) return "";
    var a = Math.abs(n);
    if (a !== 0 && (a >= 1e15 || a < 1e-4)) return Number(n.toPrecision(6)).toString();
    var r = Number(n.toFixed(decimales)).toString();
    return r === "-0" ? "0" : r;
}

function unidad(t) {
    t = t.trim();
    for (var i = 0; i < UNIDADES.length; i++) {
        var u = UNIDADES[i];
        if (t === u[1] || t === u[1].toLowerCase()) return u;
        for (var j = 3; j < u.length; j++) if (t === u[j]) return u;
    }
    return null;
}

function divisa(t, tasas) {
    t = t.trim();
    if (DIVISAS.hasOwnProperty(t)) return DIVISAS[t];
    if (/^[a-z]{3}$/.test(t)) {
        var c = t.toUpperCase();
        if (tasas && Object.keys(tasas).length > 0) return tasas.hasOwnProperty(c) ? c : null;
        return CODIGOS.indexOf(c) >= 0 ? c : null; // sin tasas todavía: solo las conocidas
    }
    return null;
}

function temperatura(v, de, a) {
    var c = de === "°C" ? v : de === "°F" ? (v - 32) * 5 / 9 : v - 273.15;
    return a === "°C" ? c : a === "°F" ? c * 9 / 5 + 32 : c + 273.15;
}

function calcular(texto, tasas) {
    if (!texto) return null;
    var t = sinAcentos(String(texto).toLowerCase().trim()).replace(/\s+/g, " ");
    var NUM = "(-?\\d[\\d.,]*)";
    var m;

    // 15% de 230
    m = t.match(new RegExp("^" + NUM + " ?% ?(?:de|del|of) " + NUM + "$"));
    if (m) {
        var p = numero(m[1]), base = numero(m[2]);
        if (isNaN(p) || isNaN(base)) return null;
        var r = formatear(p * base / 100, 6);
        return { mostrar: texto.trim() + " = " + r, valor: r, divisa: false, faltanTasas: false };
    }
    // 230 + 15%   /   230 - 15%
    m = t.match(new RegExp("^" + NUM + " ?([+-]) ?" + NUM + " ?%$"));
    if (m) {
        var b = numero(m[1]), q = numero(m[3]);
        if (isNaN(b) || isNaN(q)) return null;
        var s = formatear(b * (1 + (m[2] === "+" ? q : -q) / 100), 6);
        return { mostrar: texto.trim() + " = " + s, valor: s, divisa: false, faltanTasas: false };
    }

    // 5 km a millas  /  5km in mi  /  100 usd a mxn  /  $100 a mxn
    m = t.match(/^([$€£¥])? ?(-?\d[\d.,]*) ?(.*?) (?:a|en|to|in|=|->|→) (.+)$/);
    if (!m) return null;
    var v = numero(m[2]);
    var origen = (m[3] || "").trim() || (m[1] || "");
    var destino = m[4].trim();
    if (isNaN(v) || origen === "") return null;

    var u1 = unidad(origen), u2 = unidad(destino);
    if (u1 && u2 && u1[0] === u2[0]) {
        var res = u1[0] === "temperatura" ? temperatura(v, u1[1], u2[1]) : v * u1[2] / u2[2];
        var f = formatear(res, 4);
        return { mostrar: formatear(v, 6) + " " + u1[1] + " = " + f + " " + u2[1], valor: f, divisa: false, faltanTasas: false };
    }

    var d1 = divisa(origen, tasas), d2 = divisa(destino, tasas);
    if (!d1 || !d2) return null;
    var hayTasas = tasas && tasas.hasOwnProperty(d1) && tasas.hasOwnProperty(d2) && tasas[d1] > 0;
    if (!hayTasas) {
        return { mostrar: formatear(v, 2) + " " + d1 + " → " + d2, valor: "", divisa: true, faltanTasas: true };
    }
    var x = v / tasas[d1] * tasas[d2];
    var fx = formatear(x, Math.abs(x) >= 1 ? 2 : 6);
    return { mostrar: formatear(v, 2) + " " + d1 + " = " + fx + " " + d2, valor: fx, divisa: true, faltanTasas: false };
}
