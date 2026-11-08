#!/usr/bin/env bash
# El logo de Vortexine's OS («Rotor», docs/logo/rotor.svg) es el mismo en todo: el icono y el logo de cada app, el
# instalador, el shell y la terminal. Si algo se queda con otro dibujo, esta prueba lo dice.
#
#   pruebas/logo.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$AQUI" || exit 1
MAESTRO=docs/logo/rotor.svg
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

distintos=0
while IFS= read -r f; do
  cmp -s "$f" "$MAESTRO" || { mal "$f no es el logo"; distintos=$((distintos + 1)); }
done < <(git ls-files --cached --others --exclude-standard \
  'apps/*/share/icons/hicolor/scalable/apps/*.svg' 'apps/*/qml/logo.svg' 'apps/shop/vortex_shop/qml/vortex-os.svg' \
  'apps/shop/vortex_shop/qml/propias/*.svg' 'apps/miku/icono/*.svg' \
  'iso/airootfs/etc/calamares/branding/vortex/vortex-os.svg' 'iso/airootfs/usr/share/icons/hicolor/scalable/apps/*.svg')
[[ $distintos == 0 ]] && bien "apps e instalador: el mismo logo"

for t in 16 24; do
  f="apps/app-installer/share/icons/hicolor/${t}x$t/apps/vortex-app-installer.svg"
  if [[ "$(sed "s/width=\"$t\" height=\"$t\"/width=\"256\" height=\"256\"/" "$f")" == "$(cat "$MAESTRO")" ]]; then
    bien "App Installer a $t px: el mismo dibujo"
  else
    mal "App Installer a $t px: otro dibujo"
  fi
done

# La silueta del shell tiene las mismas aspas que el logo, sin degradado (el shell la pinta con el color del tema).
aspas() { grep -o '<path d="[^"]*"' "$1"; }
if [[ -n "$(aspas "$MAESTRO")" && "$(aspas shell/src/assets/logo.svg)" == "$(aspas "$MAESTRO")" ]] &&
  ! grep -q 'url(#' shell/src/assets/logo.svg; then
  bien "shell: la silueta del logo, sin degradado"
else
  mal "shell: la silueta no coincide con el logo"
fi

grep -q '\$1' config/terminal/vortex-logo.txt && grep -q '\$3' config/terminal/vortex-logo.txt &&
  bien "terminal: el texto usa los colores del logo" || mal "terminal: el texto sin colores"
[[ -s config/terminal/vortex-logo.png ]] && bien "terminal: la imagen existe" || mal "terminal: falta la imagen"

if ((FALLOS)); then
  echo "Fallaron $FALLOS"
  exit 1
fi
echo
echo "Todas las pruebas pasaron."
