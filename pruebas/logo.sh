#!/usr/bin/env bash
# El logo de Vortexine's OS («Rotor», docs/logo/rotor.svg) es solo del sistema: el ícono del sistema (instalador e
# ícono «vortex-os», también el de la Shop), el shell y la terminal. Las apps Vortexine llevan su propio logo
# (Mike, 09/11/2026): esta prueba también revisa que ninguna se quede con el Rotor.
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
done < <(git ls-files --cached --others --exclude-standard 'apps/shop/vortex_shop/qml/vortex-os.svg' \
  'iso/airootfs/etc/calamares/branding/vortex/vortex-os.svg' 'iso/airootfs/usr/share/icons/hicolor/scalable/apps/*.svg')
[[ $distintos == 0 ]] && bien "sistema e instalador: el logo Rotor"

rotor_en_apps=0
while IFS= read -r f; do
  cmp -s "$f" "$MAESTRO" && { mal "$f es el Rotor: las apps llevan su propio logo"; rotor_en_apps=$((rotor_en_apps + 1)); }
done < <(git ls-files --cached --others --exclude-standard \
  'apps/*/share/icons/hicolor/*/apps/*.svg' 'apps/*/qml/logo.svg' 'apps/shop/vortex_shop/qml/propias/*.svg' \
  'apps/miku/icono/*.svg')
[[ $rotor_en_apps == 0 ]] && bien "apps: cada una con su propio logo"

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
