#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba del ecualizador del reproductor (1.10, idea de THYRVAREN: frecuencias editables y más bandas)
# con un HOME temporal y un easyeffects falso: no toca tu sonido ni tu preset de EasyEffects.
#
#   pruebas/ecualizador.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-eq.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }
# EasyEffects se lanza en segundo plano: espera a que aparezca su registro
espera_log() { for _ in $(seq 20); do [[ -s "$T/easyeffects.log" ]] && return 0; sleep 0.1; done; return 1; }

H="$T/home"
mkdir -p "$T/bin" "$H"
# easyeffects falso: «-l PRESET» queda anotado; pgrep dice que ya corre
printf '#!/bin/sh\necho "$@" >>"%s/easyeffects.log"\n' "$T" >"$T/bin/easyeffects"
printf '#!/bin/sh\nexit 0\n' >"$T/bin/pgrep"
chmod +x "$T/bin/"*
eq() {
  HOME="$H" XDG_STATE_HOME="$H/.local/state" XDG_CACHE_HOME="$H/.cache" XDG_DATA_HOME="$H/.local/share" \
    XDG_RUNTIME_DIR="$T/run" PATH="$T/bin:$PATH" bash "$AQUI/shell/src/quickshell/media/equalizer.sh" "$@"
}
ESTADO="$H/.local/state/serpantinum/music/eq_state.json"
PRESET="$H/.local/share/easyeffects/output/live_eq.json"
mkdir -p "$T/run"

echo "== estado nuevo y estado viejo"
eq get >/dev/null
[[ -f "$ESTADO" ]] || ESTADO="$(find "$H" -name eq_state.json | head -1)"
probar "empieza con las 10 bandas de siempre en plano" \
  "jq -e '(.bandas | map(.f)) == [31,63,125,250,500,1000,2000,4000,8000,16000] and all(.bandas[]; .g == 0)' '$ESTADO' >/dev/null"
echo '{"b1":"5","b2":7,"b3":5,"b4":2,"b5":1,"b6":0,"b7":0,"b8":0,"b9":1,"b10":2,"preset":"Bass","pending":false}' >"$ESTADO"
eq get >/dev/null
probar "un estado viejo (b1…b10) pasa a la lista con sus mismas ganancias" \
  "jq -e '(.bandas | map(.g)) == [5,7,5,2,1,0,0,0,1,2] and (has(\"b1\") | not) and .preset == \"Bass\" and .dynamics == \"off\"' '$ESTADO' >/dev/null"

echo "== editar bandas"
eq set_band 3 9
probar "set_band cambia esa banda y queda pendiente" "jq -e '.bandas[2].g == 9 and .pending and .preset == \"Custom\"' '$ESTADO' >/dev/null"
eq set_band 1 40
probar "la ganancia se queda entre -12 y 12" "jq -e '.bandas[0].g == 12' '$ESTADO' >/dev/null"
probar "una banda que no existe no rompe el estado" "! eq set_band 20 3 2>/dev/null && jq -e '.bandas | length == 10' '$ESTADO' >/dev/null"
eq set_bands "1000:3,45:-2,90:0,14000:6,25:1"
probar "set_bands guarda la lista ordenada por frecuencia" \
  "jq -e '(.bandas | map(.f)) == [25,45,90,1000,14000] and .bandas[0].g == 1 and .pending' '$ESTADO' >/dev/null"
eq set_bands "5:0,99999:0,440.4:-30"
probar "las frecuencias se quedan entre 20 Hz y 20 kHz" "jq -e '(.bandas | map(.f)) == [20,440,20000] and .bandas[1].g == -12' '$ESTADO' >/dev/null"
antes="$(cat "$ESTADO")"
probar "menos de 2 bandas no se acepta" "! eq set_bands '1000:0' 2>/dev/null && [[ \"\$(cat '$ESTADO')\" == '$antes' ]]"
probar "más de 16 tampoco" "! eq set_bands \"\$(seq -s, 100 100 1700 | sed 's/,/:0,/g'):0\" 2>/dev/null"
probar "basura no se acepta" "! eq set_bands 'hola; rm -rf ~' 2>/dev/null && [[ \"\$(cat '$ESTADO')\" == '$antes' ]]"
eq set_bands "60:2,1000:0" guardado
probar "con «guardado» no queda pendiente" "jq -e '(.pending | not) and (.bandas | length) == 2' '$ESTADO' >/dev/null"

echo "== aplicar en EasyEffects"
eq set_bands "31:4,63:0,125:0,250:0,500:0,1000:0,2000:0,4000:0,8000:0,12000:0,14000:-3,16000:0,18000:2"
eq apply
probar "el preset trae las 13 bandas" "jq -e '.output.equalizer[\"num-bands\"] == 13 and (.output.equalizer.left | length) == 13' '$PRESET' >/dev/null"
probar "cada banda con su frecuencia y ganancia" \
  "jq -e '.output.equalizer.left.band10 == (.output.equalizer.left.band10 | .frequency = 14000 | .gain = -3) and .output.equalizer.left.band0.gain == 4' '$PRESET' >/dev/null"
probar "las bandas juntas son más angostas (Q más alto) que las separadas" \
  "jq -e '.output.equalizer.left.band11.q > .output.equalizer.left.band1.q' '$PRESET' >/dev/null"
probar "se carga en EasyEffects" "espera_log && grep -q -- '-l live_eq' '$T/easyeffects.log'"
eq preset Rock
probar "un preset vuelve a las 10 bandas de siempre" \
  "jq -e '(.bandas | map(.f)) == [31,63,125,250,500,1000,2000,4000,8000,16000] and (.bandas | map(.g)) == [5,4,2,-1,-2,-1,2,4,5,6] and .preset == \"Rock\" and (.pending | not)' '$ESTADO' >/dev/null"
probar "y el preset de EasyEffects también" "jq -e '.output.equalizer[\"num-bands\"] == 10' '$PRESET' >/dev/null"

echo "== restaurar al iniciar sesión"
rm -f "$T/easyeffects.log"
eq restaurar
probar "con ganancias, restaurar lo aplica" "espera_log"
eq preset Flat
espera_log
rm -f "$T/easyeffects.log"
eq restaurar
probar "en plano, restaurar no arranca EasyEffects" "! espera_log"

echo
if ((FALLOS)); then echo "$FALLOS fallaron"; exit 1; fi
echo "Todo bien"
