.pragma library
// Vortex 1.10: la lógica del buscaminas tryhard (idea de THYRVAREN). Sin nada de QML, para poder
// probarla con node (pruebas/buscaminas.sh). El tablero es un objeto con arreglos planos de w*h:
//   mina[i]  bool    num[i]  minas alrededor    estado[i]  0 oculta · 1 abierta · 2 bandera
// Las minas se ponen en el primer clic, nunca en esa casilla ni en sus 8 vecinas (abre un hueco).

var OCULTA = 0, ABIERTA = 1, BANDERA = 2;

var DIFICULTADES = {
    "facil":   { w: 9,  h: 9,  minas: 10 },
    "medio":   { w: 16, h: 16, minas: 40 },
    "experto": { w: 30, h: 16, minas: 99 },
    "tryhard": { w: 30, h: 20, minas: 145 }
};

function crear(w, h, minas) {
    var t = { w: w, h: h, minas: minas, puestas: false, perdio: false, gano: false, exploto: -1,
              mina: [], num: [], estado: [], abiertas: 0, banderas: 0 };
    for (var i = 0; i < w * h; i++) {
        t.mina.push(false);
        t.num.push(0);
        t.estado.push(OCULTA);
    }
    return t;
}

function vecinas(t, i) {
    var x = i % t.w, y = Math.floor(i / t.w), r = [];
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            var nx = x + dx, ny = y + dy;
            if (nx >= 0 && ny >= 0 && nx < t.w && ny < t.h) r.push(ny * t.w + nx);
        }
    }
    return r;
}

// Generador con semilla (mulberry32): mismo tablero con la misma semilla, para las pruebas
function azar(semilla) {
    var a = semilla >>> 0;
    return function() {
        a = (a + 0x6D2B79F5) >>> 0;
        var t = a;
        t = Math.imul(t ^ (t >>> 15), t | 1);
        t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
        return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
}

function poner(t, primera, rng) {
    var prohibidas = {};
    prohibidas[primera] = true;
    var v = vecinas(t, primera);
    for (var k = 0; k < v.length; k++) prohibidas[v[k]] = true;
    var libres = [];
    for (var i = 0; i < t.w * t.h; i++) if (!prohibidas[i]) libres.push(i);
    // Si el tablero está muy lleno, al menos la casilla del primer clic queda libre
    if (libres.length < t.minas) {
        libres = [];
        for (i = 0; i < t.w * t.h; i++) if (i !== primera) libres.push(i);
    }
    for (var n = 0; n < t.minas; n++) {
        var j = n + Math.floor(rng() * (libres.length - n));
        var tmp = libres[n]; libres[n] = libres[j]; libres[j] = tmp;
        t.mina[libres[n]] = true;
    }
    for (i = 0; i < t.w * t.h; i++) {
        var c = 0, vv = vecinas(t, i);
        for (k = 0; k < vv.length; k++) if (t.mina[vv[k]]) c++;
        t.num[i] = c;
    }
    t.puestas = true;
}

// Abre una casilla (y el hueco si es 0). Devuelve las casillas que se abrieron.
function abrir(t, i, rng) {
    if (t.perdio || t.gano || t.estado[i] !== OCULTA) return [];
    if (!t.puestas) poner(t, i, rng || Math.random);
    if (t.mina[i]) {
        t.estado[i] = ABIERTA;
        t.perdio = true;
        t.exploto = i;
        return [i];
    }
    var abiertas = [], pila = [i];
    while (pila.length) {
        var c = pila.pop();
        if (t.estado[c] !== OCULTA || t.mina[c]) continue;
        t.estado[c] = ABIERTA;
        t.abiertas++;
        abiertas.push(c);
        if (t.num[c] === 0) {
            var v = vecinas(t, c);
            for (var k = 0; k < v.length; k++) if (t.estado[v[k]] === OCULTA) pila.push(v[k]);
        }
    }
    if (t.abiertas === t.w * t.h - t.minas) ganar(t);
    return abiertas;
}

function ganar(t) {
    t.gano = true;
    // Las minas que faltaban quedan con bandera, como en todos los buscaminas
    for (var i = 0; i < t.w * t.h; i++) {
        if (t.mina[i] && t.estado[i] !== BANDERA) { t.estado[i] = BANDERA; t.banderas++; }
    }
}

// Acorde: en un número con todas sus banderas puestas, abre las vecinas que faltan
function acorde(t, i) {
    if (t.perdio || t.gano || t.estado[i] !== ABIERTA || t.num[i] === 0) return [];
    var v = vecinas(t, i), banderas = 0, k;
    for (k = 0; k < v.length; k++) if (t.estado[v[k]] === BANDERA) banderas++;
    if (banderas !== t.num[i]) return [];
    var abiertas = [];
    for (k = 0; k < v.length; k++) {
        if (t.estado[v[k]] === OCULTA) {
            abiertas = abiertas.concat(abrir(t, v[k]));
            if (t.perdio) break;
        }
    }
    return abiertas;
}

// Pone o quita la bandera. Con «muerte súbita», una bandera sobre algo que no es mina pierde.
function bandera(t, i, muerteSubita) {
    if (t.perdio || t.gano || !t.puestas) return false;
    if (t.estado[i] === OCULTA) {
        t.estado[i] = BANDERA;
        t.banderas++;
        if (muerteSubita && !t.mina[i]) {
            t.perdio = true;
            t.exploto = i;
        }
        return true;
    }
    if (t.estado[i] === BANDERA) {
        t.estado[i] = OCULTA;
        t.banderas--;
        return true;
    }
    return false;
}

// 3BV: los clics mínimos para resolverlo (cada hueco de ceros cuenta 1, más cada número que no
// toca un hueco). La medida de los tryhard: 3BV/s y eficiencia = 3BV / clics.
function tresBV(t) {
    var n = t.w * t.h, visto = [], total = 0, i, k;
    for (i = 0; i < n; i++) visto.push(false);
    for (i = 0; i < n; i++) {
        if (visto[i] || t.mina[i] || t.num[i] !== 0) continue;
        total++;
        var pila = [i];
        while (pila.length) {
            var c = pila.pop();
            if (visto[c]) continue;
            visto[c] = true;
            if (t.num[c] !== 0) continue;
            var v = vecinas(t, c);
            for (k = 0; k < v.length; k++) if (!visto[v[k]] && !t.mina[v[k]]) pila.push(v[k]);
        }
    }
    for (i = 0; i < n; i++) if (!visto[i] && !t.mina[i]) total++;
    return total;
}

// La clave de los récords: dificultad + modificadores, en orden fijo («experto+niebla+sin-banderas»)
function clave(dificultad, mods) {
    var orden = ["sin-banderas", "contrarreloj", "niebla", "muerte-subita"], partes = [dificultad];
    for (var k = 0; k < orden.length; k++) if (mods[orden[k]]) partes.push(orden[k]);
    return partes.join("+");
}

// Contrarreloj: 1 s por cada clic del 3BV en experto/tryhard (1.5 s en los fáciles)
function limite(dificultad, bv) {
    var porClic = (dificultad === "facil" || dificultad === "medio") ? 1.5 : 1.0;
    return Math.max(10, Math.round(bv * porClic));
}
