#!/usr/bin/env bash
# Pruebas de Vortexine's Runner (3.2): el motor (biblioteca, .exe, Protons, umu) y la ventana
# sin pantalla, con umu y Protons falsos; y que «vortex runner» la instala y la
# quita en un HOME temporal. No corre nada de Windows ni toca tu biblioteca.
#
#   pruebas/vortex-runner.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fallos=0

(cd "$AQUI/apps/vortex-runner" && QT_QPA_PLATFORM=offscreen python3 -m unittest discover -s pruebas -p 'test_*.py' "$@") || fallos=1

casa="$(mktemp -d)"
trap 'rm -rf "$casa"' EXIT
runner() { HOME="$casa" XDG_DATA_HOME="$casa/.local/share" XDG_CONFIG_HOME="$casa/.config" "$AQUI/bin/vortex-exe" "$@"; }
revisar() { if eval "$2"; then echo "ok: $1"; else echo "FALLA: $1"; fallos=1; fi; }

runner estado >/dev/null 2>&1
revisar "sin instalar, estado dice que no" "[[ $? -eq 1 ]]"
runner instalar >/dev/null
revisar "instalar deja el código, el lanzador y el ícono" \
  "[[ -f '$casa/.local/share/vortex-runner/vortex_runner/__init__.py' && -L '$casa/.local/bin/vortex-runner' && -f '$casa/.local/share/icons/hicolor/scalable/apps/vortex-runner.svg' ]]"
revisar "el .desktop apunta a la copia instalada" \
  "grep -qx 'Exec=$casa/.local/share/vortex-runner/vortex-runner %f' '$casa/.local/share/applications/vortex-runner.desktop'"
revisar "sin caché de íconos en tu carpeta" "[[ ! -e '$casa/.local/share/icons/hicolor/icon-theme.cache' ]]"
revisar "la copia instalada arranca" "[[ \"\$('$casa/.local/bin/vortex-runner' --version)\" == \"Vortexine's Runner \"* ]]"
# shellcheck disable=SC2034  # se usa dentro del eval de revisar
estado_runner="$(runner estado | head -1)"
revisar "estado dice la versión" "[[ \"\$estado_runner\" =~ ^Vortexine.s\ Runner\ [0-9]+\.[0-9]+ ]]"
mkdir -p "$casa/.local/share/vortex-runner-datos" && echo '[]' >"$casa/.local/share/vortex-runner-datos/biblioteca.json"
printf '[Desktop Entry]\nName=Juego\n' >"$casa/.local/share/applications/vortex-runner-juego-123.desktop"
runner quitar >/dev/null
revisar "quitar borra la app" "[[ ! -e '$casa/.local/share/vortex-runner' && ! -e '$casa/.local/bin/vortex-runner' && ! -e '$casa/.local/share/applications/vortex-runner.desktop' ]]"
revisar "quitar deja tu biblioteca" "[[ -f '$casa/.local/share/vortex-runner-datos/biblioteca.json' ]]"
revisar "quitar borra los accesos de tus programas (sin Runner no abren)" "[[ ! -e '$casa/.local/share/applications/vortex-runner-juego-123.desktop' ]]"
runner actualizar >/dev/null
revisar "actualizar no la instala a quien no la tiene" "[[ ! -e '$casa/.local/share/vortex-runner' ]]"

exit "$fallos"
