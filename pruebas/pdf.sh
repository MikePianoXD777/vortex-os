#!/usr/bin/env bash
# Prueba de «Fotos de la tarea a PDF» (2.5, bin/vortex-pdf) con ImageMagick de verdad: «fotos» hechas aquí
# (una hoja chueca sobre una mesa con sombra, y otra que llena la foto con viñeta), el PDF junto a ellas, y la
# acción «Hacer PDF» de Thunar en un uca.xml temporal. HOME y XDG_* temporales; notify-send y xdg-open son
# falsos (anotan lo que les piden y contestan el botón de RESPUESTA).
#
#   pruebas/pdf.sh               (GUARDAR=carpeta deja ahí las fotos y las páginas, para verlas)
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VP="$AQUI/bin/vortex-pdf"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if ! command -v magick >/dev/null || ! command -v pdfinfo >/dev/null; then
  echo "  --  falta magick o pdfinfo: se salta"
  exit 0
fi
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-pdf.XXXXXX")"
trap '[[ -n "${GUARDAR:-}" ]] && cp -r "$T/pag" "$T/fotos" "$GUARDAR"/; rm -rf "$T"' EXIT
mkdir -p "$T/home" "$T/cfg" "$T/bin" "$T/fotos" "$T/pag"
export HOME="$T/home" XDG_CONFIG_HOME="$T/cfg" PATH="$T/bin:$PATH" AVISOS="$T/avisos"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "${@: -2:1}" >>"$AVISOS"\nprintf "%%s\\n" "${RESPUESTA:-}"\n' >"$T/bin/notify-send"
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$1" >>"%s/abiertos"\n' "$T" >"$T/bin/xdg-open"
chmod +x "$T/bin/"*
: >"$AVISOS"

echo "== Fotos de prueba"
{ echo 'font-size 36'; echo 'text 90,140 "Tarea de matemáticas"'; echo 'font-size 26'
  for i in $(seq 0 22); do echo "text 90,$((230 + i * 52)) \"Ejercicio $((i + 1)): 3/4 + 1/2 = 5/4, se simplifica y se comprueba\""; done
} >"$T/hoja.mvg"
magick -size 1100x1500 xc:white -fill '#222' -font DejaVu-Sans -draw "@$T/hoja.mvg" "$T/hoja.png"
# 1: la hoja girada 6° sobre una mesa café, con sombra de un lado
magick -size 1600x2000 xc:'#5b4630' -attenuate 0.4 +noise Gaussian \
  \( "$T/hoja.png" -background none -rotate 6 \) -gravity center -geometry +20-30 -composite \
  \( -size 1600x2000 gradient:'#ffffff-#707070' -rotate 90 \) -compose Multiply -composite -quality 88 "$T/fotos/b-mesa.jpg"
# 2: la hoja llena la foto, crema y con viñeta, girada 3°
magick -size 1100x1500 xc:'#f4f0e6' -fill '#1a237e' -font DejaVu-Sans -draw "@$T/hoja.mvg" \
  \( -size 1100x1500 radial-gradient:'#ffffff-#9a9a9a' \) -compose Multiply -composite \
  -background '#9a9a9a' -rotate -3 -quality 88 "$T/fotos/a-llena.jpg"
echo "no soy una foto" >"$T/fotos/mala.jpg"
probar "se hicieron las fotos" "[[ -s '$T/fotos/b-mesa.jpg' && -s '$T/fotos/a-llena.jpg' ]]"

echo "== El PDF"
# shellcheck disable=SC2034  # salida la lee probar (eval)
salida="$(RESPUESTA=abrir VORTEX_PDF_PAGINAS="$T/pag" "$VP" "$T/fotos/b-mesa.jpg" "$T/fotos/a-llena.jpg")"
probar "queda junto a las fotos, «Tarea fecha hora.pdf»" "[[ \"\$salida\" == '$T/fotos/Tarea '*.pdf && -s \"\$salida\" ]]"
probar "una página por foto" "pdfinfo \"\$salida\" | grep -qE '^Pages: +2$'"
probar "en el orden de sus nombres (a-llena primero, la que no tenía mesa)" \
  "[[ \$(magick '$T/pag/000.jpg' -format %w info:) -gt 1100 ]]"
