#!/usr/bin/env bash
# shellcheck disable=SC2015,SC2034
# Prueba de «Novedades» (2.7, cmd_novedades de bin/vortex): una copia de bin/vortex en una carpeta con notas
# de mentira, hyprctl falso (anota si se abrió el panel) y estado en un HOME temporal.
#
#   pruebas/novedades.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-novedades.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

R="$T/repo"
mkdir -p "$R/bin" "$R/docs" "$R/install" "$T/fbin" "$T/home/.config" "$T/home/.local/state/vortex"
cp "$AQUI/bin/vortex" "$R/bin/vortex"
touch "$R/install.sh" # sin él, bin/vortex usaría el repo instalado
nota() { printf '# Versión %s\n\nLo nuevo de la %s.\n\n## Lo nuevo\n\n- Algo.\n\n## Para actualizar\n\n```\nvortex actualizar\n```\n' "$1" "$1" >"$R/docs/notas-$1.md"; }
for v in 2.5.0 2.6.0 2.6.3 2.7.0 2.10.0; do nota "$v"; done
version() { echo "VORTEX_VERSION=\"$1\"" >"$R/install/lib.sh"; }

cat >"$T/fbin/hyprctl" <<FIN
#!/bin/sh
case "\$*" in
  *activewindow*) echo "{\"fullscreen\": \${FALSO_PANTALLA:-0}}" ;;
  *dispatch*) echo "\$*" >>"$T/abierto" ;;
esac
FIN
chmod +x "$T/fbin/hyprctl"
export PATH="$T/fbin:$PATH" HOME="$T/home" XDG_STATE_HOME="$T/home/.local/state" XDG_CONFIG_HOME="$T/home/.config"
export VORTEX_NOVEDADES_ESPERA=0 HYPRLAND_INSTANCE_SIGNATURE=prueba
E="$XDG_STATE_HOME/vortex"
V="$R/bin/vortex"

version 2.7.0
"$V" novedades mostrar
probar "al instalar de cero no abre nada y queda marcada" "[[ ! -e '$T/abierto' && \$(cat '$E/novedades-vistas') == 2.7.0 ]]"

rm -f "$E/novedades-vistas"
"$V" novedades mostrar --tras-actualizar
probar "la primera actualización sí abre el panel" "grep -q 'serpantinum:toggle novedades' '$T/abierto'"
t="$("$V" novedades texto)"
probar "sin marca: solo la versión instalada" "[[ \$t == *'Versión 2.7.0'* && \$t != *'Versión 2.6.3'* ]]"
probar "sin la parte de «Para actualizar»" "[[ \$t != *'vortex actualizar'* && \$t != *'Para actualizar'* ]]"

echo 2.6.0 >"$E/novedades-vistas"
t="$("$V" novedades texto)"
probar "te saltaste versiones: salen todas las que faltan" "[[ \$t == *'Versión 2.7.0'* && \$t == *'Versión 2.6.3'* ]]"
probar "ni la vista ni las viejas" "[[ \$t != *'Versión 2.6.0'* && \$t != *'Versión 2.5.0'* ]]"
probar "ni las más nuevas que la instalada" "[[ \$t != *'Versión 2.10.0'* ]]"
probar "la más nueva primero" "[[ \$(grep -m1 Versión <<<\"\$t\") == *2.7.0* ]]"

version 2.10.0
echo 2.7.0 >"$E/novedades-vistas"
probar "2.10 es más nueva que 2.7" "[[ \$('$V' novedades texto) == *'Versión 2.10.0'* ]]"
version 2.7.0

"$V" novedades texto --marcar >/dev/null
probar "«--marcar» las deja vistas" "[[ \$(cat '$E/novedades-vistas') == 2.7.0 ]]"
rm -f "$T/abierto"
"$V" novedades mostrar --tras-actualizar
probar "actualizar sin versión nueva no abre nada" "[[ ! -e '$T/abierto' ]]"
probar "abrirlo desde el lanzador muestra la última" "[[ \$('$V' novedades texto) == *'Versión 2.7.0'* ]]"

echo 2.6.3 >"$E/novedades-vistas"
touch "$E/modo-juego"
"$V" novedades mostrar --tras-actualizar
probar "jugando no se abre" "[[ ! -e '$T/abierto' ]]"
probar "y queda pendiente" "[[ \$(cat '$E/novedades-vistas') == 2.6.3 ]]"
rm -f "$E/modo-juego"
FALSO_PANTALLA=1 "$V" novedades mostrar --tras-actualizar
probar "con algo en pantalla completa tampoco" "[[ ! -e '$T/abierto' ]]"
"$V" novedades mostrar
probar "al entrar de nuevo, ya sale" "grep -q 'toggle novedades' '$T/abierto'"

rm -f "$T/abierto" "$R/docs/notas-2.7.0.md"
echo 2.6.3 >"$E/novedades-vistas"
"$V" novedades mostrar --tras-actualizar
probar "una versión sin notas no abre un panel vacío" "[[ ! -e '$T/abierto' ]]"

probar "las notas de esta versión existen" "[[ -s '$AQUI/docs/notas-$(sed -n 's/^VORTEX_VERSION="\(.*\)"$/\1/p' "$AQUI/install/lib.sh").md' ]]"
probar "el panel está registrado en el shell" \
  "grep -q 'novedades/NovedadesPopup.qml' '$AQUI/shell/src/quickshell/WindowRegistry.js' && [[ \$(grep -c '\"novedades\"\|=== \"novedades\"' '$AQUI/shell/src/quickshell/Main.qml') == 3 ]]"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
