#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2016
# Prueba del resumen de la partida (1.10, vortex-partida) con sensores, nvidia-smi, hyprctl,
# notify-send y un registro de MangoHud falsos en un HOME temporal: no lee tu PC ni te avisa nada.
#
#   pruebas/partida.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-partida.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

H="$T/home"
E="$H/.local/state/vortex"
mkdir -p "$T/bin" "$H" "$T/hwmon/hwmon0" "$T/hwmon/hwmon1"
echo acpitz >"$T/hwmon/hwmon0/name"; echo 99000 >"$T/hwmon/hwmon0/temp1_input"
echo coretemp >"$T/hwmon/hwmon1/name"; echo 61000 >"$T/hwmon/hwmon1/temp1_input"
echo 55 >"$T/gpu"
echo '{"class":"steam_app_1","initialTitle":"Hollow Knight","fullscreen":2}' >"$T/ventana"
printf '#!/bin/sh\ncat "%s/gpu"\n' "$T" >"$T/bin/nvidia-smi"
printf '#!/bin/sh\n[ "$1" = activewindow ] && cat "%s/ventana"\n' "$T" >"$T/bin/hyprctl"
# Cada aviso en una línea: «título|cuerpo» (el cuerpo con « / » en vez de saltos de línea)
cat >"$T/bin/notify-send" <<FALSO
#!/bin/bash
while [[ \$1 == -* ]]; do shift; [[ \$1 != -* ]] && shift; done
printf '%s|%s\n' "\$1" "\${2//\$'\n'/ / }" >>"$T/avisos.log"
FALSO
chmod +x "$T/bin/"*
# El medidor sigue vivo si tiene el candado
midiendo() { ! flock -n "$E/partida.lock" true; }
partida() {
  HOME="$H" XDG_STATE_HOME="$H/.local/state" PATH="$T/bin:$PATH" VORTEX_HWMON="$T/hwmon" \
    VORTEX_PARTIDA_PASO=0.1 VORTEX_PARTIDA_MINIMO="${MINIMO:-0}" bash "$AQUI/bin/vortex-partida" "$@"
}

echo "== durante la partida"
partida empezar
partida empezar
sleep 0.4
probar "dos «empezar» seguidos son una sola partida (un solo medidor)" \
  "[[ \$(grep -c '^inicio=' '$E/partida') == 1 ]] && midiendo"
echo 84000 >"$T/hwmon/hwmon1/temp1_input"; echo 77 >"$T/gpu"
sleep 0.4
echo 70000 >"$T/hwmon/hwmon1/temp1_input"; echo 60 >"$T/gpu"
sleep 0.4
probar "guarda la máxima de la CPU (coretemp, no el acpitz de 99)" "grep -qx 'cpu=84' '$E/partida'"
probar "y la de la GPU (nvidia-smi)" "grep -qx 'gpu=77' '$E/partida'"
probar "el juego es la ventana en pantalla completa" "grep -qx 'juego=Hollow Knight' '$E/partida'"

echo "== al terminar"
mkdir -p "$E/mangohud"
{ echo "os,cpu,gpu"; echo "Arch,i7,RTX"; echo "fps,frametime,cpu_load"
  for _ in $(seq 99); do echo "120,8.3,40"; done; echo "30,33,90"; } >"$E/mangohud/HollowKnight_2026-10-28.csv"
partida terminar >"$T/salida"
probar "avisa con el nombre del juego" "grep -q '^Hollow Knight|Jugaste 0 min / ' '$T/avisos.log'"
probar "dice las temperaturas máximas" "grep -q 'Máximo: CPU 84 °C · GPU 77 °C' '$T/avisos.log'"
probar "FPS promedio y 1 % bajo del registro de MangoHud" "grep -q 'FPS promedio 119 · 1 % bajo 30' '$T/avisos.log'"
probar "la partida queda en el historial" "tail -1 '$E/partidas.csv' | grep -q ',Hollow Knight,0,119,30,84,77$'"
probar "el medidor se detiene solo" "sleep 0.4; ! midiendo && [[ ! -e '$E/partida' ]]"
probar "un segundo «terminar» no avisa otra vez" "partida terminar; [[ \$(wc -l <'$T/avisos.log') == 1 ]]"

echo "== partidas cortas y sin datos"
rm -f "$E/mangohud/"*.csv
MINIMO=120 partida empezar
MINIMO=120 partida terminar
probar "menos de 2 minutos: sin resumen ni historial" "[[ \$(wc -l <'$T/avisos.log') == 1 && \$(wc -l <'$E/partidas.csv') == 2 ]]"
rm -rf "$T/hwmon/hwmon1"; printf '#!/bin/sh\nexit 1\n' >"$T/bin/nvidia-smi"
echo '{"class":"kitty","fullscreen":0}' >"$T/ventana"
partida empezar
sleep 0.3
partida terminar >/dev/null
probar "sin sensores ni MangoHud solo dice cuánto jugaste" "tail -1 '$T/avisos.log' | grep -qx 'Resumen de la partida|Jugaste 0 min'"
probar "historial muestra las partidas" "partida historial | grep -q 'Hollow Knight'"

echo
if ((FALLOS)); then echo "$FALLOS fallaron"; exit 1; fi
echo "Todo bien"
