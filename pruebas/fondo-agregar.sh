#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex fondo-agregar» (1.10, botón «Agregar» del selector de fondos) en un HOME temporal,
# con hyprctl, quickshell y notify-send falsos: no toca el fondo ni el shell de verdad.
#
#   pruebas/fondo-agregar.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-fondo-agregar.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config/serpantinum" "$T/fondos" "$T/descargas"
printf '#!/bin/sh\necho %s\n' "'[{\"name\":\"DP-1\"},{\"name\":\"HDMI-A-1\"}]'" >"$T/bin/hyprctl"
printf '#!/bin/sh\necho "$*" >>"%s/registro"\n' "$T" >"$T/bin/quickshell"
printf '#!/bin/sh\necho "$*" >>"%s/avisos"\n' "$T" >"$T/bin/notify-send"
chmod +x "$T/bin/"*
printf '{"wallpaperDir": "%s"}\n' "$T/fondos" >"$T/home/.config/serpantinum/settings.json"
echo uno >"$T/descargas/Mi video.MOV"
echo dos >"$T/descargas/clip.avi"
echo tres >"$T/descargas/foto.webp"
touch "$T/descargas/nota.txt"
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_CACHE_HOME="$T/home/.cache" XDG_STATE_HOME="$T/home/.local/state" \
    HYPRLAND_INSTANCE_SIGNATURE=prueba VORTEX_SHELL_QML="$T/Shell.qml" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}

echo "== vortex fondo-agregar"
: >"$T/registro"
vortex fondo-agregar "$T/descargas/Mi video.MOV" "$T/descargas/clip.avi" 2>/dev/null
[[ -f "$T/fondos/Mi video.MOV" && -f "$T/fondos/clip.avi" ]] && bien "copia un .MOV y un .avi a la carpeta de fondos" || mal "fondos: $(ls "$T/fondos")"
[[ -f "$T/descargas/Mi video.MOV" ]] && bien "deja el original donde estaba" || mal "movió el original"
[[ "$(grep -c "setWallpaper.*$T/fondos/Mi video.MOV" "$T/registro")" == 2 ]] && bien "pone el primero en los dos monitores" || mal "registro: $(cat "$T/registro")"
grep -q "Fondo agregado" "$T/avisos" 2>/dev/null && bien "avisa con una notificación" || mal "sin aviso"

echo cambiado >"$T/descargas/Mi video.MOV"
vortex fondo-agregar "$T/descargas/Mi video.MOV" 2>/dev/null
[[ "$(cat "$T/fondos/Mi video.MOV")" == uno && "$(cat "$T/fondos/Mi video (2).MOV")" == cambiado ]] &&
  bien "no pisa uno que ya está: «Mi video (2).MOV»" || mal "fondos: $(ls "$T/fondos")"

: >"$T/registro"
vortex fondo-agregar "$T/fondos/clip.avi" 2>/dev/null
[[ "$(find "$T/fondos" -name 'clip*' | wc -l)" == 1 ]] && grep -q "clip.avi" "$T/registro" &&
  bien "uno que ya está en la carpeta solo se pone, no se copia otra vez" || mal "fondos: $(ls "$T/fondos")"

: >"$T/registro"
vortex fondo-agregar "$T/descargas/nota.txt" 2>/dev/null && mal "aceptó un .txt" || bien "un .txt no es fondo: sale con error"
[[ ! -e "$T/fondos/nota.txt" && ! -s "$T/registro" ]] && bien "y no copia ni cambia nada" || mal "copió o cambió el fondo"

vortex fondo-agregar "$T/descargas/foto.webp" 2>/dev/null
[[ -f "$T/fondos/foto.webp" ]] && bien "también imágenes" || mal "no copió la imagen"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
