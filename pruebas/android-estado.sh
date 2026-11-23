#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex-android estado» sin prender Android (4.3): «waydroid status» prende el contenedor por
# D-Bus, y la Shop pregunta el estado cada vez que abre la sección de Android, así que el contenedor se
# quedaba prendido sin usarse. Con el contenedor apagado, ya no se le pregunta a Waydroid.
#
# Con waydroid y systemctl falsos: no toca tu Waydroid.
#
#   pruebas/android-estado.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-android-estado.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home"
cat >"$T/bin/waydroid" <<EOS
#!/bin/sh
echo "waydroid \$*" >>"$T/waydroid.log"
[ "\$1" = status ] && printf 'Session:\tRUNNING\nContainer:\tRUNNING\n'
exit 0
EOS
cat >"$T/bin/systemctl" <<EOS
#!/bin/sh
[ -e "$T/activo" ]
EOS
chmod +x "$T/bin/"*
va() { HOME="$T/home" XDG_STATE_HOME="$T/home/estado" XDG_DATA_HOME="$T/home/datos" \
  VORTEX_ANDROID_WAYDROID="$T/bin/waydroid" VORTEX_ANDROID_SYSTEMCTL="$T/bin/systemctl" \
  bash "$AQUI/bin/vortex-android" "$@" 2>&1; }

echo "== contenedor apagado"
: >"$T/waydroid.log"
salida="$(va estado)"
grep -q '"corriendo": false' <<<"$salida" && bien "estado: no corre" || mal "estado: $salida"
salida="$(va apps)"
[[ "$salida" == "[]" ]] && bien "apps: lista del lanzador, vacía" || mal "apps: $salida"
[[ ! -s "$T/waydroid.log" ]] && bien "no le pregunta a Waydroid (no lo prende)" || mal "llamó: $(cat "$T/waydroid.log")"

echo "== contenedor prendido"
: >"$T/activo"
salida="$(va estado)"
grep -q '"corriendo": true' <<<"$salida" && bien "estado: corre" || mal "estado: $salida"
grep -q '^waydroid status' "$T/waydroid.log" && bien "ahí sí le pregunta" || mal "no preguntó"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS"
  exit 1
fi
echo "Todo bien"
