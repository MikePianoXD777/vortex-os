#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba del metrónomo y el afinador (1.10, shell/src/quickshell/musica/musica.py) sin bocinas ni
# micrófono: pw-cat falso que guarda lo que sonaría y pw-record falso que «canta» notas generadas.
#
#   pruebas/musica.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MUSICA="$AQUI/shell/src/quickshell/musica/musica.py"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-musica-prueba.XXXXXX")"
trap 'pkill -f "$T/run" 2>/dev/null; rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
mkdir -p "$T/bin"
falso() { printf '#!/usr/bin/env bash\n%s\n' "$2" >"$T/bin/$1"; chmod +x "$T/bin/$1"; }
musica() { PATH="$T/bin:$PATH" python3 "$MUSICA" "$@"; }
musica_t() { PATH="$T/bin:$PATH" timeout 10 python3 "$MUSICA" "$@"; }

# Dónde empieza cada golpe en lo que «sonó» y qué tan fuerte es (el acento suena más).
golpes() {
  python3 -I - "$1" <<'FIN'
import sys
from array import array
a = array("h")
a.frombytes(open(sys.argv[1], "rb").read())
quieto, salida = 10**9, []
for i, v in enumerate(a):
    if abs(v) > 2000 and quieto > 2000:
        salida.append(f"{i}:{'A' if max(abs(x) for x in a[i:i + 200]) > 25000 else 'n'}")
    quieto = 0 if abs(v) > 2000 else quieto + 1
print(" ".join(salida))
FIN
}

echo "== metrónomo"
# 2 s a 48 kHz, 16 bits: pw-cat «se cierra» al llenarse y el metrónomo termina solo.
falso pw-cat "head -c 192000 >\"$T/salida\""
musica_t "$T/run" metronomo 120 4
[[ "$(golpes "$T/salida")" == "1:A 24001:n 48001:n 72001:n" ]] &&
  bien "120 por minuto en 4/4: un golpe cada medio segundo exacto, el primero con acento" || mal "golpes: $(golpes "$T/salida")"
musica_t "$T/run" metronomo 90 3
[[ "$(golpes "$T/salida")" == "1:A 32001:n 64001:n" ]] && bien "90 en 3/4" || mal "90 en 3/4: $(golpes "$T/salida")"
musica_t "$T/run" metronomo 180 1
[[ "$(golpes "$T/salida")" == "1:n 16001:n 32001:n 48001:n 64001:n 80001:n" ]] && bien "sin compás: todos iguales" || mal "sin acento: $(golpes "$T/salida")"
[[ "$(jq -r .sonando "$T/run/metronomo.json")" == false ]] && bien "si se cae el sonido, el panel lo ve parado" || mal "estado: $(cat "$T/run/metronomo.json")"
musica "$T/run" metronomo 999 99 & sleep 0.5
[[ "$(jq -c . "$T/run/metronomo-control.json")" == '{"bpm":300,"compas":12,"sonando":true}' ]] && bien "tempo y compás dentro de lo posible" || mal "límites: $(cat "$T/run/metronomo-control.json")"
wait

# Un pw-cat que nunca se llena: el metrónomo sigue hasta que lo paran.
falso pw-cat 'cat >/dev/null'
musica "$T/run" metronomo 100 4 &
DAEMON=$!
sleep 0.6
inicio=$(date +%s%N)
musica "$T/run" metronomo 70 2
fin=$(date +%s%N)
((fin - inicio < 1500000000)) && kill -0 "$DAEMON" 2>/dev/null && bien "con uno sonando, otro «metronomo» solo cambia el tempo y sale" || mal "segunda llamada"
sleep 0.5
[[ "$(jq -r '"\(.sonando) \(.bpm) \(.compas)"' "$T/run/metronomo.json")" == "true 70 2" ]] && bien "el que suena toma el tempo nuevo" || mal "cambio de tempo: $(cat "$T/run/metronomo.json")"
musica "$T/run" parar
for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$DAEMON" 2>/dev/null || break; sleep 0.2; done
! kill -0 "$DAEMON" 2>/dev/null && [[ "$(jq -r .sonando "$T/run/metronomo.json")" == false ]] && bien "«parar» lo detiene" || { mal "no se detuvo"; kill "$DAEMON"; }
musica "$T/sin-carpeta" parar && [[ ! -e "$T/sin-carpeta" ]] && bien "«parar» sin nada sonando no hace nada" || mal "parar en vacío"
rm -f "$T/bin/pw-cat"
PATH="$T/bin:/nada" /usr/bin/timeout 5 /usr/bin/python3 "$MUSICA" "$T/run2" metronomo 100 4 2>/dev/null
[[ "$(jq -r '"\(.sonando) \(.error)"' "$T/run2/metronomo.json" 2>/dev/null)" == "false pw-cat" ]] &&
  bien "sin pw-cat lo dice" || mal "sin pw-cat: $(cat "$T/run2/metronomo.json" 2>&1)"

echo "== afinador"
# tono FRECUENCIA: 1 s a 16 kHz con armónicos, como una voz o una cuerda.
tono() {
  python3 -I -c "
import math, sys
from array import array
f = float(sys.argv[1])
a = array('h', (int(6000 * (math.sin(2*math.pi*f*i/16000) + 0.5*math.sin(4*math.pi*f*i/16000) + 0.3*math.sin(6*math.pi*f*i/16000))) for i in range(16000)))
sys.stdout.buffer.write(a.tobytes())" "$1" >"$T/tono.raw"
  falso pw-record "cat \"$T/tono.raw\""
  musica afinador | tail -n 1 | jq -r '"\(.midi) \(.cents)"'
}
[[ "$(tono 440)" == "69 0" ]] && bien "La 440: La4, afinado" || mal "La: $(tono 440)"
[[ "$(tono 261.63)" == "60 0" ]] && bien "Do central" || mal "Do: $(tono 261.63)"
[[ "$(tono 98)" == "43 0" ]] && bien "Sol grave (98 Hz)" || mal "Sol: $(tono 98)"
[[ "$(tono 784)" == "79 0" ]] && bien "Sol agudo (784 Hz)" || mal "Sol5: $(tono 784)"
r="$(tono "$(python3 -c 'print(440 * 2 ** (20 / 1200))')")"
[[ "$r" == "69 20" || "$r" == "69 19" || "$r" == "69 21" ]] && bien "20 cents arriba: lo dice" || mal "+20 cents: $r"
r="$(tono "$(python3 -c 'print(329.63 * 2 ** (-30 / 1200))')")"
[[ "$r" == "64 -30" || "$r" == "64 -29" || "$r" == "64 -31" ]] && bien "Mi 30 cents abajo" || mal "-30 cents: $r"
head -c 32000 /dev/zero >"$T/tono.raw"
musica afinador | tail -n 1 | jq -e '.hz == 0 and .nivel == 0' >/dev/null && bien "en silencio no inventa notas" || mal "silencio: $(musica afinador | tail -n 1)"
rm -f "$T/bin/pw-record"
[[ "$(PATH="$T/bin:/nada" /usr/bin/python3 "$MUSICA" afinador | jq -r .error)" == pw-record ]] && bien "sin pw-record lo dice" || mal "sin pw-record"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
