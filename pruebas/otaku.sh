#!/usr/bin/env bash
# Pruebas del modo otaku (bin/vortex-otaku), sin red, sin shell y sin tocar tu escritorio:
#   1. sintaxis (bash -n) y shellcheck si está instalado;
#   2. el jpg de respaldo del repo (original, 2560x1440);
#   3. el comando entero con HOME temporal y curl, quickshell, hyprctl, notify-send y sleep
#      falsos: encender con Wallhaven (respuesta de la API inventada), apagar, otro fondo,
#      sin red (fondo de respaldo), con la IPC del shell caída, ajustes sin «theme», ajustes
#      rotos o inexistentes y respuestas de la API que intentan mandarnos a otro servidor.
# Nada se baja de Internet: el curl falso sirve un JSON y «imágenes» (bytes de jpg) de mentira.
#
#   pruebas/otaku.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-otaku-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

echo "== Sintaxis"
probar "bash -n bin/vortex-otaku" "bash -n '$AQUI/bin/vortex-otaku'"
probar "bash -n bin/vortex" "bash -n '$AQUI/bin/vortex'"
if command -v shellcheck >/dev/null; then
  probar "shellcheck" "shellcheck -x -S warning '$AQUI/bin/vortex-otaku' '$AQUI/pruebas/otaku.sh'"
else
  echo "  --  shellcheck no está instalado: se salta"
fi
for c in jq flock shuf od; do
  command -v "$c" >/dev/null || { echo "Falta $c para las pruebas"; exit 1; }
done

echo "== Fondo de respaldo del repo"
RESP="$AQUI/config/fondos/otaku/otaku-respaldo.jpg"
probar "existe y es un jpg" "[[ -s '$RESP' && \"\$(head -c 3 '$RESP' | od -An -tx1 | tr -d ' \\n')\" == ffd8ff ]]"
if python3 -c 'import PIL' 2>/dev/null; then
  probar "mide 2560x1440" "[[ \"\$(python3 -I -c \"from PIL import Image; print(Image.open('$RESP').size)\")\" == '(2560, 1440)' ]]"
else
  echo "  --  python-pillow no está instalado: no se mide el jpg"
fi
probar "ninguna imagen del personaje en el repo (solo el respaldo original)" \
  "[[ -z \"\$(cd '$AQUI' && git ls-files | grep -i -E 'wallhaven|nakano|miku' | grep -v -E '^(bin/vortex-otaku|pruebas/otaku.sh|shell/)' || true)\" ]]"

# --- Entorno falso ---
mkdir -p "$T/bin"
cat >"$T/bin/curl" <<'EOF'
#!/usr/bin/env bash
# curl falso: registra la petición; la API devuelve $T/api.json y las imágenes, bytes de jpg.
url="" salida=""
while (($#)); do
  case "$1" in
    -o) salida="$2"; shift 2 ;;
    -A | --max-time | --retry | --max-filesize | --proto | --proto-redir) shift 2 ;;
    -*) shift ;;
    *) url="$1"; shift ;;
  esac
done
echo "$url" >>"$T/curl.log"
[[ -e "$T/sin-red" ]] && exit 6
case "$url" in
  https://wallhaven.cc/api/v1/search\?*) cat "$T/api.json" ;;
  https://w.wallhaven.cc/full/*/wallhaven-bbbbbb.*) exit 22 ;;
  https://w.wallhaven.cc/full/*)
    printf '\xff\xd8\xff\xe0' >"$salida"
    head -c 70000 /dev/urandom >>"$salida"
    ;;
  *) exit 6 ;;
esac
EOF
cat >"$T/bin/quickshell" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$T/ipc.log"
[[ -e "$T/ipc-falla" ]] && exit 1
exit 0
EOF
cat >"$T/bin/hyprctl" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$T/hyprctl.log"
case "$*" in
  *"-j monitors"*) echo '[{"name":"DP-1"},{"name":"HDMI-A-1"}]' ;;
  *) exit 1 ;;