dim() { magick "$1" -format '%w %h' info:; }
read -r w h <<<"$(dim "$T/pag/001.jpg")"
probar "la de la mesa se recorta a la hoja (de 1600x2000 a cerca de 1100x1500: $w x $h)" \
  "(( w > 1000 && w < 1250 && h > 1400 && h < 1650 ))"
ang() { magick "$1" -deskew 40% -format '%[deskew:angle]' info: | awk '{printf "%d", ($1 < 0 ? -$1 : $1) * 10}'; }
probar "y queda derecha (menos de 1°)" "(( \$(ang '$T/pag/001.jpg') < 10 ))"
probar "la que llena la foto también se endereza" "(( \$(ang '$T/pag/000.jpg') < 10 ))"
claro() { magick "$1" -colorspace gray -format '%[fx:int(100*mean)]' info:; }
probar "el fondo queda blanco, sin la sombra ni la mesa (claridad $(claro "$T/pag/001.jpg") %)" "(( \$(claro '$T/pag/001.jpg') >= 90 ))"
probar "sin la viñeta en la que llena la foto (claridad $(claro "$T/pag/000.jpg") %)" "(( \$(claro '$T/pag/000.jpg') >= 88 ))"
probar "avisa «Listo» con las páginas, y «Abrir» abre el PDF" \
  "grep -qx 'Listo: Tarea .*\.pdf' '$AVISOS' && grep -qxF \"\$salida\" '$T/abiertos'"
# shellcheck disable=SC2034  # uno y dos los lee probar (eval)
uno="$("$VP" "$T/fotos/b-mesa.jpg")"
# shellcheck disable=SC2034
dos="$("$VP" "$T/fotos/b-mesa.jpg")"
probar "con una sola foto se llama como ella, y no pisa el anterior" \
  "[[ \"\$uno\" == '$T/fotos/b-mesa.pdf' && \"\$dos\" == '$T/fotos/b-mesa (2).pdf' ]] && pdfinfo \"\$uno\" | grep -qE '^Pages: +1$'"
antes="$(ls "$T/fotos" | wc -l)"
probar "una foto que no se puede leer: avisa y no deja PDF" \
  "! \"$VP\" '$T/fotos/mala.jpg' 2>/dev/null && tail -1 '$AVISOS' | grep -qx 'No se pudo hacer el PDF' && [[ \$(ls '$T/fotos' | wc -l) == $antes ]]"

echo "== La acción de Thunar"
U="$T/cfg/Thunar/uca.xml"
"$VP" accion
probar "sin uca.xml, lo crea con «Hacer PDF» para imágenes" \
  "grep -q '<name>Hacer PDF</name>' '$U' && grep -q '<command>vortex-pdf %F</command>' '$U' && grep -q '<image-files/>' '$U'"
cat >"$U" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<actions>
<action>
	<icon>utilities-terminal</icon>
	<name>Open Terminal Here</name>
	<unique-id>1710575157271461-1</unique-id>
	<command>foot -D %f</command>
	<patterns>*</patterns>
	<directories/>
</action>
</actions>
EOF
"$VP" accion
"$VP" accion
probar "con tus acciones, las deja y agrega la suya una sola vez" \
  "grep -q 'Open Terminal Here' '$U' && [[ \$(grep -c '<unique-id>vortex-pdf</unique-id>' '$U') == 1 ]]"
probar "y sigue siendo XML válido" "python3 -c 'import sys,xml.dom.minidom as m; m.parse(sys.argv[1])' '$U'"
echo "algo raro" >"$U"
"$VP" accion
probar "un uca.xml que no entiende no lo toca" "[[ \"\$(cat '$U')\" == 'algo raro' ]]"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "FALLARON $FALLOS pruebas"; cat "$AVISOS"; exit 1; }
