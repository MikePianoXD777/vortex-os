#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «vortex sonidos» (2.2, «Agregar desde un enlace» en Ajustes → Notificaciones) con QS_STATE_DIR
# en una carpeta temporal y un servidor web local: no toca tus sonidos ni baja nada de internet.
# (La página de myinstants.com no se prueba aquí: va a internet. A mano:
#  vortex sonidos agregar https://www.myinstants.com/es/instant/vine-boom-sound-70972/)
#
#   pruebas/sonidos.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-sonidos.XXXXXX")"
SERVIDOR=""
trap '[[ -n "$SERVIDOR" ]] && kill "$SERVIDOR" 2>/dev/null; rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

command -v ffmpeg >/dev/null || { echo "Falta ffmpeg para hacer los audios de prueba" >&2; exit 1; }
mkdir -p "$T/web" "$T/archivos"
ffmpeg -v error -f lavfi -i sine=frequency=880:duration=0.5 "$T/archivos/Ding Dong.wav"
cp "$T/archivos/Ding Dong.wav" "$T/web/campana.wav"
echo "<html>no soy audio</html>" >"$T/web/pagina.html"
echo "tampoco" >"$T/archivos/nota.mp3"
head -c $((11 * 1024 * 1024)) /dev/zero >"$T/web/enorme.mp3"

python3 -u -m http.server 0 --bind 127.0.0.1 --directory "$T/web" >"$T/servidor.log" 2>&1 &
SERVIDOR=$!
for _ in $(seq 50); do grep -q "port" "$T/servidor.log" && break; sleep 0.1; done
PUERTO="$(grep -oE 'port [0-9]+' "$T/servidor.log" | head -1 | cut -d' ' -f2)"
[[ -n "$PUERTO" ]] || { echo "No arrancó el servidor de prueba" >&2; exit 1; }
WEB="http://127.0.0.1:$PUERTO"

vortex() { QS_STATE_DIR="$T/estado" bash "$AQUI/bin/vortex" "$@"; }
D="$T/estado/sounds/notifications"

echo "== vortex sonidos"
[[ -z "$(vortex sonidos lista)" ]] && bien "sin carpeta: la lista sale vacía" || mal "lista: $(vortex sonidos lista)"

salida="$(vortex sonidos agregar "$T/archivos/Ding Dong.wav" 2>/dev/null)"
[[ -f "$D/Ding Dong.wav" && "$(tail -1 <<<"$salida")" == "$D/Ding Dong.wav" ]] &&
  bien "un archivo: lo copia con su nombre y da la ruta en la última línea" || mal "salida: $salida"

vortex sonidos agregar "$T/archivos/Ding Dong.wav" >/dev/null 2>&1
[[ -f "$D/Ding Dong (2).wav" ]] && bien "no pisa uno que ya está: «Ding Dong (2)»" || mal "carpeta: $(ls "$D")"

salida="$(vortex sonidos agregar "$WEB/campana.wav?descarga=1" "Mi/campana" 2>/dev/null)"
[[ -f "$D/Mi-campana.wav" && "$(tail -1 <<<"$salida")" == "$D/Mi-campana.wav" ]] &&
  bien "un enlace directo con nombre (sin «/» ni el «?…» en la extensión)" || mal "salida: $salida / $(ls "$D")"

vortex sonidos agregar "$WEB/campana.wav" >/dev/null 2>&1
[[ -f "$D/campana.wav" ]] && bien "sin nombre: el del archivo del enlace" || mal "carpeta: $(ls "$D")"

antes="$(ls "$D" | wc -l)"
error="$(vortex sonidos agregar "$WEB/pagina.html" 2>&1 >/dev/null)" && mal "aceptó una página web" ||
  { [[ "$error" == *"no es un sonido"* ]] && bien "una página que no es audio: «eso no es un sonido»" || mal "error: $error"; }
error="$(vortex sonidos agregar "$WEB/enorme.mp3" 2>&1 >/dev/null)" && mal "aceptó uno de 11 MB" ||
  { [[ "$error" == *"10 MB"* ]] && bien "uno de más de 10 MB no se baja" || mal "error: $error"; }
error="$(vortex sonidos agregar "$WEB/no-existe.mp3" 2>&1 >/dev/null)" && mal "aceptó un 404" ||
  { [[ -n "$error" ]] && bien "un enlace roto sale con un mensaje" || mal "sin mensaje"; }
vortex sonidos agregar "$T/archivos/nota.mp3" >/dev/null 2>&1 && mal "aceptó un .mp3 que no es audio" || bien "un .mp3 falso tampoco"
error="$(vortex sonidos agregar "/no/existe.wav" 2>&1 >/dev/null)" && mal "aceptó un archivo que no existe" ||
  { [[ "$error" == *"no encuentro"* ]] && bien "un archivo que no existe: «no encuentro…»" || mal "error: $error"; }
[[ "$(ls "$D" | wc -l)" == "$antes" ]] && [[ -z "$(find "${TMPDIR:-/tmp}" -maxdepth 1 -name 'vortex-sonido.*' -newer "$T/servidor.log" 2>/dev/null)" ]] &&
  bien "los que fallan no dejan nada (ni en la carpeta ni temporales)" || mal "carpeta: $(ls "$D")"

[[ "$(vortex sonidos lista | tr '\n' '|')" == "campana.wav|Ding Dong (2).wav|Ding Dong.wav|Mi-campana.wav|" ]] &&
  bien "lista: los nombres en orden" || mal "lista: $(vortex sonidos lista | tr '\n' '|')"

vortex sonidos quitar "Ding Dong (2).wav" >/dev/null && [[ ! -e "$D/Ding Dong (2).wav" && -f "$D/Ding Dong.wav" ]] &&
  bien "quitar con extensión borra ese y nada más" || mal "carpeta: $(ls "$D")"
vortex sonidos quitar "campana" >/dev/null && [[ ! -e "$D/campana.wav" ]] && bien "quitar sin extensión también" || mal "carpeta: $(ls "$D")"
vortex sonidos quitar "no-existe" >/dev/null 2>&1 && mal "quitó uno que no existe" || bien "quitar uno que no existe sale con error"
vortex sonidos quitar "../estado" >/dev/null 2>&1 && mal "aceptó «../»" || bien "quitar no sale de la carpeta"

echo
((FALLOS == 0)) && echo "Todo bien" || { echo "$FALLOS fallos"; exit 1; }
