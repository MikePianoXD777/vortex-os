#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «el bloqueo sobrevive a que el shell se caiga o se reinicie» (4.2).
#  - serpantinumd (lanzar_quickshell): si quickshell muere y queda la marca «locked», lo vuelve a abrir,
#    hasta 5 veces en 2 minutos; sin la marca, no.
#  - Lock.qml: al arrancar ya no borra la marca; si existe, vuelve a bloquear.
#
# Sin pantalla ni quickshell de verdad: la función se saca del daemon y corre con setsid y quickshell
# falsos en una carpeta temporal. No toca tu shell ni el /tmp/serpantinumd.* del daemon que ya corre.
#
#   pruebas/bloqueo-reinicio.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-bloqueo.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

sed -n '/^lanzar_quickshell() {/,/^}/p' "$AQUI/shell/bin/serpantinumd" >"$T/funcion.sh"
grep -q 'lanzar_quickshell' "$T/funcion.sh" || { echo "  MAL no encontré lanzar_quickshell en serpantinumd"; exit 1; }

mkdir -p "$T/bin" "$T/run/lock"
printf '#!/bin/sh\nexec "$@"\n' >"$T/bin/setsid"
# quickshell falso: cuenta cuántas veces arrancó; si existe $T/quitar-en-N, quita la marca en esa vuelta.
cat >"$T/bin/quickshell" <<EOS
#!/bin/sh
n=\$(( \$(cat "$T/veces" 2>/dev/null || echo 0) + 1 ))
echo \$n >"$T/veces"
[ -f "$T/quitar-en-\$n" ] && rm -f "$T/run/lock/locked"
exit 139
EOS
chmod +x "$T/bin/"*

correr() {
  rm -f "$T/veces"
  QS_RUN_DIR="$T/run" MAIN_QML=/no/existe PATH="$T/bin:$PATH" SERPANTINUM_RESPAWN_PAUSA=0 \
    timeout 20 bash -c "source '$T/funcion.sh'; lanzar_quickshell" 2>"$T/err"
  echo $? >"$T/rc"
}

echo "== se cierra sin estar bloqueado"
rm -f "$T/run/lock/locked"; correr
[[ "$(cat "$T/veces")" == 1 ]] && bien "no lo vuelve a abrir" || mal "arrancó $(cat "$T/veces") veces"

echo "== se cae con la pantalla bloqueada y al volver se desbloquea"
touch "$T/run/lock/locked" "$T/quitar-en-2"; correr; rm -f "$T/quitar-en-2"
[[ "$(cat "$T/veces")" == 2 ]] && bien "lo abre otra vez y se queda" || mal "arrancó $(cat "$T/veces") veces"
grep -q 'pantalla bloqueada' "$T/err" && bien "lo dice en el log" || mal "log: $(cat "$T/err")"

echo "== truena siempre al arrancar con la pantalla bloqueada"
touch "$T/run/lock/locked"; correr
[[ "$(cat "$T/veces")" == 6 ]] && bien "se rinde después de 5 intentos" || mal "arrancó $(cat "$T/veces") veces"
[[ "$(cat "$T/rc")" == 1 ]] && bien "y sale con error" || mal "rc=$(cat "$T/rc")"

echo "== Lock.qml"
L="$AQUI/shell/src/quickshell/lock/Lock.qml"
inicio="$(sed -n '/Component.onCompleted: {/,/^    }/p' "$L")"
! grep -q '"rm"' <<<"$inicio" && bien "al arrancar ya no borra la marca" || mal "onCompleted todavía la borra"
grep -q 'relockProbe.running = true' <<<"$inicio" && bien "al arrancar busca la marca" || mal "no la busca"
! grep -q 'Component.onDestruction' "$L" && bien "al cerrarse no borra la marca" || mal "onDestruction sigue"
grep -A4 'id: relockProbe' "$L" | grep -q '"test", "-f"' && bien "revisa el archivo «locked»" || mal "relockProbe raro"
grep -A6 'id: relockTimer' "$L" | grep -q 'root.lockStatic()' && bien "y bloquea (sin captura)" || mal "relockTimer no bloquea"
FMT="$(command -v qmlformat6 || command -v qmlformat || echo /usr/lib/qt6/bin/qmlformat)"
if [[ -x "$FMT" ]]; then
  "$FMT" "$L" >/dev/null 2>"$T/fmt" && bien "Lock.qml se lee sin errores de sintaxis" || mal "sintaxis: $(head -3 "$T/fmt")"
fi

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
