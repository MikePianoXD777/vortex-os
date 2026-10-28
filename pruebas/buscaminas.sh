#!/usr/bin/env bash
# Prueba de la lógica del buscaminas tryhard (1.10, idea de THYRVAREN): carga
# shell/src/quickshell/buscaminas/Minas.js en node (sin su «.pragma library») y juega tableros con
# semilla fija. No abre nada en tu sesión.
#
#   pruebas/buscaminas.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v node >/dev/null || { echo "  --  sin node: no se prueba el buscaminas"; exit 0; }

{
  tail -n +2 "$AQUI/shell/src/quickshell/buscaminas/Minas.js"
  cat <<'EOF'
let fallos = 0;
function probar(nombre, ok) { console.log((ok ? "  ok  " : "  MAL ") + nombre); if (!ok) fallos++; }
const cuenta = (arr, f) => arr.filter(f).length;

console.log("== primer clic y minas");
let todoBien = true, hueco = true;
for (let s = 1; s <= 300; s++) {
  const d = DIFICULTADES.tryhard, t = crear(d.w, d.h, d.minas), i = (s * 37) % (d.w * d.h);
  abrir(t, i, azar(s));
  if (t.perdio || cuenta(t.mina, m => m) !== d.minas) todoBien = false;
  if (t.mina[i] || vecinas(t, i).some(v => t.mina[v]) || t.num[i] !== 0) hueco = false;
}
probar("300 tableros tryhard: el primer clic nunca pierde y salen las 145 minas", todoBien);
probar("el primer clic siempre abre un hueco (sin minas en las 8 vecinas)", hueco);
let t = crear(3, 3, 8);
abrir(t, 4, azar(7));
probar("tablero casi lleno (3×3 con 8): la casilla del primer clic queda libre y gana", !t.mina[4] && t.gano);
probar("las esquinas tienen 3 vecinas y el centro 8", vecinas(t, 0).length === 3 && vecinas(t, 4).length === 8);

console.log("== abrir, bandera y acorde");
t = crear(9, 9, 10);
abrir(t, 40, azar(42));
probar("el hueco abre más de una casilla", t.abiertas > 1);
const misma = crear(9, 9, 10);
abrir(misma, 40, azar(42));
probar("misma semilla, mismo tablero", JSON.stringify(misma.mina) === JSON.stringify(t.mina));
let num = -1;
for (let i = 0; i < 81 && num < 0; i++) {
  if (t.estado[i] === ABIERTA && t.num[i] > 0 && vecinas(t, i).some(v => t.estado[v] === OCULTA && !t.mina[v])) num = i;
}
probar("acorde sin las banderas completas no hace nada", acorde(t, num).length === 0);
for (const v of vecinas(t, num)) if (t.mina[v]) bandera(t, v, false);
const antes = t.abiertas;
acorde(t, num);
probar("acorde con sus banderas abre las vecinas que faltan", t.abiertas > antes && !t.perdio);
const b = vecinas(t, num).find(v => t.estado[v] === BANDERA);
probar("una bandera no se abre", abrir(t, b).length === 0 && t.estado[b] === BANDERA);
bandera(t, b, false);
probar("quitar la bandera la deja oculta", t.estado[b] === OCULTA && t.banderas === cuenta(t.estado, e => e === BANDERA));
const mala = t.estado.findIndex((e, i) => e === OCULTA && !t.mina[i]);
for (const v of vecinas(t, num)) if (t.mina[v] && t.estado[v] === OCULTA) bandera(t, v, false);
bandera(t, mala, false);
probar("bandera mala normal: no pierdes", !t.perdio);
bandera(t, mala, false);
bandera(t, mala, true);
probar("bandera mala con muerte súbita: pierdes ahí", t.perdio && t.exploto === mala);

console.log("== ganar y perder");
t = crear(16, 16, 40);
abrir(t, 0, azar(9));
for (let i = 0; i < 256; i++) if (!t.mina[i]) abrir(t, i);
probar("abrir todo lo que no es mina gana", t.gano && !t.perdio);
probar("al ganar, las minas quedan con bandera", t.mina.every((m, i) => !m || t.estado[i] === BANDERA));
t = crear(16, 16, 40);
abrir(t, 0, azar(9));
const mina = t.mina.indexOf(true);
abrir(t, mina);
probar("abrir una mina pierde y marca dónde", t.perdio && t.exploto === mina);
probar("después de perder ya no se juega", abrir(t, t.mina.indexOf(false, 1)).length === 0 && !bandera(t, 5, false));

console.log("== 3BV, récords y contrarreloj");
// 3×1 sin minas: un solo hueco = 1 clic
t = crear(3, 1, 0); t.puestas = true;
probar("sin minas el 3BV es 1", tresBV(t) === 1);
// 3×1 con la mina en medio: 2 números sueltos = 2 clics
t = crear(3, 1, 1); t.mina = [false, true, false]; t.num = [1, 0, 1];
probar("dos números sueltos = 2", tresBV(t) === 2);
// 5×1 con la mina al final: el hueco abre también el número que lo toca = 1
t = crear(5, 1, 1); t.mina = [false, false, false, false, true]; t.num = [0, 0, 0, 1, 0];
probar("un número pegado a un hueco no suma", tresBV(t) === 1);
t = crear(30, 16, 99);
abrir(t, 0, azar(3));
const bv = tresBV(t);
probar("experto: 3BV entre 50 y 300 (" + bv + ")", bv > 50 && bv < 300);
probar("la clave del récord no depende del orden", clave("experto", { "niebla": true, "sin-banderas": true }) === "experto+sin-banderas+niebla");
probar("sin modificadores la clave es solo la dificultad", clave("tryhard", {}) === "tryhard");
probar("contrarreloj: experto 1 s por clic, fácil 1.5 s, mínimo 10", limite("experto", 150) === 150 && limite("facil", 20) === 30 && limite("facil", 3) === 10);

console.log();
if (fallos) { console.log(fallos + " fallaron"); process.exit(1); }
console.log("Todo bien");
EOF
} | node -