esac
EOF
cat >"$T/bin/notify-send" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$T/notify.log"
EOF
cat >"$T/bin/sleep" <<'EOF'
#!/usr/bin/env bash
echo "$*" >>"$T/sleep.log"
EOF
chmod +x "$T"/bin/*
export T

# Respuesta de la API: 7 resultados buenos (uno, bbbbbb, falla al bajar; otro, gggggg, viene sin «url»), uno vertical, uno
# que no es de Wallhaven, uno con un nombre sospechoso y uno sin «url».
python3 - "$T/api.json" <<'EOF'
import json, sys
def r(i, ext="jpg", w=1920, h=1080, host="w.wallhaven.cc", tipo=None, nombre=None, url=True):
    d = {"id": i, "path": f"https://{host}/full/{i[:2]}/{nombre or 'wallhaven-' + i + '.' + ext}",
         "dimension_x": w, "dimension_y": h, "file_type": tipo or ("image/png" if ext == "png" else "image/jpeg")}
    if url:
        d["url"] = f"https://wallhaven.cc/w/{i}"
    return d
datos = [r("aaaaaa"), r("bbbbbb"), r("gggggg", url=False), r("cccccc", "png"), r("dddddd"), r("eeeeee", "png"), r("ffffff"),
         r("hhhhhh", w=1080, h=1920), r("iiiiii", host="evil.example.com"), r("jjjjjj", nombre="..%2f..%2fx.jpg"),
         r("kkkkkk", tipo="image/gif", ext="gif"), r("llllll")]
json.dump({"data": datos, "meta": {"total": len(datos)}}, open(sys.argv[1], "w"))
EOF

H="$T/home"
FONDOS_ESP="$H/Imágenes/Fondos"
# Arranca una «sesión» limpia: HOME nuevo, ajustes del shell y un fondo ya puesto en DP-1.
nueva_sesion() {
  rm -rf "$H"
  rm -f "$T/curl.log" "$T/ipc.log" "$T/notify.log" "$T/hyprctl.log" "$T/sleep.log" "$T/sin-red" "$T/ipc-falla"
  mkdir -p "$H/.config/serpantinum" "$FONDOS_ESP" "$H/.cache/serpantinum/wallpaper"
  printf 'XDG_PICTURES_DIR="$HOME/Imágenes"\n' >"$H/.config/user-dirs.dirs"
  printf 'JPG-DEL-USUARIO' >"$FONDOS_ESP/vortex-azul.jpg"
  printf 'JPG-ORIGINAL-2' >"$FONDOS_ESP/mi fondo (raro).jpg"
  printf '%s' "$FONDOS_ESP/vortex-azul.jpg" >"$H/.cache/serpantinum/wallpaper/current_DP-1"
  printf 'vortex-azul.jpg' >"$H/.cache/serpantinum/wallpaper/current_DP-1_name"
  printf 'PNG' >"$H/.cache/serpantinum/wallpaper/current_wallpaper.png"
  cat >"$H/.config/serpantinum/settings.json" <<EOF
{
  "general": {
    "language": "es"
  },
  "theme": {
    "fontFamily": "Adwaita Mono",
    "borderRadius": 8,
    "activePreset": "Matugen",
    "matugen": true
  },
  "wallpaperDir": "$FONDOS_ESP"
}
EOF
  cp "$H/.config/serpantinum/settings.json" "$T/settings-original.json"
}
# vortex-otaku con el entorno falso. Con 2>&1 se ven también los avisos.
otaku() {
  env -u XDG_CONFIG_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME -u XDG_RUNTIME_DIR -u HYPRLAND_INSTANCE_SIGNATURE -u QS_SETTINGS \
    HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent "$AQUI/bin/vortex-otaku" "$@" 2>&1
}
ajuste() { jq -r "$1" "$H/.config/serpantinum/settings.json"; }
numero_fondos() { find "$FONDOS_ESP/otaku" -maxdepth 1 -type f \( -name '*.jpg' -o -name '*.png' \) 2>/dev/null | wc -l; }
MARCA="$H/.config/vortex/otaku"
ANTES="$H/.config/vortex/otaku-antes.json"
CALL='ipc -p /usr/share/serpantinum/src/quickshell/Shell.qml call wallpaper setWallpaper'

echo "== Encender con Wallhaven"
nueva_sesion
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on termina bien" "((rc == 0))"
probar "queda la marca ~/.config/vortex/otaku" "[[ -f '$MARCA' ]]"
probar "la primera petición es la búsqueda SFW de Nakano Miku" \
  "[[ \"\$(head -n1 '$T/curl.log')\" == 'https://wallhaven.cc/api/v1/search?q=Nakano%20Miku&categories=010&purity=100&sorting=favorites&atleast=1920x1080' ]]"
probar "baja 5 fondos a Imágenes/Fondos/otaku (con tilde y espacios en la ruta)" "((\$(numero_fondos) == 5))"
probar "los que fallan (bbbbbb) se saltan y se baja el siguiente" "[[ ! -e '$FONDOS_ESP/otaku/wallhaven-bbbbbb.jpg' ]] && [[ -e '$FONDOS_ESP/otaku/wallhaven-eeeeee.png' && ! -e '$FONDOS_ESP/otaku/wallhaven-ffffff.jpg' ]]"
probar "no pide nada fuera de wallhaven.cc ni los verticales ni los gif" \
  "! grep -q -E 'evil|hhhhhh|kkkkkk|x\\.jpg|llllll' '$T/curl.log'"
probar "fuentes.json guarda la URL de Wallhaven de cada fondo" \
  "[[ \"\$(jq -r '.fondos | map(.url | startswith(\"https://wallhaven.cc/w/\")) | all' '$FONDOS_ESP/otaku/fuentes.json')\" == true ]] && [[ \"\$(jq '.fondos | length' '$FONDOS_ESP/otaku/fuentes.json')\" == 5 ]]"
probar "el que no traía «url» se arma con su id" \
  "[[ \"\$(jq -r '.fondos[] | select(.id == \"gggggg\") | .url' '$FONDOS_ESP/otaku/fuentes.json')\" == https://wallhaven.cc/w/gggggg ]]"
probar "pone el fondo con la IPC del shell en cada monitor" \
  "grep -q -E '^$CALL DP-1 $FONDOS_ESP/otaku/wallhaven-[a-z]+\\.(jpg|png) fade\$' '$T/ipc.log' && grep -q -E '^$CALL HDMI-A-1 ' '$T/ipc.log'"
probar "guarda el fondo de antes y los ajustes de antes" \
  "[[ \"\$(jq -c . '$ANTES')\" == '{\"fondos\":{\"DP-1\":\"$FONDOS_ESP/vortex-azul.jpg\"},\"tema\":{\"fontFamily\":\"Adwaita Mono\",\"borderRadius\":8}}' ]]"
probar "ajustes: fuente y bordes nuevos" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Google Sans Flex' && \"\$(ajuste .theme.borderRadius)\" == 16 ]]"
probar "ajustes: lo demás sigue igual" \
  "[[ \"\$(ajuste .theme.activePreset)\" == Matugen && \"\$(ajuste .general.language)\" == es && \"\$(ajuste .wallpaperDir)\" == '$FONDOS_ESP' ]]"
probar "el settings.json sigue con los mismos permisos y sin temporales" \
  "[[ \"\$(stat -c %a '$H/.config/serpantinum/settings.json')\" == \"\$(stat -c %a '$T/settings-original.json')\" && -z \"\$(ls -A '$H/.config/serpantinum' | grep -v '^settings.json\$')\" ]]"
probar "avisa con una notificación" "grep -q 'Modo otaku activado' '$T/notify.log'"
otaku estado >"$T/salida"
probar "estado dice encendido y el fondo" "grep -q 'Modo otaku: encendido' '$T/salida' && grep -q 'Fondo: $FONDOS_ESP/otaku/' '$T/salida'"

echo "== Encender dos veces, otro fondo y apagar"
n_curl="$(wc -l <"$T/curl.log")"
antes_json="$(cat "$ANTES")"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "un segundo «on» no hace nada (ni baja, ni pisa lo de antes)" \
  "((rc == 0)) && [[ \"\$(wc -l <'$T/curl.log')\" == '$n_curl' ]] && [[ \"\$(cat '$ANTES')\" == '$antes_json' ]]"
fondo1="$(sed -n 's/^fondo=//p' "$MARCA")"
otaku otro >/dev/null; rc=$?
fondo2="$(sed -n 's/^fondo=//p' "$MARCA")"
probar "«otro» cambia al siguiente fondo" "((rc == 0)) && [[ '$fondo1' != '$fondo2' && -f '$fondo2' ]] && grep -q \"$CALL DP-1 \$fondo2 fade\" '$T/ipc.log'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "off termina bien" "((rc == 0))"
probar "off devuelve el fondo de antes en DP-1" "grep -q '^$CALL DP-1 $FONDOS_ESP/vortex-azul.jpg fade\$' '$T/ipc.log'"
probar "off pone ese mismo fondo en el monitor que no estaba antes" "grep -q '^$CALL HDMI-A-1 $FONDOS_ESP/vortex-azul.jpg fade\$' '$T/ipc.log'"
probar "off deja los ajustes como estaban" "[[ \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]]"
probar "off borra lo que se bajó y la carpeta" "[[ ! -e '$FONDOS_ESP/otaku' ]]"
probar "no toca los fondos del usuario" "[[ -f '$FONDOS_ESP/vortex-azul.jpg' && -f '$FONDOS_ESP/mi fondo (raro).jpg' ]]"
probar "off quita la marca y lo guardado" "[[ ! -e '$MARCA' && ! -e '$ANTES' ]]"
probar "off espera un poco antes de borrar el fondo anterior" "[[ -s '$T/sleep.log' ]]"
otaku estado >"$T/salida"
probar "estado dice apagado" "grep -q 'Modo otaku: apagado' '$T/salida'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "apagar lo que ya está apagado no rompe nada" "((rc == 0)) && grep -q 'ya está apagado' '$T/salida'"
otaku otro >/dev/null; rc=$?
probar "«otro» con el modo apagado avisa y falla" "((rc == 1))"
otaku alternar >/dev/null
probar "alternar enciende" "[[ -f '$MARCA' ]]"
otaku alternar >/dev/null
probar "alternar apaga" "[[ ! -f '$MARCA' && ! -e '$FONDOS_ESP/otaku' ]]"

echo "== Sin red"
nueva_sesion
touch "$T/sin-red"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on funciona igual" "((rc == 0)) && [[ -f '$MARCA' ]]"
probar "usa el fondo original de respaldo (copia del repo)" "cmp -s '$RESP' '$FONDOS_ESP/otaku/otaku-respaldo.jpg'"
probar "lo dice en la marca y en fuentes.json" "grep -q '^fuente=respaldo\$' '$MARCA' && jq -e '.fondos[0].url == null' '$FONDOS_ESP/otaku/fuentes.json' >/dev/null"
probar "pone ese fondo con la IPC" "grep -q '^$CALL DP-1 $FONDOS_ESP/otaku/otaku-respaldo.jpg fade\$' '$T/ipc.log'"
probar "avisa de que no hubo red" "grep -q 'sin red' '$T/salida'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "off lo deja todo como estaba" "((rc == 0)) && [[ ! -e '$FONDOS_ESP/otaku' && \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]]"

echo "== La IPC del shell no responde (no hay Serpantinum)"
nueva_sesion
touch "$T/ipc-falla"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on no se rompe: sale bien y avisa" "((rc == 0)) && grep -q 'el shell no responde' '$T/salida'"
probar "igual guarda el estado y aplica los ajustes" "[[ -f '$MARCA' && -f '$ANTES' && \"\$(ajuste .theme.borderRadius)\" == 16 ]] && ((\$(numero_fondos) == 5))"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "off tampoco: devuelve los ajustes y dice cómo poner el fondo" \
  "((rc == 0)) && [[ \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]] && grep -q \"vortex fondo '$FONDOS_ESP/vortex-azul.jpg'\" '$T/salida'"
probar "off sin IPC conserva los archivos que el shell aún podría estar usando" "((\$(numero_fondos) == 5)) && [[ ! -e '$MARCA' && ! -e '$ANTES' ]]"
rm -f "$T/ipc-falla"

echo "== Ajustes sin «theme»"
nueva_sesion
printf '{\n  "general": {\n    "language": "es"\n  }\n}\n' >"$H/.config/serpantinum/settings.json"
cp "$H/.config/serpantinum/settings.json" "$T/settings-original.json"
otaku on >/dev/null
probar "on crea theme con la fuente y los bordes" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Google Sans Flex' && \"\$(ajuste .theme.borderRadius)\" == 16 ]]"
otaku off >/dev/null
probar "off deja la fuente y los bordes que el shell usa por defecto (borrar la clave no se notaría en un shell abierto)" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Adwaita Mono' && \"\$(ajuste .theme.borderRadius)\" == 8 && \"\$(ajuste .general.language)\" == es ]]"

echo "== Ajustes rotos o inexistentes"
nueva_sesion
printf '{ esto no es json' >"$H/.config/serpantinum/settings.json"
otaku on >/dev/null; rc=$?
probar "con settings.json roto: on funciona y no lo toca" "((rc == 0)) && [[ \"\$(cat '$H/.config/serpantinum/settings.json')\" == '{ esto no es json' ]]"
otaku off >/dev/null; rc=$?
probar "…y off también" "((rc == 0)) && [[ \"\$(cat '$H/.config/serpantinum/settings.json')\" == '{ esto no es json' && ! -e '$MARCA' ]]"
nueva_sesion
rm -f "$H/.config/serpantinum/settings.json"
otaku on >/dev/null; rc=$?
probar "sin settings.json: on funciona y no lo crea" "((rc == 0)) && [[ ! -e '$H/.config/serpantinum/settings.json' ]]"
otaku off >/dev/null; rc=$?
probar "…y off también" "((rc == 0)) && [[ ! -e '$MARCA' && ! -e '$FONDOS_ESP/otaku' ]]"

echo "== Sin fondo de antes ni Hyprland"
nueva_sesion
rm -f "$H/.cache/serpantinum/wallpaper/current_DP-1"
printf '#!/usr/bin/env bash\nexit 1\n' >"$T/bin/hyprctl"
otaku on >/dev/null
probar "sin Hyprland usa «all» en la IPC" "grep -q '^$CALL all ' '$T/ipc.log'"
otaku off >/dev/null
probar "sin fondo anterior, off pone el de Vortex" "grep -q '^$CALL all $FONDOS_ESP/vortex-azul.jpg fade\$' '$T/ipc.log'"

echo "== Cortado a medias"
nueva_sesion
mkdir -p "$H/.config/vortex"
echo '{"fondos":{"DP-1":"/de/antes.jpg"},"tema":null}' >"$ANTES"
otaku on >/dev/null
probar "si ya hay un «antes» sin marca (se cortó una pasada), se conserva" "[[ \"\$(jq -r '.fondos[\"DP-1\"]' '$ANTES')\" == /de/antes.jpg ]]"
otaku off >/dev/null

echo "== Dos a la vez"
nueva_sesion
mkdir -p "$H/.config/vortex"
flock "$H/.config/vortex/vortex-otaku.lock" sleep 3 &
PID_CANDADO=$!
sleep 0.5
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "con otro vortex-otaku trabajando, el segundo avisa y no hace nada" "((rc == 1)) && grep -q 'otro vortex-otaku' '$T/salida' && [[ ! -e '$MARCA' && ! -e '$FONDOS_ESP/otaku' && ! -s '$T/curl.log' ]]"
kill "$PID_CANDADO" 2>/dev/null
wait "$PID_CANDADO" 2>/dev/null

echo "== bin/vortex"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent "$AQUI/bin/vortex" otaku estado >"$T/salida" 2>&1
probar "«vortex otaku estado» llega al comando" "grep -q 'Modo otaku: apagado' '$T/salida'"
env HOME="$H" "$AQUI/bin/vortex" ayuda >"$T/salida" 2>&1
probar "la ayuda de vortex lo lista" "grep -q 'otaku' '$T/salida'"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
