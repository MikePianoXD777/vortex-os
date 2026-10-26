#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex fps» (1.8, contador de FPS con MangoHud) en un HOME temporal, con un steam.desktop
# del sistema falso, flatpak, mangohud y pgrep falsos: no toca los lanzadores ni los Flatpak de verdad.
#
#   pruebas/fps.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-fps-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config" "$T/sistema/applications"
printf '[Desktop Entry]\nName=Steam\nExec=/usr/bin/steam %%U\n\n[Desktop Action Store]\nExec=/usr/bin/steam steam://store\n' \
  >"$T/sistema/applications/steam.desktop"
printf '[Desktop Entry]\nName=Heroic\nExec=heroic %%u\n' >"$T/sistema/applications/heroic.desktop"
cat >"$T/bin/flatpak" <<FIN
#!/bin/sh
echo "flatpak \$*" >>"$T/registro"
case "\$1" in
  list) printf 'org.mozilla.firefox\norg.vinegarhq.Sober\norg.prismlauncher.PrismLauncher\n' ;;
  info) exit 1 ;;
esac
exit 0
FIN
printf '#!/bin/sh\nexit 0\n' >"$T/bin/mangohud"
printf '#!/bin/sh\n[ -e "%s/steam-abierto" ]\n' "$T" >"$T/bin/pgrep"
printf '#!/bin/sh\necho "notify-send $*" >>"%s/registro"\n' "$T" >"$T/bin/notify-send"
chmod +x "$T/bin/"*
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_DATA_HOME="$T/home/.local/share" XDG_STATE_HOME="$T/home/.local/state" \
    XDG_DATA_DIRS="$T/sistema" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}
APPS="$T/home/.local/share/applications"
MANGO="$T/home/.config/MangoHud/MangoHud.conf"

echo "== vortex fps"
[[ "$(vortex fps estado)" == apagado ]] && bien "de fábrica: apagado" || mal "estado: $(vortex fps estado)"

: >"$T/registro"; touch "$T/steam-abierto"
vortex fps on
[[ "$(vortex fps estado)" == encendido ]] && bien "on: encendido" || mal "estado tras on"
[[ "$(grep -c '^Exec=env MANGOHUD=1 /usr/bin/steam' "$APPS/steam.desktop" 2>/dev/null)" == 2 ]] &&
  grep -q '^X-Vortex-Fps=true' "$APPS/steam.desktop" && bien "Steam arranca con MangoHud (también sus acciones)" || mal "steam.desktop: $(cat "$APPS/steam.desktop" 2>&1)"
grep -q '^Exec=env MANGOHUD=1 heroic' "$APPS/heroic.desktop" 2>/dev/null && bien "y Heroic" || mal "heroic.desktop"
[[ ! -e "$APPS/net.lutris.Lutris.desktop" ]] && bien "lo que no está instalado no se crea (Lutris)" || mal "creó Lutris"
grep -q "override --user --env=MANGOHUD=1 org.vinegarhq.Sober" "$T/registro" &&
  grep -q "override --user --env=MANGOHUD=1 org.prismlauncher.PrismLauncher" "$T/registro" &&
  bien "los juegos de Flatpak (Sober, Prism)" || mal "flatpak: $(grep override "$T/registro")"
grep -q "firefox" <<<"$(grep override "$T/registro")" && mal "tocó Firefox" || bien "las demás apps de Flatpak no"
grep -q "VulkanLayer.MangoHud" "$T/registro" && bien "avisa que falta la capa de MangoHud para Flatpak" || mal "sin aviso de la capa"
grep -q "vuelve a abrir Steam" "$T/registro" && bien "con Steam abierto, avisa que hay que reabrirlo" || mal "sin aviso de Steam"
grep -q "toggle_hud=Shift_R+F12" "$MANGO" 2>/dev/null && bien "pone la config de Vortex en MangoHud.conf" || mal "MangoHud.conf"

echo "# mía" >"$MANGO"
vortex fps on
[[ "$(cat "$MANGO")" == "# mía" ]] && bien "no pisa un MangoHud.conf que ya existe" || mal "pisó MangoHud.conf"

: >"$T/registro"
vortex fps off
[[ "$(vortex fps estado)" == apagado ]] && bien "off: apagado" || mal "estado tras off"
[[ ! -e "$APPS/steam.desktop" && ! -e "$APPS/heroic.desktop" ]] && bien "borra las copias de Vortex" || mal "quedaron los .desktop"
grep -q "override --user --unset-env=MANGOHUD org.vinegarhq.Sober" "$T/registro" && bien "quita MANGOHUD de los Flatpak" || mal "off flatpak: $(cat "$T/registro")"
[[ -e "$MANGO" ]] && bien "la config de MangoHud se queda (es de la persona)" || mal "borró MangoHud.conf"

printf '[Desktop Entry]\nExec=/usr/bin/steam -mío\n' >"$APPS/steam.desktop"
vortex fps on; vortex fps off
[[ "$(cat "$APPS/steam.desktop")" == *"-mío"* ]] && bien "un steam.desktop propio no se pisa ni se borra" || mal "tocó el steam.desktop de la persona"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
