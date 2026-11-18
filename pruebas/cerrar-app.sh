#!/usr/bin/env bash
# shellcheck disable=SC2015  # bien() y mal() nunca fallan
# «Cerrar por completo» (vortex-cerrar-app, 4.0): cierra la app entera y nada más.
# Solo usa procesos propios (sleep) en scopes propios: nunca toca apps, ventanas ni la sesión reales.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
CMD="$PWD/bin/vortex-cerrar-app"
export VORTEX_CERRAR_ESPERA=2
fallos=0
bien() { printf '  ok  %s\n' "$1"; }
mal() { printf '  MAL %s\n' "$1"; fallos=$((fallos + 1)); }
vivo() { [[ -d "/proc/$1" ]] && [[ "$(awk '{print $3}' "/proc/$1/stat" 2>/dev/null)" != Z ]]; }
esperar_pid() { for _ in $(seq 1 50); do [[ -s "$1" ]] && return 0; sleep 0.1; done; return 1; }
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"; systemctl --user stop vortex-prueba-cerrar-$$.scope >/dev/null 2>&1' EXIT

echo "Protegidos"
[[ "$("$CMD" --explicar --pid 1 2>&1)" == *"no cierro"* ]] && bien "no toca el pid 1" || mal "intentó con el pid 1"
"$CMD" --explicar --pid "$$" 2>&1 | grep -q 'no cierro' && bien "no se cierra a sí mismo ni a quien lo llama" || mal "se pudo cerrar a quien lo llamó"
"$CMD" --explicar --pid 999999999 2>&1 | grep -q 'no cierro' && bien "un proceso que no existe no hace nada" || mal "aceptó un pid inexistente"
"$CMD" 2>/dev/null; (($? == 1)) && bien "sin app, sale con error" || mal "sin app no dio error"

echo "App en su propio scope (como las mete el gestor)"
# Una «app» con un hijo que ignora SIGTERM (como un proceso de bandeja terco).
# (en un archivo: systemd-run cambiaría los «$$» de la línea de comandos)
cat >"$tmp/app.sh" <<EOF
echo \$\$ >"$tmp/app"; (trap "" TERM; exec sleep 300) & echo \$! >"$tmp/hijo"; sleep 300 & wait
EOF
systemd-run --user --scope -q --unit="vortex-prueba-cerrar-$$" -p TimeoutStopSec=5 -- bash "$tmp/app.sh" &
esperar_pid "$tmp/app" && esperar_pid "$tmp/hijo" || mal "no arrancó la app de prueba"
app="$(cat "$tmp/app")"; hijo="$(cat "$tmp/hijo")"
[[ "$("$CMD" --explicar --pid "$app")" == "unidad vortex-prueba-cerrar-$$.scope" ]] && bien "reconoce que el scope es solo de la app" || mal "no reconoció su scope: $("$CMD" --explicar --pid "$app")"
"$CMD" --pid "$app" --nombre "Prueba" && bien "cerró la app" || mal "falló al cerrar"
sleep 0.5
! vivo "$app" && bien "el proceso principal ya no está" || mal "el proceso principal sigue vivo"
! vivo "$hijo" && bien "el hijo terco (ignora SIGTERM) también se cerró" || mal "el hijo terco sigue vivo"
! systemctl --user is-active -q "vortex-prueba-cerrar-$$.scope" && bien "el scope se paró" || mal "el scope sigue activo"

echo "App sin scope propio (abierta desde una terminal): solo su árbol"
bash -c 'echo $$ >"'"$tmp"'/app2"; (trap "" TERM; exec sleep 300) & echo $! >"'"$tmp"'/hijo2"; sleep 300 & wait' &
sleep 300 & vecino=$!
esperar_pid "$tmp/app2" && esperar_pid "$tmp/hijo2" || mal "no arrancó la segunda app"
app2="$(cat "$tmp/app2")"; hijo2="$(cat "$tmp/hijo2")"
[[ "$("$CMD" --explicar --pid "$app2")" == arbol* ]] && bien "en un scope compartido cierra solo el árbol" || mal "quiso parar el scope compartido: $("$CMD" --explicar --pid "$app2")"
"$CMD" --pid "$app2" && bien "cerró la segunda app" || mal "falló con la segunda app"
sleep 0.3
! vivo "$app2" && ! vivo "$hijo2" && bien "la app y su hijo terco se cerraron" || mal "quedó algo de la segunda app"
vivo "$vecino" && bien "el proceso vecino (fuera del árbol) sigue vivo" || mal "cerró un proceso que no era de la app"
kill "$vecino" 2>/dev/null

echo "Gestor: los scopes nuevos esperan 5 s, no 90"
grep -q 'TimeoutStopUSec t 5000000' bin/vortex-gestor && grep -q '"$unidad" fail 7' bin/vortex-gestor \
  && bien "scope_systemd pide TimeoutStopSec=5s (7 propiedades)" || mal "scope_systemd sin TimeoutStopSec o con la cuenta mal"
grep -q 'cerrar-app' shell/src/quickshell/popouts/DockAppMenu.qml && bien "el menú del dock tiene «Cerrar por completo»" || mal "falta el botón en el dock"
grep -q 'vortex-cerrar-app' install/30-escritorio.sh && bien "el instalador copia el comando" || mal "el instalador no copia el comando"

((fallos == 0)) && echo "Todo bien" || { echo "$fallos fallos"; exit 1; }
