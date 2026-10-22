#!/usr/bin/env bash
# Mide la CPU en reposo de la VM tras un arranque en frío: procesos creados, tiempo de CPU
# (todo el sistema) y cambios de contexto en 60 s sin tocar nada, 2,5 min después de entrar.
#
#   vm/medir-reposo.sh NOMBRE
#
# Usa VORTEX_VM y VORTEX_VM_PUERTO para elegir la VM (por defecto, la de Vortex).
set -euo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export VORTEX_VM_USUARIO=vortex
nombre="$1"
"$AQUI/vm.sh" ssh "sudo reboot" >/dev/null 2>&1 || true
sleep 20
for _ in $(seq 60); do "$AQUI/vm.sh" ssh true 2>/dev/null && break; sleep 5; done
sleep 150
"$AQUI/vm.sh" ssh 'leer() { awk "/^processes/{p=\$2} /^ctxt/{c=\$2} /^cpu /{t=\$2+\$3+\$4+\$7+\$8} END{print p, c, t}" /proc/stat; }
read -r p1 c1 t1 < <(leer); sleep 60; read -r p2 c2 t2 < <(leer)
hz=$(getconf CLK_TCK)
printf "%-26s procesos/min %6d   cambios de contexto/s %7d   CPU ocupada %5.2f %% de un núcleo\n" "'"$nombre"'" \
  $((p2 - p1 - 3)) $(((c2 - c1) / 60)) "$(echo "($t2 - $t1) * 100 / $hz / 60" | bc -l)"'
