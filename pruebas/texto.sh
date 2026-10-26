#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de bin/vortex-texto (1.8, copiar el texto de la pantalla) sin tocar la pantalla: slurp, grim,
# wl-copy, notify-send y tesseract falsos en un PATH temporal. Si tesseract de verdad está, además lee
# una imagen generada (con ImageMagick) en una carpeta temporal.
#
#   pruebas/texto.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-texto-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/run"
# slurp contesta lo que diga $T/region (vacío = Esc); tesseract, lo que diga $T/texto.
cat >"$T/bin/slurp" <<EOF
#!/bin/sh
cat "$T/region" 2>/dev/null
EOF
# shellcheck disable=SC2016
printf '#!/bin/sh\nfor a; do last=$a; done; : >"$last"\n' >"$T/bin/grim"
cat >"$T/bin/tesseract" <<EOF
#!/bin/sh
[ "\$1" = --list-langs ] && { printf 'List of available languages (3):\neng\nosd\nspa\n'; exit 0; }
echo "tesseract \$*" >>"$T/registro"
cat "$T/texto" 2>/dev/null
EOF
printf '#!/bin/sh\ncat >"%s/portapapeles"\n' "$T" >"$T/bin/wl-copy"
printf '#!/bin/sh\necho "notify-send $*" >>"%s/registro"\n' "$T" >"$T/bin/notify-send"
chmod +x "$T/bin/"*

# shellcheck disable=SC2120
correr() { : >"$T/registro"; rm -f "$T/portapapeles"; XDG_RUNTIME_DIR="$T/run" PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex-texto" "$@"; }

echo "== vortex-texto"
echo "10,10 200x50" >"$T/region"
printf '\n\nHola, señor   \n\nSegundo párrafo\n\n\n' >"$T/texto"
correr
[[ "$(cat "$T/portapapeles" 2>/dev/null)" == $'Hola, señor\n\nSegundo párrafo' ]] && bien "copia el texto limpio" ||
  mal "portapapeles: $(cat -A "$T/portapapeles" 2>/dev/null)"
grep -q -- "-l spa+eng" "$T/registro" && bien "lee en español e inglés" || mal "idiomas: $(cat "$T/registro")"
grep -q "Texto copiado Hola, señor Segundo párrafo" "$T/registro" && bien "avisa con el principio del texto" ||
  mal "aviso: $(cat "$T/registro")"
[[ -z "$(ls -A "$T/run")" ]] && bien "no deja archivos temporales" || mal "quedó: $(ls "$T/run")"

: >"$T/region"
correr
[[ ! -e "$T/portapapeles" && ! -s "$T/registro" ]] && bien "Esc: no hace nada" || mal "con Esc hizo algo"

echo "10,10 200x50" >"$T/region"
printf '   \n\n' >"$T/texto"
correr
grep -q "No encontré texto" "$T/registro" && [[ ! -e "$T/portapapeles" ]] && bien "zona sin texto: lo dice y no copia" ||
  mal "sin texto: $(cat "$T/registro")"
[[ "$(grep -c "^tesseract" "$T/registro")" == 2 ]] && bien "sin texto reintenta como un solo bloque (psm 6)" ||
  mal "reintentos: $(grep -c "^tesseract" "$T/registro")"

mkdir -p "$T/sin"
for p in bash cat grep sed awk tr cut mktemp rm head tail; do ln -s "$(command -v "$p")" "$T/sin/$p"; done
ln -s "$T/bin/notify-send" "$T/sin/notify-send"
: >"$T/registro"
XDG_RUNTIME_DIR="$T/run" PATH="$T/sin" bash "$AQUI/bin/vortex-texto"
grep -q "Falta tesseract.*tesseract-data-spa" "$T/registro" && bien "sin tesseract dice cómo instalarlo" ||
  mal "sin tesseract: $(cat "$T/registro")"

if command -v tesseract >/dev/null && command -v magick >/dev/null && [[ "$(magick -list font 2>/dev/null)" == *"Font: DejaVu-Sans"* ]]; then
  magick -size 700x70 xc:'#1e1e2e' -fill '#cdd6f4' -font DejaVu-Sans -pointsize 24 -annotate +20+45 "Copiar texto de la pantalla" "$T/real.png"
  leido="$(XDG_RUNTIME_DIR="$T/run" bash "$AQUI/bin/vortex-texto" "$T/real.png")"
  [[ "$leido" == *"Copiar texto de la pantalla"* ]] && bien "tesseract de verdad lee una imagen" || mal "leyó: $leido"
fi

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
