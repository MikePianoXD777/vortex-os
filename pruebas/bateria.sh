#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex bateria» (1.8, modo laptop) con una laptop falsa: hyprctl (pantalla eDP-1 a 144 Hz
# y un monitor externo), powerprofilesctl y brightnessctl falsos en un PATH temporal y un HOME temporal.
#
#   pruebas/bateria.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-bateria-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config/serpantinum" "$T/home/.local/state/vortex"
MONITORES='[{"name":"eDP-1","width":1920,"height":1080,"refreshRate":144.0,"scale":1.25,
  "availableModes":["1920x1080@144.00Hz","1920x1080@60.01Hz","1920x1080@48.00Hz","1280x720@60.00Hz"]},
 {"name":"HDMI-A-1","width":2560,"height":1440,"refreshRate":165.0,"scale":1.0,
  "availableModes":["2560x1440@165.00Hz","2560x1440@60.00Hz"]}]'
printf '%s' "$MONITORES" >"$T/monitores.json"
cat >"$T/bin/hyprctl" <<FIN
#!/bin/sh
echo "hyprctl \$*" >>"$T/registro"
case "\$*" in *monitors*) cat "$T/monitores.json" ;; esac
FIN
cat >"$T/bin/powerprofilesctl" <<FIN
#!/bin/sh
echo "powerprofilesctl \$*" >>"$T/registro"
[ "\$1" = get ] && echo balanced
exit 0
FIN
cat >"$T/bin/brightnessctl" <<FIN
#!/bin/sh
echo "brightnessctl \$*" >>"$T/registro"
[ "\$1" = -m ] && echo "intel_backlight,backlight,900,90%,1000"
exit 0
FIN
printf '#!/bin/sh\necho "vortex-pantallas" >>"%s/registro"\n' "$T" >"$T/bin/vortex-pantallas"
chmod +x "$T/bin/"*
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_STATE_HOME="$T/home/.local/state" \
    HYPRLAND_INSTANCE_SIGNATURE=prueba PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}

echo "== vortex bateria"
: >"$T/registro"
vortex bateria on
grep -q "powerprofilesctl set power-saver" "$T/registro" && bien "energía en ahorro" || mal "perfil: $(cat "$T/registro")"
grep -q "animations = { enabled = false }" "$T/registro" && bien "sin animaciones" || mal "animaciones"
grep -q "brightnessctl -q set 60%" "$T/registro" && bien "brillo a 60 %" || mal "brillo: $(grep brightness "$T/registro")"
grep -q 'output = "eDP-1", mode = "1920x1080@60.01"' "$T/registro" && bien "la pantalla de la laptop a 60 Hz" || mal "Hz: $(grep monitor "$T/registro")"
grep -q 'output = "HDMI-A-1"' "$T/registro" && mal "tocó el monitor externo" || bien "el monitor externo no se toca"
[[ "$(vortex bateria estado)" == "ahorro (desenchufada)" ]] && bien "estado: ahorro" || mal "estado"

: >"$T/registro"
vortex bateria on
[[ ! -s "$T/registro" ]] && bien "un segundo «on» no hace nada (no pisa lo guardado)" || mal "segundo on: $(cat "$T/registro")"

: >"$T/registro"
vortex bateria off
grep -q "powerprofilesctl set balanced" "$T/registro" && bien "al enchufar vuelve el perfil de antes" || mal "perfil off: $(cat "$T/registro")"
grep -q "brightnessctl -q set 90%" "$T/registro" && bien "y el brillo de antes" || mal "brillo off"
grep -q "hyprctl reload" "$T/registro" && grep -q "vortex-pantallas" "$T/registro" && bien "y las animaciones y los Hz (recarga)" || mal "recarga"
[[ "$(vortex bateria estado)" == normal ]] && bien "estado: normal" || mal "estado tras off"
: >"$T/registro"; vortex bateria off
[[ ! -s "$T/registro" ]] && bien "«off» sin haber estado en ahorro no hace nada (PC de escritorio)" || mal "off de más"

printf '{"modoLaptop": false}\n' >"$T/home/.config/serpantinum/settings.json"
: >"$T/registro"; vortex bateria on
[[ ! -s "$T/registro" ]] && bien "apagado en Ajustes: no hace nada" || mal "con modoLaptop false hizo algo"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
