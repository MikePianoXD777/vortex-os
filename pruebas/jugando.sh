#!/usr/bin/env bash
# Prueba de «no interrumpir partidas» (4.0): «vortex jugando» y lo que se salta mientras hay una partida
# (los puntos automáticos de Restore, la salud del disco). Todo con hyprctl, notify-send y vortex falsos y
# con el estado en una carpeta temporal: no toca tu sesión.
#
#   pruebas/jugando.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-jugando.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/estado/vortex" "$T/home"
# hyprctl falso: la ventana activa sale de $T/ventana
cat >"$T/bin/hyprctl" <<F
#!/bin/sh
case "\$*" in
  *version*) echo '{"tag":"falso"}' ;;
  *activewindow*) cat "$T/ventana" ;;
esac
F
cat >"$T/bin/notify-send" <<F
#!/bin/sh
echo "\$*" >>"$T/aviso"
F
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" XDG_STATE_HOME="$T/estado" HOME="$T/home"
echo '{"fullscreen":0}' >"$T/ventana"
jugando() { bash "$AQUI/bin/vortex" jugando; }
MARCA="$T/estado/vortex/modo-juego"

echo "== vortex jugando"
probar "sin nada: no hay partida" "! jugando"
touch "$MARCA"
probar "con el modo juego puesto: sí" "jugando"
touch -d '3 days ago' "$MARCA"
probar "una marca de antes de este arranque no cuenta" "! jugando"
rm -f "$MARCA"
echo '{"fullscreen":2}' >"$T/ventana"
probar "algo en pantalla completa: sí" "jugando"
echo '{"fullscreen":false}' >"$T/ventana"
probar "ventana normal: no" "! jugando"
echo 'no es json' >"$T/ventana"
probar "si hyprctl dice algo raro: no molesta de más (no hay partida)" "! jugando"

echo "== lo que se salta durante la partida"
# «vortex» falso en el PATH: «jugando» sale con lo que diga $T/jugando
cat >"$T/bin/vortex" <<F
#!/bin/sh
[ "\$1" = jugando ] && [ -e "$T/jugando" ]
F
chmod +x "$T/bin/vortex"
printf '[{"x":1}]' >/dev/null
cat >"$T/bin/lsblk" <<'F'
#!/bin/sh
echo 'NAME="sda" TYPE="disk" TRAN="sata" MODEL="Disco de prueba"'
F
cat >"$T/bin/smartctl" <<'F'
#!/bin/sh
echo '{"smart_status":{"passed":false}}'
F
chmod +x "$T/bin/lsblk" "$T/bin/smartctl"
touch "$T/jugando"
rm -f "$T/aviso"
bash "$AQUI/bin/vortex-disco" salud
probar "salud del disco: con partida, no avisa ni lee el disco" "[[ ! -e '$T/aviso' && -z \"\$(ls '$T/estado/vortex')\" ]]"
rm -f "$T/jugando"
bash "$AQUI/bin/vortex-disco" salud
probar "salud del disco: sin partida, sí avisa" "grep -q 'está fallando' '$T/aviso'"
touch "$T/jugando"
bash "$AQUI/bin/vortex-restore" auto >/dev/null 2>&1
probar "Restore: el punto de cada 30 min espera a la partida (no escribe nada)" "[[ -z \"\$(find '$T/home' -type f 2>/dev/null)\" ]]"

echo "== conexiones"
probar "modo-juego avisa al shell para callar los avisos" "grep -q 'notificaciones jugando true' '$AQUI/bin/vortex' && grep -q 'notificaciones jugando false' '$AQUI/bin/vortex'"
probar "el shell tiene su receptor" "grep -q 'target: \"notificaciones\"' '$AQUI/shell/src/quickshell/notifications/NotificationPopups.qml'"
probar "actualizaciones revisar y actualizar lo consultan" "grep -c 'partida_en_curso' '$AQUI/bin/vortex' | grep -qE '^[4-9]'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
