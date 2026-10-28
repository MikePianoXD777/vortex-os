#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2016
# Prueba de «vortex clips» (1.10, los últimos 30 s con Alt+F10) con gpu-screen-recorder, hyprctl,
# notify-send y xdg-open falsos en un HOME temporal: no graba tu pantalla ni toca tus Vídeos.
#
#   pruebas/clips.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-clips.XXXXXX")"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }
# Espera hasta 3 s a que se cumpla algo
hasta() { for _ in $(seq 30); do eval "$1" 2>/dev/null && return 0; sleep 0.1; done; return 1; }

H="$T/home"
mkdir -p "$T/bin" "$H/Vídeos" "$T/run"
echo DP-2 >"$T/foco"
echo 0 >"$T/completa"
echo '{"class":"Minecraft* 1.21.1","initialTitle":"Minecraft* 1.21.1"}' >"$T/ventana"
cat >"$T/bin/hyprctl" <<FIN
#!/bin/bash
case "\$1" in
  monitors) jq -n --arg f "\$(cat '$T/foco')" '[{name: "DP-2", focused: (\$f == "DP-2")}, {name: "HDMI-A-2", focused: (\$f == "HDMI-A-2")}]' ;;
  activewindow) jq -c --argjson c "\$(cat '$T/completa')" '. + {fullscreen: \$c}' '$T/ventana' ;;
  version) exit 0 ;;
esac
FIN
# El grabador falso anota con qué monitor arrancó y, con USR1, «guarda» un clip y llama al -sc
cat >"$T/bin/gpu-screen-recorder" <<'FIN'
#!/bin/bash
args=("$@"); mon="" sc="" out=""
for ((i = 0; i < ${#args[@]}; i++)); do
  case "${args[i]}" in -w) mon="${args[i+1]}" ;; -sc) sc="${args[i+1]}" ;; -o) out="${args[i+1]}" ;; esac
done
echo "empieza $mon $*" >>"$GSR_LOG"
guardar() { f="$out/Replay_$(date +%s%N).mp4"; echo clip >"$f"; echo "guarda $mon" >>"$GSR_LOG"; "$sc" "$f" replay; }
trap guardar USR1
trap 'echo "termina $mon" >>"$GSR_LOG"; exit 0' TERM
while true; do sleep 0.05; done
FIN
printf '#!/bin/sh\nfor a; do case "$a" in --action=*) echo carpeta; exit 0 ;; esac; done\n' >"$T/bin/notify-send"
printf '#!/bin/sh\necho "$@" >>"%s/abrir.log"\n' "$T" >"$T/bin/xdg-open"
printf '#!/bin/sh\necho "%s/Vídeos"\n' "$H" >"$T/bin/xdg-user-dir"
chmod +x "$T/bin/"*
export GSR_LOG="$T/gsr.log"
clips() {
  HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_RUNTIME_DIR="$T/run" PATH="$T/bin:$PATH" \
    VORTEX_CLIPS_PASO_MS=100 VORTEX_CLIPS_ESPERA_MS=1000 bash "$AQUI/bin/vortex-clips" "$@"
}
SERVICIO=""
terminar() { [[ -n "$SERVICIO" ]] && kill "$SERVICIO" 2>/dev/null; sleep 0.3; pkill -f "$T/bin/gpu-screen-recorder" 2>/dev/null; rm -rf "$T"; }
trap terminar EXIT

echo "== servicio"
clips servicio &
SERVICIO=$!
probar "empieza a grabar el monitor con el foco, en la RAM y 30 s" \
  "hasta \"grep -q 'empieza DP-2 .*-r 30 -replay-storage ram' '$GSR_LOG'\""
clips servicio
probar "un segundo servicio no arranca otro grabador" "sleep 0.3; [[ \$(grep -c '^empieza' '$GSR_LOG') == 1 ]]"
echo HDMI-A-2 >"$T/foco"
sleep 0.4
probar "un Alt+Tab rápido a otro monitor no lo cambia" "[[ \$(grep -c '^empieza' '$GSR_LOG') == 1 ]]"
echo DP-2 >"$T/foco"
sleep 1.3
probar "si regresas antes de 10 s se queda igual" "[[ \$(grep -c '^empieza' '$GSR_LOG') == 1 ]]"
echo HDMI-A-2 >"$T/foco"
probar "si te quedas en el otro monitor, te sigue" "hasta \"grep -q '^empieza HDMI-A-2' '$GSR_LOG' && grep -q '^termina DP-2' '$GSR_LOG'\""
echo 1 >"$T/completa"
echo DP-2 >"$T/foco"
probar "con pantalla completa va directo" "sleep 0.4; [[ \$(grep -c '^empieza DP-2' '$GSR_LOG') == 2 ]]"
probar "estado dice qué graba" "clips estado | grep -q 'grabando los últimos 30 s de DP-2'"

echo "== guardar (Alt+F10)"
clips guardar
probar "guarda un clip con el nombre del juego" "hasta \"compgen -G '$H/Vídeos/Clips/Minecraft 1.21.1 *.mp4' >/dev/null\""
probar "«Abrir carpeta» del aviso abre Vídeos/Clips" "hasta \"grep -q '$H/Vídeos/Clips' '$T/abrir.log'\""
probar "no quedan archivos Replay_ sueltos" "! compgen -G '$H/Vídeos/Clips/Replay_*' >/dev/null"

echo "== apagar / encender"
clips apagar >/dev/null
probar "apagar para el grabador y el servicio" "hasta '! kill -0 $SERVICIO' && hasta \"! pgrep -f '$T/bin/gpu-screen-recorder' >/dev/null\""
SERVICIO=""
probar "apagado se queda apagado" "[[ -e '$H/.config/vortex/clips-apagado' ]] && clips estado | grep -q apagado"
probar "guardar con los clips apagados lo dice y no arranca nada" "! clips guardar && sleep 0.3 && ! pgrep -f '$T/bin/gpu-screen-recorder' >/dev/null"
clips encender >/dev/null
probar "encender vuelve a grabar" "hasta \"pgrep -f '$T/bin/gpu-screen-recorder' >/dev/null\""
clips apagar >/dev/null

echo
if ((FALLOS)); then echo "$FALLOS fallaron"; exit 1; fi
echo "Todo bien"
