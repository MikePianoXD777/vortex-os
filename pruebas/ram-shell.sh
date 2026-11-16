#!/usr/bin/env bash
# shellcheck disable=SC2015  # bien() y mal() nunca fallan
# La memoria del shell no sube sola (3.5.2).
#
# Con NVIDIA, el driver perdía ~1 KB por cada cuadro que el shell presentaba con «explicit sync»
# (fondo animado a 30 cuadros/s, el visualizador con música): 2-10 MB por minuto que nunca volvían.
# El arreglo es __NV_DISABLE_EXPLICIT_SYNC=1 solo para el shell (config/hypr/hyprland.lua).
#
#   pruebas/ram-shell.sh            revisa el código y, si hay un shell corriendo, que tenga el arreglo
#   pruebas/ram-shell.sh medir [MIN] además mide el shell que corre (solo lee /proc, no toca nada) y
#                                    falla si sube más de VORTEX_RAM_TOPE KB por minuto (300 por defecto)
#
# Solo lee: no abre ventanas, no arranca quickshell ni Hyprland, no reinicia nada.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

fallos=0
bien() { printf '  ok  %s\n' "$1"; }
mal() { printf '  MAL %s\n' "$1"; fallos=$((fallos + 1)); }

echo "Código"
lua=config/hypr/hyprland.lua
bloque="$(sed -n '/^local function comando_shell/,/^end/p' "$lua")"
grep -q '/proc/driver/nvidia/version' <<<"$bloque" && bien "comando_shell detecta la NVIDIA" || mal "comando_shell no detecta la NVIDIA"
grep -q '__NV_DISABLE_EXPLICIT_SYNC=1' <<<"$bloque" && bien "el shell arranca sin explicit sync en NVIDIA" || mal "falta __NV_DISABLE_EXPLICIT_SYNC=1 en comando_shell"
# Solo el shell: que no quede en el entorno de toda la sesión (los juegos lo heredarían).
if grep -rqs '__NV_DISABLE_EXPLICIT_SYNC' config/hypr/config/env.lua config/hypr/env.lua 2>/dev/null; then
  mal "__NV_DISABLE_EXPLICIT_SYNC está en el entorno de toda la sesión"
else
  bien "no se pone para toda la sesión"
fi
grep -q 'for f in hyprland.lua' bin/vortex && bien "vortex actualizar copia hyprland.lua" || mal "vortex actualizar no copia hyprland.lua"

pid="$(pgrep -u "$(id -u)" -xo quickshell || true)"
if [[ -n "$pid" ]]; then
  echo "Shell corriendo (pid $pid)"
  if [[ -r /proc/driver/nvidia/version ]]; then
    if tr '\0' '\n' <"/proc/$pid/environ" | grep -qx '__NV_DISABLE_EXPLICIT_SYNC=1'; then
      bien "tiene el arreglo"
    else
      mal "es NVIDIA y el shell corre sin el arreglo (falta «vortex actualizar» o reiniciar el shell)"
    fi
  else
    bien "sin NVIDIA: no hace falta"
  fi

  if [[ "${1:-}" == medir ]]; then
    min="${2:-10}"
    tope="${VORTEX_RAM_TOPE:-300}"
    anon() { awk '/^RssAnon/{print $2}' "/proc/$pid/status"; }
    # El primer minuto no cuenta: lo que se acaba de abrir todavía se está acomodando.
    sleep 60
    a="$(anon)"
    printf '  midiendo %s min desde %s MB…\n' "$min" "$((a / 1024))"
    for ((i = 1; i <= min; i++)); do
      sleep 60
      printf '    min %2d: %s MB\n' "$i" "$(($(anon) / 1024))"
    done
    b="$(anon)"
    ritmo=$(((b - a) / min))
    if ((ritmo <= tope)); then
      bien "sube $ritmo KB/min (tope $tope)"
    else
      mal "sube $ritmo KB/min (tope $tope): la memoria se sigue juntando"
    fi
  fi
else
  echo "(no hay shell corriendo: solo se revisó el código)"
fi

((fallos == 0)) && echo "Todo bien" || { echo "$fallos fallos"; exit 1; }
