#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de Vortexine Restore (bin/vortex-restore, 1.10) en un HOME temporal, con hyprctl, kitty y
# «vortex» falsos: no toca tu configuración ni tu escritorio.
#
#   pruebas/restore.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-restore-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

H="$T/home"
mkdir -p "$T/bin" "$H/.config/hypr" "$H/.config/serpantinum" "$H/.config/vortex/onedrive" \
  "$H/.config/vortex/respaldo-20261026-1" "$H/.local/state/serpantinum/widgets/DP-1" "$H/.config/kitty"
printf '#!/bin/sh\necho "hyprctl $*" >>"%s/registro"\n' "$T" >"$T/bin/hyprctl"
printf '#!/bin/sh\necho "vortex $*" >>"%s/registro"\n' "$T" >"$T/bin/vortex"
chmod +x "$T/bin/"*
echo 'gaps = 7' >"$H/.config/hypr/hyprland.lua"
echo '{"theme": "azul"}' >"$H/.config/serpantinum/settings.json"
echo '{"widgets": 1}' >"$H/.local/state/serpantinum/widgets/DP-1/layout.json"
echo 'font_size 13' >"$H/.config/kitty/kitty.conf"
echo 'hl.bind("SUPER + X")' >"$H/.config/vortex/hypr-mio.lua"
echo 'token = secreto' >"$H/.config/vortex/onedrive/rclone.conf"
echo 'viejo' >"$H/.config/vortex/respaldo-20261026-1/hyprland.lua"
r() { HOME="$H" XDG_DATA_HOME="$H/.local/share" HYPRLAND_INSTANCE_SIGNATURE=prueba PATH="$T/bin:$PATH" \
  bash "$AQUI/bin/vortex-restore" "$@"; }

echo "== crear y lista"
r crear "Todo bien" >/dev/null && bien "crea el primer punto" || mal "crear"
[[ "$(r lista --json | jq length)" == 1 ]] && bien "la lista tiene 1 punto" || mal "lista: $(r lista --json)"
guardados="$(git --git-dir="$H/.local/share/vortex/restore" ls-files)"
grep -q 'hypr/hyprland.lua' <<<"$guardados" && grep -q 'widgets/DP-1/layout.json' <<<"$guardados" &&
  grep -q 'hypr-mio.lua' <<<"$guardados" && bien "guarda Hyprland, widgets y hypr-mio.lua" || mal "guardados: $guardados"
! grep -q 'rclone.conf\|respaldo-' <<<"$guardados" && bien "no guarda claves (OneDrive) ni respaldos viejos" || mal "guardó de más: $guardados"

echo "== auto"
r auto && [[ "$(r lista --json | jq length)" == 1 ]] && bien "sin cambios, «auto» no crea punto" || mal "auto sin cambios"
echo 'gaps = 0  # se rompió' >"$H/.config/hypr/hyprland.lua"
echo 'bind = roto' >"$H/.config/hypr/nuevo.lua"
echo '{"theme": "rojo"}' >"$H/.config/serpantinum/settings.json"
r auto
[[ "$(r lista --json | jq -r '.[0].partes | join(",")')" == "hyprland,tema" ]] && bien "con cambios, punto con sus partes (hyprland, tema)" || mal "partes: $(r lista --json | jq -c '.[0]')"
[[ "$(r lista --json | jq -r '.[0].tipo')" == auto ]] && bien "marcado como automático" || mal "tipo"

echo "== volver solo una parte"
: >"$T/registro"
r volver 2 hyprland >/dev/null
[[ "$(cat "$H/.config/hypr/hyprland.lua")" == 'gaps = 7' ]] && bien "Hyprland volvió al punto «Todo bien»" || mal "hyprland.lua: $(cat "$H/.config/hypr/hyprland.lua")"
[[ ! -e "$H/.config/hypr/nuevo.lua" ]] && bien "quita el archivo que se agregó después" || mal "nuevo.lua sigue"
[[ "$(cat "$H/.config/serpantinum/settings.json")" == '{"theme": "rojo"}' ]] && bien "el tema (otra parte) no se tocó" || mal "settings.json cambió"
grep -q 'hyprctl reload' "$T/registro" && bien "recarga Hyprland" || mal "registro: $(cat "$T/registro")"
! grep -q 'reiniciar-shell' "$T/registro" && bien "no reinicia el shell si no hacía falta" || mal "reinició el shell"
[[ "$(r lista --json | jq -r '.[0].tipo')" == volver ]] && r lista --json | jq -e 'any(.[]; .tipo == "antes")' >/dev/null &&
  bien "queda el punto «Antes de volver» (se puede deshacer)" || mal "puntos: $(r lista --json | jq -c '[.[].tipo]')"

echo "== deshacer y volver todo"
antes="$(r lista --json | jq -r 'map(.tipo) | index("antes") + 1')"
r volver "$antes" hyprland >/dev/null
[[ "$(cat "$H/.config/hypr/hyprland.lua")" == 'gaps = 0  # se rompió' && -f "$H/.config/hypr/nuevo.lua" ]] &&
  bien "volver a «Antes de volver» lo deshace" || mal "no se deshizo"
: >"$T/registro"
primero="$(r lista --json | jq length)"
r volver "$primero" >/dev/null
[[ "$(cat "$H/.config/hypr/hyprland.lua")" == 'gaps = 7' && "$(cat "$H/.config/serpantinum/settings.json")" == '{"theme": "azul"}' ]] &&
  bien "sin partes, vuelve todo" || mal "no volvió todo"
grep -q 'reiniciar-shell' "$T/registro" && bien "y reinicia el shell (tema y widgets)" || mal "no reinició el shell"
[[ -f "$H/.config/vortex/onedrive/rclone.conf" && -d "$H/.config/vortex/respaldo-20261026-1" ]] &&
  bien "las claves y los respaldos siguen ahí" || mal "borró lo que no debía"

echo "== errores"
r volver 999 2>/dev/null && mal "aceptó un punto que no existe" || bien "un punto que no existe da error"
r volver 1 inventada 2>/dev/null && mal "aceptó una parte inventada" || bien "una parte inventada da error"

echo "== emergencia"
r auto inicio && r auto inicio
echo 'roto' >"$H/.config/hypr/hyprland.lua"
salida="$(printf '\n' | r emergencia 2>&1)"
grep -q 'Sugerido: 2' <<<"$salida" && bien "sugiere el inicio de sesión anterior" || mal "salida: $salida"
[[ "$(cat "$H/.config/hypr/hyprland.lua")" == 'gaps = 7' ]] && bien "con Enter vuelve a ese punto" || mal "hyprland.lua: $(cat "$H/.config/hypr/hyprland.lua")"
printf 'q\n' | r emergencia >/dev/null 2>&1 && bien "con q sale sin tocar nada" || mal "q"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
