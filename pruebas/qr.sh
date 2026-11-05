#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «Pasar al cel» (2.7, bin/vortex-qr): el servidor de verdad en 127.0.0.1, con curl en el lugar
# del cel y la ventana sin pantalla (offscreen). HOME y XDG_* temporales; notify-send y systemctl falsos.
# No toca el cortafuegos.
#
#   pruebas/qr.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VQ="$AQUI/bin/vortex-qr"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if ! command -v qrencode >/dev/null || ! python3 -c 'import PySide6' 2>/dev/null; then
  echo "  --  falta qrencode o PySide6: se salta"
  exit 0
fi

T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-qr.XXXXXX")"
PID=""
trap '[[ -n "$PID" ]] && kill "$PID" 2>/dev/null; rm -rf "$T"' EXIT
mkdir -p "$T/bin" "$T/home" "$T/config" "$T/datos/carpeta/dentro"
printf '#!/bin/sh\nexit 0\n' >"$T/bin/notify-send"
printf '#!/bin/sh\nexit 1\n' >"$T/bin/systemctl"
chmod +x "$T/bin/"*
export PATH="$T/bin:$PATH" HOME="$T/home" XDG_CONFIG_HOME="$T/config" QT_QPA_PLATFORM=offscreen
export VORTEX_QR_IP=127.0.0.1 VORTEX_QR_SALIDA="$T/url"

head -c 300000 /dev/urandom >"$T/datos/Foto de mañana.jpg"
echo uno >"$T/datos/carpeta/uno.txt"
echo dos >"$T/datos/carpeta/dentro/dos.txt"

arrancar() { # ARCHIVOS...: deja el enlace en $URL
  rm -f "$T/url"
  "$VQ" "$@" >/dev/null 2>"$T/error" &
  PID=$!
  local _
  for _ in {1..50}; do [[ -s "$T/url" ]] && break; sleep 0.1; done
  URL="$(cat "$T/url" 2>/dev/null)"
}
parar() { kill "$PID" 2>/dev/null; wait "$PID" 2>/dev/null; PID=""; }

arrancar "$T/datos/Foto de mañana.jpg"
probar "da un enlace con clave, en los puertos de la regla" "[[ \$URL =~ ^http://127\.0\.0\.1:176[0-4]/[A-Za-z0-9_-]{16}\$ ]]"
base="${URL%/*}"
probar "sin la clave no da nada" "[[ \$(curl -s -o /dev/null -w '%{http_code}' '$base/') == 404 ]]"
probar "con otra clave tampoco" "[[ \$(curl -s -o /dev/null -w '%{http_code}' '$base/otra') == 404 ]]"
probar "ni leer otras rutas" "[[ \$(curl -s --path-as-is -o /dev/null -w '%{http_code}' '$base/../../etc/passwd') == 404 ]]"
curl -s -D "$T/cabeceras" -I "$URL" >/dev/null
probar "HEAD no cuenta como bajado (la ventana sigue)" "kill -0 $PID 2>/dev/null"
probar "el nombre con acentos y espacios sale bien" "grep -qi \"filename\*=UTF-8''Foto%20de%20ma%C3%B1ana.jpg\" '$T/cabeceras'"
curl -s -o "$T/bajado" "$URL"
probar "el archivo llega completo" "cmp -s '$T/datos/Foto de mañana.jpg' '$T/bajado'"
# Al terminar de bajarse, la ventana se cierra sola (4 s)
for _ in {1..70}; do kill -0 "$PID" 2>/dev/null || break; sleep 0.1; done
probar "la ventana se cierra sola al terminar" "! kill -0 $PID 2>/dev/null"
PID=""
probar "ya cerrada, el enlace no sirve" "! curl -s -m 2 -o /dev/null '$URL'"

arrancar "$T/datos/carpeta" "$T/datos/Foto de mañana.jpg"
curl -s -o "$T/junto.zip" "$URL"
probar "varios, en un zip con lo de la carpeta" \
  "python3 -c \"import zipfile,sys; n=set(zipfile.ZipFile('$T/junto.zip').namelist()); sys.exit(0 if {'carpeta/uno.txt','carpeta/dentro/dos.txt','Foto de mañana.jpg'} <= n else 1)\""
parar

arrancar "$T/datos/carpeta"
curl -s -D "$T/cabeceras" -o "$T/una.zip" "$URL"
probar "una carpeta sola se llama como ella" "grep -qi \"filename\*=UTF-8''carpeta.zip\" '$T/cabeceras'"
parar

probar "un archivo que no existe: error" "! '$VQ' '$T/no-existe' 2>/dev/null"

U="$XDG_CONFIG_HOME/Thunar/uca.xml"
"$VQ" accion
probar "sin uca.xml, lo crea con «Pasar al cel»" \
  "grep -q '<name>Pasar al cel</name>' '$U' && grep -q '<command>vortex-qr %F</command>' '$U' && grep -q '<directories/>' '$U'"
cat >"$U" <<'FIN'
<?xml version="1.0" encoding="UTF-8"?>
<actions>
<action><name>Open Terminal Here</name><unique-id>1</unique-id></action>
</actions>
FIN
"$VQ" accion
"$VQ" accion
probar "respeta las tuyas y no se repite" \
  "grep -q 'Open Terminal Here' '$U' && [[ \$(grep -c '<unique-id>vortex-qr</unique-id>' '$U') == 1 ]]"

echo
if ((FALLOS)); then echo "Fallaron $FALLOS"; exit 1; fi
echo "Todas las pruebas pasaron."
