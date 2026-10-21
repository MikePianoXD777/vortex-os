#!/usr/bin/env bash
# Mide la RAM del escritorio en la VM con un arranque en frío por variante (lo confiable:
# reiniciar solo el shell deja memoria acumulada y sesga los resultados ~20 MB por vuelta).
#
#   vm/medir-frio.sh NOMBRE ['COMANDO PARA PREPARAR LA VARIANTE (corre en la VM)']
#
# Imprime: NOMBRE  shell MB  sesión MB  usada MB (promedio de 3 lecturas, 2 min tras entrar).
# Usa VORTEX_VM para elegir la VM (por defecto, la de Vortex).
set -euo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export VORTEX_VM_USUARIO=vortex
nombre="$1" preparar="${2:-true}"
"$AQUI/vm.sh" ssh "$preparar; sudo reboot" >/dev/null 2>&1 || true
sleep 20
for _ in $(seq 60); do "$AQUI/vm.sh" ssh true 2>/dev/null && break; sleep 5; done
sleep 120
"$AQUI/vm.sh" ssh 'sh=0; se=0; us=0
for i in 1 2 3; do
  j=$(vortex-bench --json)
  sh=$(echo "$sh + $(echo "$j" | jq "[.ram_sesion_detalle | to_entries[] | select(.key == \"quickshell\" or .key == \"qs\") | .value] | add // 0")" | bc -l)
  se=$(echo "$se + $(echo "$j" | jq .ram_sesion_mb)" | bc -l)
  us=$(echo "$us + $(echo "$j" | jq .ram_usada_mb)" | bc -l)
  sleep 10
done
printf "%-26s shell %6.1f MB   sesión %6.1f MB   usada %6.0f MB\n" "'"$nombre"'" "$(echo "$sh/3" | bc -l)" "$(echo "$se/3" | bc -l)" "$(echo "$us/3" | bc -l)"'
