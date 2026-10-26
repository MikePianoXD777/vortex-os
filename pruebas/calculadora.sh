#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de la calculadora del lanzador (1.9): Calculadora.js con node (porcentajes, unidades y
# divisas) y divisas.sh con un curl falso (sin internet de verdad) en un HOME temporal.
#
#   pruebas/calculadora.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
L="$AQUI/shell/src/quickshell/launcher"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-calc-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

echo "== Calculadora.js"
if ! command -v node >/dev/null; then
  echo "  (sin node: me salto Calculadora.js)"
else
  # Cada línea: texto ⇥ lo que debe copiar Enter («null» = no es un cálculo; «falta» = faltan tasas).
  cat >"$T/casos" <<'FIN'
15% de 230	34.5
15 % del 1,000	150
230 + 15%	264.5
230 - 15%	195.5
5 km a millas	3.1069
5km in mi	3.1069
5 in to cm	12.7
100 f a c	37.7778
0 °C a °F	32
300 kelvin a celsius	26.85
1,5 kg a lb	3.3069
2 gb en mb	2000
1 GiB a MB	1073.7418
1 hora a minutos	60
90 km/h a mph	55.9234
2 kilómetros a metros	2000
100 usd a mxn	1850
$100 a pesos	1850
50 euros a dólares	54.35
1 mxn a usd	0.054054
100 usd a xyz	null
5 km a kg	null
7 days to die	null
firefox	null
2+2	null
-5 a 10	null
FIN
  cat >"$T/correr.js" <<'FIN'
const fs = require("fs"), vm = require("vm");
const ctx = {};
vm.runInNewContext(fs.readFileSync(process.argv[2], "utf8"), ctx);
const tasas = { USD: 1, MXN: 18.5, EUR: 0.92 };
for (const linea of fs.readFileSync(process.argv[3], "utf8").trim().split("\n")) {
  const [q, esperado] = linea.split("\t");
  const r = ctx.calcular(q, tasas);
  const obtenido = r === null ? "null" : r.valor;
  console.log((obtenido === esperado ? "ok" : "MAL") + "\t" + q + "\t" + esperado + "\t" + obtenido);
}
const sin = ctx.calcular("100 usd a mxn", {});
console.log((sin && sin.faltanTasas && sin.valor === "" ? "ok" : "MAL") + "\tsin tasas: pide el tipo de cambio\tfalta\t" + JSON.stringify(sin));
const raro = ctx.calcular("5 abc a xyz", {});
console.log((raro === null ? "ok" : "MAL") + "\tsin tasas, un código inventado no cuenta\tnull\t" + JSON.stringify(raro));
FIN
  while IFS=$'\t' read -r estado q esperado obtenido; do
    [[ "$estado" == ok ]] && bien "$q → $esperado" || mal "$q: esperaba $esperado, salió $obtenido"
  done < <(node "$T/correr.js" "$L/Calculadora.js" "$T/casos")
fi

echo "== divisas.sh"
mkdir -p "$T/bin" "$T/home"
cat >"$T/bin/curl" <<FIN
#!/bin/sh
echo "curl" >>"$T/registro"
[ -e "$T/sin-internet" ] && exit 6
while [ \$# -gt 0 ]; do [ "\$1" = -o ] && { shift; printf '%s' '{"result":"success","base_code":"USD","rates":{"USD":1,"MXN":18.7}}' >"\$1"; }; shift; done
FIN
chmod +x "$T/bin/curl"
divisas() { HOME="$T/home" XDG_CACHE_HOME="$T/home/.cache" PATH="$T/bin:$PATH" bash "$L/divisas.sh"; }
: >"$T/registro"
[[ "$(divisas)" == '{"USD":1,"MXN":18.7}' ]] && bien "baja las tasas y las imprime" || mal "salida: $(divisas)"
divisas >/dev/null
[[ "$(grep -c curl "$T/registro")" == 1 ]] && bien "las reusa (no vuelve a bajarlas en 12 h)" || mal "bajó $(grep -c curl "$T/registro") veces"
touch -d "13 hours ago" "$T/home/.cache/vortex/divisas.json"; touch "$T/sin-internet"
[[ "$(divisas)" == '{"USD":1,"MXN":18.7}' ]] && bien "sin internet, usa las últimas que había" || mal "sin internet: $(divisas)"
rm -rf "$T/home/.cache"
[[ "$(divisas)" == '{}' ]] && bien "sin internet y sin nada guardado: {}" || mal "vacío: $(divisas)"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
