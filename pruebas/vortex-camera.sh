#!/usr/bin/env bash
# Pruebas de Vortexine's Camera (2.4): el motor (cámaras, búsqueda DHIP, RTSP, fotos, grabación, PTZ)
# y la ventana sin pantalla, con una cámara falsa en 127.0.0.1; y que «vortex camara» la instala y la
# quita en un HOME temporal. No toca tus cámaras ni tu configuración.
#
#   pruebas/vortex-camera.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fallos=0

(cd "$AQUI/apps/vortex-camera" && python3 -W error::ResourceWarning -m unittest discover -s pruebas "$@") || fallos=1

casa="$(mktemp -d)"
trap 'rm -rf "$casa"' EXIT
camara() { HOME="$casa" XDG_DATA_HOME="$casa/.local/share" XDG_CONFIG_HOME="$casa/.config" "$AQUI/bin/vortex-camara" "$@"; }
revisar() { if eval "$2"; then echo "ok: $1"; else echo "FALLA: $1"; fallos=1; fi; }

camara estado >/dev/null 2>&1
revisar "sin instalar, estado dice que no" "[[ $? -eq 1 ]]"
camara instalar >/dev/null
revisar "instalar deja el código, el lanzador y el ícono" \
  "[[ -f '$casa/.local/share/vortex-camera/vortex_camera/__init__.py' && -L '$casa/.local/bin/vortex-camera' && -f '$casa/.local/share/icons/hicolor/scalable/apps/vortex-camera.svg' ]]"
revisar "el .desktop apunta a la copia instalada" \
  "grep -qx 'Exec=$casa/.local/share/vortex-camera/vortex-camera' '$casa/.local/share/applications/vortex-camera.desktop'"
revisar "sin caché de íconos en tu carpeta" "[[ ! -e '$casa/.local/share/icons/hicolor/icon-theme.cache' ]]"
revisar "la copia instalada arranca" "[[ \"\$('$casa/.local/bin/vortex-camera' --version)\" == \"Vortexine's Camera \"* ]]"
mkdir -p "$casa/.config/vortex-camera" && echo '{}' >"$casa/.config/vortex-camera/camaras.json"
camara quitar >/dev/null
revisar "quitar borra la app" "[[ ! -e '$casa/.local/share/vortex-camera' && ! -e '$casa/.local/bin/vortex-camera' && ! -e '$casa/.local/share/applications/vortex-camera.desktop' ]]"
revisar "quitar deja tus cámaras" "[[ -f '$casa/.config/vortex-camera/camaras.json' ]]"
camara actualizar >/dev/null
revisar "actualizar no la instala a quien no la tiene" "[[ ! -e '$casa/.local/share/vortex-camera' ]]"

exit "$fallos"
