#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «Hyprland en modo de emergencia» (bin/vortex, 4.1): al instalar vortex-shell, Hyprland puede
# recargar justo cuando pacman quitó config/env.lua y quedarse sin atajos. «vortex reiniciar-shell» (y el final
# de «vortex actualizar», que lo llama) recarga Hyprland solo si tiene errores de configuración.
#
# En un HOME temporal, con hyprctl, pgrep y serpantinum falsos: no toca tu Hyprland ni tu shell.
#
#   pruebas/hypr-emergencia.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-hypr-emergencia.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

H="$T/home"
mkdir -p "$T/bin" "$H"
# hyprctl falso: anota todo; «configerrors» contesta lo que haya en $T/errores.
cat >"$T/bin/hyprctl" <<EOF
#!/bin/sh
echo "hyprctl \$*" >>"$T/registro"
case "\$*" in
  *configerrors*) cat "$T/errores" 2>/dev/null ;;
  *version*) echo "Hyprland falso" ;;
esac
exit 0
EOF
# Sin quickshell corriendo: reiniciar_shell no relanza nada (solo se prueba la recarga).
printf '#!/bin/sh\nexit 1\n' >"$T/bin/pgrep"
chmod +x "$T/bin/"*

v() { HOME="$H" XDG_CONFIG_HOME="$H/.config" HYPRLAND_INSTANCE_SIGNATURE=prueba PATH="$T/bin:$PATH" \
  bash "$AQUI/bin/vortex" "$@" 2>&1; }

echo "== sin errores"
: >"$T/registro"
printf '\n' >"$T/errores"
salida="$(v reiniciar-shell)"
grep -q 'configerrors' "$T/registro" && bien "revisa los errores" || mal "no revisó: $(cat "$T/registro")"
! grep -q 'hyprctl reload' "$T/registro" && bien "sin errores no recarga (no le quita el modo juego)" || mal "recargó sin errores"
[[ -z "$salida" ]] && bien "y no dice nada" || mal "dijo: $salida"

echo "== en modo de emergencia"
: >"$T/registro"
cat >"$T/errores" <<'EOF'
/home/mike/.config/hypr/hyprland.lua:99: module 'config/env' not found:
	no field package.preload['config/env']
EOF
salida="$(v reiniciar-shell)"
grep -q 'hyprctl reload' "$T/registro" && bien "recarga Hyprland" || mal "no recargó: $(cat "$T/registro")"
grep -q 'recargué Hyprland' <<<"$salida" && bien "y lo dice" || mal "salida: $salida"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
