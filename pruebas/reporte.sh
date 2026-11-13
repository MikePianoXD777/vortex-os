#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2034
# Prueba de «Crear reporte» (2.7, bin/vortex-reporte): corre de verdad, con journalctl, ps y hyprctl falsos
# que sueltan datos personales, y revisa que no lleguen al archivo. No toca tu portapapeles.
#
#   pruebas/reporte.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-reporte.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin" "$T/casa de prueba/.local/share/Steam/logs" "$T/salida"
USUARIO="$(id -un)"
EQUIPO="$(cat /proc/sys/kernel/hostname)"
cat >"$T/bin/journalctl" <<FIN
#!/bin/sh
echo "error de $USUARIO en $EQUIPO, abriendo $T/casa de prueba/secreto.txt"
echo "ip 192.168.1.77 mac aa:bb:cc:dd:ee:ff correo alguien@ejemplo.com v6 fe80::1234:5678:9abc:def0"
echo "normal y tres horas: 12:30:45"
FIN
cat >"$T/bin/vortex-arreglar" <<'FIN'
#!/bin/sh
echo "✓ Internet: Conectado a «WiFi de mi casa»."
FIN
printf '#!/bin/sh\nexit 0\n' >"$T/bin/wl-copy"
printf '#!/bin/sh\nsleep 60\n' >"$T/bin/hyprctl.lento"
# hyprctl y Hyprland falsos: así no lee tu sesión real. El título de la ventana no debe llegar al reporte.
cat >"$T/bin/hyprctl" <<'FIN'
#!/bin/sh
case "$1" in
  clients) echo '[{"class":"firefox","title":"Mi banco secreto","workspace":{"name":"1"},"floating":true,"pinned":false,"fullscreen":0,"size":[800,600]}]' ;;
  plugin) echo "Plugin hyprbars by Vaxry" ;;
esac
FIN
printf '#!/bin/sh\necho "Hyprland 0.57.2 built from branch main at commit abc123 (x)."\n' >"$T/bin/Hyprland"
chmod +x "$T/bin/"*
# Sin XDG_*: si no, el reporte lee tu caché y tu config reales (los choques de tu Hyprland, tu modo de ventanas).
unset XDG_CACHE_HOME XDG_CONFIG_HOME XDG_STATE_HOME
export PATH="$T/bin:$PATH" HOME="$T/casa de prueba" VORTEX_REPORTE_DIR="$T/salida"

mkdir -p "$T/casa de prueba/.cache/hyprland"
echo "Hyprland received signal 11 (SIGSEGV)" >"$T/casa de prueba/.cache/hyprland/hyprlandCrashReport1234.txt"
msg="$("$AQUI/bin/vortex-reporte")"
R="$(ls "$T"/salida/*.txt 2>/dev/null | head -n1)"
probar "crea un archivo y dice dónde" "[[ -s '$R' && \$msg == *'Reporte listo'* ]]"
for s in Sistema Memoria Disco "Algo no funciona" "Errores de este arranque" Ventanas "Últimos arranques" \
  "Si se trabó antes de reiniciar (memoria, video, kernel)" "Choque de Hyprland" "Ventanas abiertas" \
  "Swap y presión de memoria" "Si se trabó antes de reiniciar (sesión)"; do
  probar "trae la sección «$s»" "grep -q '===== $s =====' '$R'"
done
probar "dice el modo de ventanas" "grep -q '^Modo: mosaico' '$R'"
probar "trae el reporte de choque de Hyprland" "grep -q 'hyprlandCrashReport1234.txt' '$R' && grep -q 'received signal 11' '$R'"
probar "dice cómo está cada ventana" "grep -q '^firefox  escritorio=1  flotante=true  fija=false  completa=0  800x600' '$R'"
probar "sin el título de las ventanas" "! grep -q 'banco secreto' '$R'"
probar "en mosaico no habla de la barra" "! grep -q 'Barra de título' '$R'"
probar "sin el usuario" "! grep -qw '$USUARIO' '$R'"
probar "sin el nombre del equipo" "! grep -qw '$EQUIPO' '$R'"
probar "sin la carpeta personal, pero se entiende la ruta" "! grep -q 'casa de prueba' '$R' && grep -q '~/secreto.txt' '$R'"
probar "sin IP" "! grep -q '192.168.1.77' '$R' && grep -q '\[ip\]' '$R'"
probar "sin IPv6" "! grep -q 'fe80::1234' '$R'"
probar "sin MAC" "! grep -q 'aa:bb:cc' '$R'"
probar "sin correos" "! grep -q 'alguien@' '$R'"
probar "sin el nombre de tu wifi" "! grep -q 'WiFi de mi casa' '$R'"
probar "las horas siguen enteras" "grep -q '12:30:45' '$R'"

# Modo «kde»: dice si falta la barra de título del Hyprland que tiene instalado, y si ya está.
mkdir -p "$HOME/.config/vortex" "$T/kde1" "$T/kde2"
echo kde >"$HOME/.config/vortex/ventanas"
VORTEX_REPORTE_DIR="$T/kde1" "$AQUI/bin/vortex-reporte" >/dev/null
probar "en kde avisa si falta la barra" "grep -q 'Barra de título: FALTA hyprbars-0.57.2.so' '$T'/kde1/*.txt"
mkdir -p "$HOME/.local/share/vortex/hyprbars"
echo so >"$HOME/.local/share/vortex/hyprbars/hyprbars-0.57.2.so"
echo "error: no encuentro hyprland.pc" >"$HOME/.local/share/vortex/hyprbars/compilar.log"
VORTEX_REPORTE_DIR="$T/kde2" "$AQUI/bin/vortex-reporte" >/dev/null
probar "en kde dice si la barra está y trae su compilación" \
  "grep -q 'Barra de título: hyprbars-0.57.2.so está' '$T'/kde2/*.txt && grep -q 'no encuentro hyprland.pc' '$T'/kde2/*.txt"
rm "$HOME/.config/vortex/ventanas"

cat >"$T/bin/wl-copy" <<FIN
#!/bin/sh
cat >"$T/portapapeles"
echo "\$*" >"$T/tipo"
FIN
sleep 1
"$AQUI/bin/vortex-reporte" >/dev/null
probar "copia el archivo como archivo" "grep -q 'text/uri-list' '$T/tipo' && grep -q '^file://.*Reporte%20Vortexine' '$T/portapapeles'"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
