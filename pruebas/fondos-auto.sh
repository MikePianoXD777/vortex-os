#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex fondos-auto» (1.8, fondos que cambian solos) en un HOME temporal, con hyprctl y
# quickshell falsos: no toca el fondo ni el shell de verdad.
#
#   pruebas/fondos-auto.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-fondos-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config/serpantinum" "$T/home/.config/vortex" "$T/home/.cache/serpantinum/wallpaper" \
  "$T/home/.local/state/vortex" "$T/fondos"
printf '#!/bin/sh\necho %s\n' "'[{\"name\":\"DP-1\"},{\"name\":\"HDMI-A-1\"}]'" >"$T/bin/hyprctl"
printf '#!/bin/sh\necho "$*" >>"%s/registro"\n' "$T" >"$T/bin/quickshell"
chmod +x "$T/bin/hyprctl" "$T/bin/quickshell"
touch "$T/fondos/"{a.jpg,b.png,c.webp,video.mp4,nota.txt}
ajustes() { printf '{"wallpaperDir": "%s", "autoWallpaper": {"minutes": %s}}\n' "$T/fondos" "$1" >"$T/home/.config/serpantinum/settings.json"; }
actual() { printf '%s' "$1" >"$T/home/.cache/serpantinum/wallpaper/current_DP-1"; }
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_CACHE_HOME="$T/home/.cache" XDG_STATE_HOME="$T/home/.local/state" \
    HYPRLAND_INSTANCE_SIGNATURE=prueba VORTEX_SHELL_QML="$T/Shell.qml" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}

echo "== vortex fondos-auto"
ajustes 0
[[ "$(vortex fondos-auto estado)" == apagado ]] && bien "de fábrica: apagado" || mal "estado: $(vortex fondos-auto estado)"
ajustes 30
[[ "$(vortex fondos-auto estado)" == "cada 30 min" ]] && bien "lee los minutos de Ajustes" || mal "estado con 30"

: >"$T/registro"; actual "$T/fondos/a.jpg"
vortex fondos-auto ya
[[ "$(grep -c setWallpaper "$T/registro")" == 2 ]] && bien "pone el fondo en los dos monitores" || mal "registro: $(cat "$T/registro")"
elegido="$(grep -o "$T/fondos/[^ ]*" "$T/registro" | head -n1)"
[[ "$(grep -o "$T/fondos/[^ ]*" "$T/registro" | sort -u | wc -l)" == 1 ]] && bien "el mismo en los dos" || mal "distintos"
[[ "$elegido" =~ /(b\.png|c\.webp)$ ]] && bien "una imagen distinta de la de ahora ($(basename "$elegido"))" || mal "eligió $elegido"

ok_todas=1
for _ in 1 2 3 4 5 6 7 8; do
  : >"$T/registro"; vortex fondos-auto ya
  grep -qE "video\.mp4|nota\.txt|/a\.jpg" "$T/registro" && ok_todas=0
done
((ok_todas)) && bien "nunca videos, otros archivos ni el mismo fondo" || mal "eligió algo que no debía"

: >"$T/registro"; actual "$T/fondos/video.mp4"
vortex fondos-auto ya
[[ ! -s "$T/registro" ]] && bien "con un fondo animado puesto no lo cambia" || mal "cambió un video"

actual "$T/fondos/a.jpg"; touch "$T/home/.local/state/vortex/modo-juego"; : >"$T/registro"
vortex fondos-auto ya
[[ ! -s "$T/registro" ]] && bien "con el modo juego no cambia" || mal "cambió en modo juego"
rm "$T/home/.local/state/vortex/modo-juego"; touch "$T/home/.config/vortex/otaku"; : >"$T/registro"
vortex fondos-auto ya
[[ ! -s "$T/registro" ]] && bien "con el modo otaku no cambia" || mal "cambió en modo otaku"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
