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
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" HOME="$T/casa de prueba" VORTEX_REPORTE_DIR="$T/salida"

msg="$("$AQUI/bin/vortex-reporte")"
R="$(ls "$T"/salida/*.txt 2>/dev/null | head -n1)"
probar "crea un archivo y dice dónde" "[[ -s '$R' && \$msg == *'Reporte listo'* ]]"
for s in Sistema Memoria Disco "Algo no funciona" "Errores de este arranque"; do
  probar "trae la sección «$s»" "grep -q '===== $s =====' '$R'"
done
probar "sin el usuario" "! grep -qw '$USUARIO' '$R'"
probar "sin el nombre del equipo" "! grep -qw '$EQUIPO' '$R'"
probar "sin la carpeta personal, pero se entiende la ruta" "! grep -q 'casa de prueba' '$R' && grep -q '~/secreto.txt' '$R'"
probar "sin IP" "! grep -q '192.168.1.77' '$R' && grep -q '\[ip\]' '$R'"
probar "sin IPv6" "! grep -q 'fe80::1234' '$R'"
probar "sin MAC" "! grep -q 'aa:bb:cc' '$R'"
probar "sin correos" "! grep -q 'alguien@' '$R'"
probar "sin el nombre de tu wifi" "! grep -q 'WiFi de mi casa' '$R'"
probar "las horas siguen enteras" "grep -q '12:30:45' '$R'"

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
