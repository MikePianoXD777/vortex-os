#!/usr/bin/env bash
# Prueba de los recordatorios del lanzador (1.10): carga shell/src/quickshell/launcher/Recordatorios.js
# en node (sin su «.pragma library») con la hora fija en un martes 28/10/2026 a las 14:00. No crea
# ningún recordatorio de verdad.
#
#   pruebas/recordatorios.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v node >/dev/null || { echo "  --  sin node: no se prueban los recordatorios"; exit 0; }

{
  tail -n +2 "$AQUI/shell/src/quickshell/launcher/Recordatorios.js"
  cat <<'JS'
let fallos = 0;
function probar(nombre, ok) { console.log((ok ? "  ok  " : "  MAL ") + nombre); if (!ok) fallos++; }
const AHORA = new Date(2026, 9, 28, 14, 0, 0).getTime();
const a = (d, h, m) => new Date(2026, 9, 28 + d, h, m || 0, 0).getTime();
function caso(frase, cuando, texto) {
  const r = interpretar(frase, AHORA);
  const ok = r !== null && r.cuando === cuando && r.texto === texto;
  probar("«" + frase + "»" + (ok ? "" : "  → " + JSON.stringify(r && { cuando: new Date(r.cuando).toString(), texto: r.texto })), ok);
}

console.log("== en cuánto");
caso("recuérdame en 20 min sacar la pizza", AHORA + 20 * 60000, "sacar la pizza");
caso("Recuérdame sacar la pizza en 20 minutos", AHORA + 20 * 60000, "sacar la pizza");
caso("recuerdame en una hora y media que ya se acabó la lavadora", AHORA + 90 * 60000, "ya se acabó la lavadora");
caso("avísame en media hora de apagar la estufa", AHORA + 30 * 60000, "apagar la estufa");
caso("recuérdame dentro de 2 h tomar la pastilla", AHORA + 2 * 3600000, "tomar la pastilla");
caso("remind me in 10 minutes to check the oven", AHORA + 10 * 60000, "check the oven");
caso("remind me to stretch in an hour", AHORA + 3600000, "stretch");
caso("recuérdame en 3 días pagar el internet", AHORA + 3 * 86400000, "pagar el internet");

console.log("== a qué hora");
caso("recuérdame a las 6 llamar a mamá", a(0, 18), "llamar a mamá");
caso("recuérdame a las 3 recoger a Irving", a(0, 15), "recoger a Irving");
caso("recuérdame a las 9 de la mañana ir al banco", a(1, 9), "ir al banco");
caso("recuérdame mañana a las 9 pagar la luz", a(1, 9), "pagar la luz");
caso("recuérdame mañana a las 5 entrenar", a(1, 17), "entrenar");
caso("recuérdame a las 18:30 el partido", a(0, 18, 30), "partido");
caso("recuérdame a la 1 de la tarde comer", a(1, 13), "comer");
caso("recuérdame a las 11 pm cerrar la puerta", a(0, 23), "cerrar la puerta");
caso("recuérdame pagar la luz a las 8 mañana", a(1, 8), "pagar la luz");
caso("remind me at 6pm to call mom", a(0, 18), "call mom");
caso("remind me tomorrow at 7:15 am to go running", a(1, 7, 15), "go running");

console.log("== lo que no es recordatorio");
probar("«en 20 min» sin «recuérdame» no cuenta", interpretar("en 20 min sacar la pizza", AHORA) === null);
probar("«recuérdame» sin hora no cuenta (todavía no hay a qué hora)", interpretar("recuérdame sacar la pizza", AHORA) === null);
probar("«a las 25» no cuenta", interpretar("recuérdame a las 25 algo", AHORA) === null);
probar("«5 km a millas» no es recordatorio", interpretar("5 km a millas", AHORA) === null);
probar("sin tarea queda vacía (se avisa «Recordatorio»)", (interpretar("recuérdame en 5 min", AHORA) || {}).texto === "");

console.log("== textos");
probar("hoy a las 18:00", cuandoTexto(a(0, 18), AHORA, false) === "hoy a las 18:00");
probar("mañana a las 09:00", cuandoTexto(a(1, 9), AHORA, false) === "mañana a las 09:00");
probar("el 3 de nov", cuandoTexto(a(6, 7, 5), AHORA, false) === "el 3 de nov a las 07:05");
probar("tomorrow at 09:00", cuandoTexto(a(1, 9), AHORA, true) === "tomorrow at 09:00");
probar("en 1 h 30 min", faltaTexto(AHORA + 90 * 60000, AHORA, false) === "en 1 h 30 min");
probar("in 20 min", faltaTexto(AHORA + 20 * 60000, AHORA, true) === "in 20 min");
probar("«mis recordatorios» pide la lista", esLista("Mis recordatorios") && esLista("reminders") && !esLista("recordatorios de ayer"));

console.log();
if (fallos) { console.log(fallos + " fallaron"); process.exit(1); }
console.log("Todo bien");
JS
} | TZ=America/Mexico_City node -
