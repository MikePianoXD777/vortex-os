#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de cómo «vortex actualizar» y el primer inicio ponen Vortex App Installer (1.10) de app por
# defecto, con xdg-mime falso en un HOME temporal: no toca tus asociaciones. Las pruebas de la app
# misma están en apps/app-installer/pruebas (pytest).
#
#   pruebas/app-installer.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-app-installer.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

H="$T/home"
mkdir -p "$T/bin" "$H/.local/share/applications" "$T/sistema/applications"
# xdg-mime falso: las asociaciones viven en un archivo «tipo=app»
cat >"$T/bin/xdg-mime" <<EOF
#!/bin/bash
db="$T/mime.db"
touch "\$db"
case "\$1 \$2" in
  "query default") sed -n "s|^\$3=||p" "\$db" | tail -1 ;;
  "default "*) echo "\$3=\$2" >>"\$db" ;;
esac
EOF
printf '#!/bin/sh\nexit 0\n' >"$T/bin/vortex-app-installer"
chmod +x "$T/bin/"*
touch "$T/sistema/applications/org.kde.ark.desktop"
# Ya elegiste Ark para los .zip; los .rar apuntan a una app que ya no existe
printf 'application/zip=org.kde.ark.desktop\napplication/vnd.rar=file-roller-viejo.desktop\n' >"$T/mime.db"

asociar() {
  HOME="$H" XDG_STATE_HOME="$H/.local/state" XDG_DATA_HOME="$H/.local/share" XDG_DATA_DIRS="$T/sistema" \
    PATH="$T/bin:$PATH" bash -c '
      set -uo pipefail
      AVISOS_ESTADO="$HOME/.local/state/vortex"
      source <(sed -n "/^APP_INSTALLER_MIME=(/,/)$/p; /^existe_desktop() {/,/^}/p; /^asociar_app_installer() {/,/^}/p" "$1")
      asociar_app_installer' _ "$AQUI/bin/vortex"
}
pide() { sed -n "s|^$1=||p" "$T/mime.db" | tail -1; }

echo "== asociar_app_installer"
asociar
probar "los .7z, .AppImage y .jar quedan con App Installer" \
  "[[ \$(pide application/x-7z-compressed) == vortex-app-installer.desktop && \$(pide application/vnd.appimage) == vortex-app-installer.desktop && \$(pide application/x-java-archive) == vortex-app-installer.desktop ]]"
probar "respeta Ark para los .zip (lo elegiste tú)" "[[ \$(pide application/zip) == org.kde.ark.desktop ]]"
probar "los .rar con una app que ya no existe pasan a App Installer" "[[ \$(pide application/vnd.rar) == vortex-app-installer.desktop ]]"
probar "los scripts no (se editan a menudo)" "[[ -z \$(pide application/x-shellscript) ]]"
probar "deja la marca de que ya lo hizo" "[[ -e '$H/.local/state/vortex/app-installer-asociado' ]]"
echo 'application/x-7z-compressed=org.kde.ark.desktop' >>"$T/mime.db"
asociar
probar "la segunda vez no cambia nada (si luego eliges otra, se queda)" "[[ \$(pide application/x-7z-compressed) == org.kde.ark.desktop ]]"
rm -rf "$H/.local/state/vortex"
rm "$T/bin/vortex-app-installer"
: >"$T/mime.db"
asociar
probar "sin la app instalada no asocia nada ni deja marca" "[[ ! -s '$T/mime.db' && ! -e '$H/.local/state/vortex/app-installer-asociado' ]]"

echo
if ((FALLOS)); then echo "$FALLOS fallaron"; exit 1; fi
echo "Todo bien"
