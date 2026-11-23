#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «driver de NVIDIA actualizado sin reiniciar» (bin/vortex, 4.2): si el módulo cargado no es la
# versión de nvidia-utils instalada, «vortex reiniciar-shell» (y el final de «vortex actualizar») no
# reinicia el shell, porque el nuevo se dibujaría con la CPU (llvmpipe), y pide reiniciar la PC.
#
# En un HOME temporal, con hyprctl, pgrep, pacman y /proc/driver/nvidia/version falsos: no toca tu shell.
#
#   pruebas/nvidia-desfasado.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-nvidia.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

H="$T/home"
mkdir -p "$T/bin" "$H"
cat >"$T/bin/hyprctl" <<EOS
#!/bin/sh
echo "hyprctl \$*" >>"$T/registro"
case "\$*" in *version*) echo "Hyprland falso" ;; esac
exit 0
EOS
printf '#!/bin/sh\nexit 0\n' >"$T/bin/pgrep"
printf '#!/bin/sh\n[ "$2" = nvidia-utils ] && echo "nvidia-utils 620.73.04-1"\n' >"$T/bin/pacman"
chmod +x "$T/bin/"*

v() { HOME="$H" XDG_CONFIG_HOME="$H/.config" HYPRLAND_INSTANCE_SIGNATURE=prueba PATH="$T/bin:$PATH" \
  VORTEX_NVIDIA_PROC="$T/nvidia-version" bash "$AQUI/bin/vortex" "$@" 2>&1; }
modulo() { printf 'NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  %s  Release Build\n' "$1" >"$T/nvidia-version"; }

echo "== módulo viejo cargado (se actualizó el driver)"
modulo 620.70.06; : >"$T/registro"
salida="$(v reiniciar-shell)"
! grep -q 'eval' "$T/registro" && bien "no reinicia el shell" || mal "lo reinició: $(cat "$T/registro")"
grep -q 'reinicia la PC' <<<"$salida" && bien "pide reiniciar la PC" || mal "salida: $salida"
grep -q '620.70.06 → 620.73.04' <<<"$salida" && bien "y dice las versiones" || mal "salida: $salida"

echo "== mismo driver"
modulo 620.73.04; : >"$T/registro"
salida="$(v reiniciar-shell)"
grep -q 'eval' "$T/registro" && bien "reinicia el shell como siempre" || mal "no lo reinició: $(cat "$T/registro")"
! grep -q 'NVIDIA' <<<"$salida" && bien "sin aviso" || mal "salida: $salida"

echo "== sin NVIDIA"
rm -f "$T/nvidia-version"; : >"$T/registro"
v reiniciar-shell >/dev/null
grep -q 'eval' "$T/registro" && bien "reinicia el shell" || mal "no lo reinició"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
