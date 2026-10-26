#!/usr/bin/env bash
# Pruebas del modo otaku (bin/vortex-otaku), sin red, sin shell y sin tocar tu escritorio:
#   1. sintaxis (bash -n) y shellcheck si está instalado;
#   2. el jpg de respaldo del repo (original, 2560x1440);
#   3. el comando entero con HOME temporal y curl, quickshell, hyprctl, notify-send y sleep
#      falsos: encender con Wallhaven (respuesta de la API inventada), apagar, otro fondo,
#      sin red (fondo de respaldo), con la IPC del shell caída, ajustes sin «theme», ajustes
#      rotos o inexistentes, ajustes que no se pueden escribir al apagar, restos de una pasada
#      anterior y respuestas de la API que intentan mandarnos a otro servidor (también uno de
#      un dominio parecido, evilwallhaven.cc);
#   4. «vortex actualizar» deja los comandos de bin/ en el destino (uno de mentira, no /usr/local/bin).
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
for c in jq flock shuf od setsid; do
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
# El ícono de la app Miku (apps/miku/icono/vortex-miku.svg) es original: una nota musical, sin el personaje.
# Solo imágenes y modelos: el asistente Miku (apps/miku, packaging/vortex-miku, pruebas/miku*.sh...) lleva
# «miku» en el nombre de sus scripts. El modelo 3D (*.vrm) nunca va a git: pruebas/miku.sh.
probar "ninguna imagen del personaje en el repo (solo el respaldo original)" \
  "[[ -z \"\$(cd '$AQUI' && git ls-files | grep -i -E 'wallhaven|nakano|miku' | grep -i -E '\\.(jpe?g|png|webp|gif|bmp|avif|svg|vrm|glb|gltf|fbx|psd)\$' | grep -v -E '^(bin/vortex-otaku|pruebas/otaku.sh|shell/|apps/miku/icono/vortex-miku\\.svg\$)' || true)\" ]]"

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
    [[ -e "$T/curl-lento" ]] && /usr/bin/sleep 3 # (sleep de PATH es falso) para poder cortar la pasada a medias
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

# Respuesta de la API: 7 resultados buenos (uno, bbbbbb, falla al bajar; otro, gggggg, viene sin «url») y,
# ANTES de ellos para que el filtro tenga que pararlos (el tope de 5 no los salvaría): uno vertical, tres que
# no son de Wallhaven (otro servidor, un dominio que solo termina en «wallhaven.cc» y otro que lo trae de
# prefijo), uno con un nombre sospechoso y un gif.
python3 - "$T/api.json" <<'EOF'
import json, sys
def r(i, ext="jpg", w=1920, h=1080, host="w.wallhaven.cc", tipo=None, nombre=None, url=True):
    d = {"id": i, "path": f"https://{host}/full/{i[:2]}/{nombre or 'wallhaven-' + i + '.' + ext}",
         "dimension_x": w, "dimension_y": h, "file_type": tipo or ("image/png" if ext == "png" else "image/jpeg")}
    if url:
        d["url"] = f"https://wallhaven.cc/w/{i}"
    return d
datos = [r("hhhhhh", w=1080, h=1920), r("iiiiii", host="evil.example.com"), r("mmmmmm", host="evilwallhaven.cc"),
         r("nnnnnn", host="wallhaven.cc.evil.com"), r("jjjjjj", nombre="..%2f..%2fx.jpg"), r("kkkkkk", tipo="image/gif", ext="gif"),
         r("aaaaaa"), r("bbbbbb"), r("gggggg", url=False), r("cccccc", "png"), r("dddddd"), r("eeeeee", "png"), r("ffffff"),
         r("llllll")]
json.dump({"data": datos, "meta": {"total": len(datos)}}, open(sys.argv[1], "w"))
EOF

H="$T/home"
FONDOS_ESP="$H/Imágenes/Fondos"
OTAKU="$H/Imágenes/Fondos otaku"   # los fondos otaku van aparte (no salen en el selector normal)
# Arranca una «sesión» limpia: HOME nuevo, ajustes del shell y un fondo ya puesto en DP-1.
nueva_sesion() {
  rm -rf "$H"
  rm -f "$T/curl.log" "$T/ipc.log" "$T/notify.log" "$T/hyprctl.log" "$T/sleep.log" "$T/sin-red" "$T/ipc-falla" "$T/curl-lento"
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
    HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_OTAKU_CUANTOS=5 \
    VORTEX_SHELL_QML=/usr/share/serpantinum/src/quickshell/Shell.qml "$AQUI/bin/vortex-otaku" "$@" 2>&1
}
ajuste() { jq -r "$1" "$H/.config/serpantinum/settings.json"; }
numero_fondos() { find "$OTAKU" -maxdepth 1 -type f \( -name '*.jpg' -o -name '*.png' \) 2>/dev/null | wc -l; }
MARCA="$H/.config/vortex/otaku"
ANTES="$H/.config/vortex/otaku-antes.json"
CALL='ipc -p /usr/share/serpantinum/src/quickshell/Shell.qml call wallpaper setWallpaper'

echo "== Encender con Wallhaven"
nueva_sesion
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on termina bien" "((rc == 0))"
probar "queda la marca ~/.config/vortex/otaku" "[[ -f '$MARCA' ]]"
probar "la primera petición es la búsqueda SFW de las quintillizas (Go-Toubun no Hanayome)" \
  "[[ \"\$(head -n1 '$T/curl.log')\" == 'https://wallhaven.cc/api/v1/search?q=Go-Toubun%20no%20Hanayome&categories=010&purity=100&sorting=favorites&atleast=1920x1080' ]]"
probar "el selector de fondos apunta a los fondos otaku mientras está encendido" \
  "[[ \"\$(jq -r .wallpaperDir '$H/.config/serpantinum/settings.json')\" == '$OTAKU' ]]"
probar "baja 5 fondos a Imágenes/Fondos/otaku (con tilde y espacios en la ruta)" "((\$(numero_fondos) == 5))"
probar "los que fallan (bbbbbb) se saltan y se baja el siguiente" "[[ ! -e '$OTAKU/wallhaven-bbbbbb.jpg' ]] && [[ -e '$OTAKU/wallhaven-eeeeee.png' && ! -e '$OTAKU/wallhaven-ffffff.jpg' ]]"
probar "no pide nada fuera de wallhaven.cc (ni evilwallhaven.cc ni wallhaven.cc.evil.com) ni los verticales ni los gif" \
  "! grep -q -E 'evil|hhhhhh|kkkkkk|mmmmmm|nnnnnn|x\\.jpg|llllll' '$T/curl.log'"
probar "sí pide a los subdominios de wallhaven.cc (w.wallhaven.cc)" "grep -q '^https://w\\.wallhaven\\.cc/full/' '$T/curl.log'"
probar "fuentes.json guarda la URL de Wallhaven de cada fondo" \
  "[[ \"\$(jq -r '.fondos | map(.url | startswith(\"https://wallhaven.cc/w/\")) | all' '$OTAKU/fuentes.json')\" == true ]] && [[ \"\$(jq '.fondos | length' '$OTAKU/fuentes.json')\" == 5 ]]"
probar "el que no traía «url» se arma con su id" \
  "[[ \"\$(jq -r '.fondos[] | select(.id == \"gggggg\") | .url' '$OTAKU/fuentes.json')\" == https://wallhaven.cc/w/gggggg ]]"
probar "pone el fondo con la IPC del shell en cada monitor" \
  "grep -q -E '^$CALL DP-1 $OTAKU/wallhaven-[a-z]+\\.(jpg|png) fade\$' '$T/ipc.log' && grep -q -E '^$CALL HDMI-A-1 ' '$T/ipc.log'"
probar "guarda el fondo de antes y los ajustes de antes" \
  "[[ \"\$(jq -c . '$ANTES')\" == '{\"fondos\":{\"DP-1\":\"$FONDOS_ESP/vortex-azul.jpg\"},\"tema\":{\"fontFamily\":\"Adwaita Mono\",\"borderRadius\":8,\"wallpaperDir\":\"$FONDOS_ESP\"}}' ]]"
probar "ajustes: fuente y bordes nuevos" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Google Sans Flex' && \"\$(ajuste .theme.borderRadius)\" == 24 ]]"
probar "ajustes: lo demás sigue igual" \
  "[[ \"\$(ajuste .theme.activePreset)\" == Matugen && \"\$(ajuste .general.language)\" == es ]]"
probar "el settings.json sigue con los mismos permisos y sin temporales" \
  "[[ \"\$(stat -c %a '$H/.config/serpantinum/settings.json')\" == \"\$(stat -c %a '$T/settings-original.json')\" && -z \"\$(ls -A '$H/.config/serpantinum' | grep -v '^settings.json\$')\" ]]"
probar "avisa con una notificación" "grep -q 'Modo otaku activado' '$T/notify.log'"
otaku estado >"$T/salida"
probar "estado dice encendido y el fondo" "grep -q 'Modo otaku: encendido' '$T/salida' && grep -q 'Fondo: $OTAKU/' '$T/salida'"

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
probar "off deja los fondos otaku en su carpeta (para usarlos en el modo normal)" "[[ -f '$OTAKU/fuentes.json' ]] && ((\$(numero_fondos) == 5))"
probar "no toca los fondos del usuario" "[[ -f '$FONDOS_ESP/vortex-azul.jpg' && -f '$FONDOS_ESP/mi fondo (raro).jpg' ]]"
probar "off quita la marca y lo guardado" "[[ ! -e '$MARCA' && ! -e '$ANTES' ]]"
otaku estado >"$T/salida"
probar "estado dice apagado" "grep -q 'Modo otaku: apagado' '$T/salida'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "apagar lo que ya está apagado no rompe nada" "((rc == 0)) && grep -q 'ya está apagado' '$T/salida'"
otaku otro >/dev/null; rc=$?
probar "«otro» con el modo apagado avisa y falla" "((rc == 1))"
otaku alternar >/dev/null
probar "alternar enciende" "[[ -f '$MARCA' ]]"
otaku alternar >/dev/null
probar "alternar apaga" "[[ ! -f '$MARCA' ]]"
otaku borrar >/dev/null; rc=$?
probar "«borrar» quita lo que se bajó y la carpeta" "((rc == 0)) && [[ ! -e '$OTAKU' ]]"

echo "== Sin red"
nueva_sesion
touch "$T/sin-red"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on funciona igual" "((rc == 0)) && [[ -f '$MARCA' ]]"
probar "usa el fondo original de respaldo (copia del repo)" "cmp -s '$RESP' '$OTAKU/otaku-respaldo.jpg'"
probar "lo dice en la marca y en fuentes.json" "grep -q '^fuente=respaldo\$' '$MARCA' && jq -e '.fondos[0].url == null' '$OTAKU/fuentes.json' >/dev/null"
probar "pone ese fondo con la IPC" "grep -q '^$CALL DP-1 $OTAKU/otaku-respaldo.jpg fade\$' '$T/ipc.log'"
probar "avisa de que no hubo red" "grep -q 'sin red' '$T/salida'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "off lo deja todo como estaba" "((rc == 0)) && [[ \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]]"

echo "== La IPC del shell no responde (no hay Serpantinum)"
nueva_sesion
touch "$T/ipc-falla"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "on no se rompe: sale bien y avisa" "((rc == 0)) && grep -q 'el shell no responde' '$T/salida'"
probar "igual guarda el estado y aplica los ajustes" "[[ -f '$MARCA' && -f '$ANTES' && \"\$(ajuste .theme.borderRadius)\" == 24 ]] && ((\$(numero_fondos) == 5))"
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
probar "on crea theme con la fuente y los bordes" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Google Sans Flex' && \"\$(ajuste .theme.borderRadius)\" == 24 ]]"
otaku off >/dev/null
probar "off deja la fuente y los bordes que el shell usa por defecto (borrar la clave no se notaría en un shell abierto)" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Adwaita Mono' && \"\$(ajuste .theme.borderRadius)\" == 18 && \"\$(ajuste .general.language)\" == es ]]"

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
probar "…y off también" "((rc == 0)) && [[ ! -e '$MARCA' ]]"

echo "== Sin fondo de antes ni Hyprland"
nueva_sesion
rm -f "$H/.cache/serpantinum/wallpaper/current_DP-1"
printf '#!/usr/bin/env bash\nexit 1\n' >"$T/bin/hyprctl"
otaku on >/dev/null
probar "sin Hyprland usa «all» en la IPC" "grep -q '^$CALL all ' '$T/ipc.log'"
otaku off >/dev/null
probar "sin fondo anterior, off pone el de Vortex" "grep -q '^$CALL all $FONDOS_ESP/vortex-azul.jpg fade\$' '$T/ipc.log'"

echo "== Los ajustes no se pueden devolver al apagar"
nueva_sesion
otaku on >/dev/null
chmod 555 "$H/.config/serpantinum" # sin permiso para crear el temporal: jq/mktemp no pueden escribir
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
chmod 755 "$H/.config/serpantinum"
probar "off avisa de que los ajustes no volvieron" "((rc == 0)) && grep -q 'no pude devolver los ajustes' '$T/salida' && grep -q 'ajustes del shell no volvieron' '$T/salida'"
probar "…conserva lo de antes (el aviso promete ese archivo) y quita la marca" \
  "[[ -f '$ANTES' && ! -e '$MARCA' && \"\$(jq -r '.tema.fontFamily' '$ANTES')\" == 'Adwaita Mono' ]]"
probar "…y no promete nada falso: el fondo sí volvió" "grep -q '^$CALL DP-1 $FONDOS_ESP/vortex-azul.jpg fade\$' '$T/ipc.log'"
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "otro off lo reintenta con lo guardado y ahora sí lo deja todo como estaba" \
  "((rc == 0)) && [[ ! -e '$ANTES' && \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]] && grep -q 'todo volvió a como estaba' '$T/salida'"

echo "== Restos de una pasada anterior (off con el shell caído y luego on con otra lista)"
nueva_sesion
touch "$T/ipc-falla"
otaku on >/dev/null
otaku off >/dev/null
rm -f "$T/ipc-falla"
probar "el off con el shell caído deja los archivos (el shell podría estar usándolos)" "((\$(numero_fondos) == 5)) && [[ ! -e '$MARCA' ]]"
touch "$T/sin-red"
printf 'JPG-DEL-USUARIO-EN-OTAKU' >"$OTAKU/mio.jpg"
otaku on >/dev/null
probar "el on sin red usa los fondos otaku que ya tenía (no los cambia por el de respaldo)" \
  "[[ \"\$(find '$OTAKU' -maxdepth 1 -name 'wallhaven-*' | wc -l)\" == 5 && ! -f '$OTAKU/otaku-respaldo.jpg' ]]"
probar "…y no toca archivos que el comando no creó" "[[ -f '$OTAKU/mio.jpg' ]]"
otaku off >/dev/null
otaku borrar >/dev/null
probar "«borrar» quita todo lo suyo (queda solo lo ajeno)" \
  "[[ \"\$(ls -A '$OTAKU')\" == mio.jpg ]]"
rm -f "$OTAKU/mio.jpg"
nueva_sesion
mkdir -p "$OTAKU"
head -c 70000 /dev/urandom >"$OTAKU/wallhaven-zzzzzz.jpg"
printf '{"fondos":[{"archivo":"wallhaven-zzzzzz.jpg"}]}' >"$OTAKU/fuentes.json"
otaku on >/dev/null
probar "un resto que la lista nueva ya no menciona se poda; los de la lista se quedan" \
  "[[ ! -e '$OTAKU/wallhaven-zzzzzz.jpg' ]] && ((\$(numero_fondos) == 5))"
otaku off >/dev/null
otaku borrar >/dev/null
probar "…y «borrar» deja la carpeta limpia" "[[ ! -e '$OTAKU' ]]"

echo "== Se corta la pasada en plena descarga (Ajustes cerrado, apagón) y se vuelve a encender"
nueva_sesion
touch "$T/curl-lento"
(
  setsid env -u XDG_CONFIG_HOME -u XDG_CACHE_HOME -u XDG_STATE_HOME -u XDG_RUNTIME_DIR -u HYPRLAND_INSTANCE_SIGNATURE -u QS_SETTINGS \
    HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent "$AQUI/bin/vortex-otaku" on >/dev/null 2>&1 &
  echo $! >"$T/pid-corte"
)
for _ in $(seq 1 50); do
  compgen -G "$OTAKU/.*.parcial" >/dev/null && break
  /usr/bin/sleep 0.1
done
probar "la pasada va por la mitad (hay un .parcial y aún no hay marca)" "compgen -G '$OTAKU/.*.parcial' >/dev/null && [[ ! -e '$MARCA' ]]"
kill -9 -- "-$(cat "$T/pid-corte")" 2>/dev/null
/usr/bin/sleep 0.3
rm -f "$T/curl-lento"
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "el siguiente on se recupera: enciende, 5 fondos y su fuentes.json" \
  "((rc == 0)) && [[ -f '$MARCA' ]] && ((\$(numero_fondos) == 5)) && [[ \"\$(jq '.fondos | length' '$OTAKU/fuentes.json')\" == 5 ]] && ! compgen -G '$OTAKU/.*.parcial' >/dev/null"
otaku off >/dev/null; rc=$?
probar "…y el off deja todo como estaba" \
  "((rc == 0)) && [[ \"\$(jq -S . '$H/.config/serpantinum/settings.json')\" == \"\$(jq -S . '$T/settings-original.json')\" ]]"

echo "== Cortado a medias"
nueva_sesion
mkdir -p "$H/.config/vortex"
echo '{"fondos":{"DP-1":"/de/antes.jpg"},"tema":null}' >"$ANTES"
otaku on >/dev/null
probar "si ya hay un «antes» sin marca (se cortó una pasada), se conserva" "[[ \"\$(jq -r '.fondos[\"DP-1\"]' '$ANTES')\" == /de/antes.jpg ]]"
otaku off >/dev/null

echo "== Ajustes que se quedaron con los del modo otaku (el fallo hasta la 1.6.2)"
# La huella: carpeta otaku + fuente y bordes del modo, sin marca ni «antes» (un «on» viejo cortado).
envenenar() { jq --arg d "$OTAKU" '.wallpaperDir = $d | .theme.fontFamily = "Google Sans Flex" | .theme.borderRadius = 24' \
  "$T/settings-original.json" >"$H/.config/serpantinum/settings.json"; }
nueva_sesion
envenenar
salida="$(otaku off)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "off con el modo ya apagado devuelve la carpeta Fondos (los fondos otaku dejan de salir)" \
  "((rc == 0)) && [[ \"\$(ajuste .wallpaperDir)\" == '$FONDOS_ESP' ]]"
probar "…y la fuente y los bordes por defecto" "[[ \"\$(ajuste .theme.fontFamily)\" == 'Adwaita Mono' && \"\$(ajuste .theme.borderRadius)\" == 18 ]]"
probar "…sin tocar lo demás" "[[ \"\$(ajuste .theme.activePreset)\" == Matugen && \"\$(ajuste .general.language)\" == es ]]"
nueva_sesion
envenenar
otaku on >/dev/null
probar "on con los ajustes contaminados no los guarda como «los de antes»" \
  "[[ \"\$(jq -c .tema '$ANTES')\" == '{\"fontFamily\":null,\"borderRadius\":null,\"wallpaperDir\":\"$FONDOS_ESP\"}' ]]"
otaku off >/dev/null
probar "…y al apagar vuelve la carpeta Fondos" "[[ \"\$(ajuste .wallpaperDir)\" == '$FONDOS_ESP' && \"\$(ajuste .theme.fontFamily)\" == 'Adwaita Mono' ]]"
nueva_sesion
mkdir -p "$H/.config/vortex"
echo "{\"fondos\":{},\"tema\":{\"fontFamily\":\"Google Sans Flex\",\"borderRadius\":24,\"wallpaperDir\":\"$OTAKU\"}}" >"$ANTES"
touch "$MARCA"
envenenar
otaku off >/dev/null
probar "un «antes» viejo con la huella tampoco devuelve los fondos otaku" "[[ \"\$(ajuste .wallpaperDir)\" == '$FONDOS_ESP' && ! -e '$ANTES' ]]"
nueva_sesion
jq --arg d "$OTAKU" '.wallpaperDir = $d' "$T/settings-original.json" >"$H/.config/serpantinum/settings.json"
otaku reparar >/dev/null; rc=$?
probar "elegir a mano la carpeta otaku (sin la fuente ni los bordes del modo) se respeta" "((rc == 0)) && [[ \"\$(ajuste .wallpaperDir)\" == '$OTAKU' ]]"
nueva_sesion
envenenar
mkdir -p "$H/.config/vortex"; touch "$MARCA"
otaku reparar >/dev/null
probar "con el modo encendido «reparar» no toca nada" "[[ \"\$(ajuste .wallpaperDir)\" == '$OTAKU' ]]"
rm -f "$MARCA"
grep -q 'vortex-otaku" reparar' "$AQUI/bin/vortex"
probar "vortex actualizar corre «vortex-otaku reparar»" "(($? == 0))"

echo "== Dos a la vez"
nueva_sesion
mkdir -p "$H/.config/vortex"
flock "$H/.config/vortex/vortex-otaku.lock" sleep 3 &
PID_CANDADO=$!
sleep 0.5
salida="$(otaku on)"; rc=$?; printf '%s\n' "$salida" >"$T/salida"
probar "con otro vortex-otaku trabajando, el segundo avisa y no hace nada" "((rc == 1)) && grep -q 'otro vortex-otaku' '$T/salida' && [[ ! -e '$MARCA' && ! -e '$OTAKU' && ! -s '$T/curl.log' ]]"
kill "$PID_CANDADO" 2>/dev/null
wait "$PID_CANDADO" 2>/dev/null

echo "== bin/vortex"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent "$AQUI/bin/vortex" otaku estado >"$T/salida" 2>&1
probar "«vortex otaku estado» llega al comando" "grep -q 'Modo otaku: apagado' '$T/salida'"
env HOME="$H" "$AQUI/bin/vortex" ayuda >"$T/salida" 2>&1
probar "la ayuda de vortex lo lista" "grep -q 'otaku' '$T/salida'"

echo "== vortex actualizar deja los comandos al día"
export VORTEX_URL="$T/repo-que-no-existe"     # sin red: no se conecta a GitHub ni toca el repo de prueba
mkdir -p "$T/destino" "$T/repo/bin" "$T/repo/shell"
for c in vortex vortex-bench vortex-mcp vortex-gestor vortex-otaku; do
  cp "$AQUI/bin/$c" "$T/repo/bin/$c"
done
cp "$AQUI/install.sh" "$T/repo/install.sh"
cp "$AQUI/shell/version.txt" "$T/repo/shell/version.txt"
printf '#!/usr/bin/env bash\nexec "$@"\n' >"$T/bin/sudo"
printf '#!/usr/bin/env bash\necho "paru $*" >>"$T/paru.log"\n' >"$T/bin/paru"
printf '#!/usr/bin/env bash\necho "vortex-shell $(cat "$T/repo/shell/version.txt")-1"\n' >"$T/bin/pacman"
chmod +x "$T/bin/sudo" "$T/bin/paru" "$T/bin/pacman"
printf '#!/usr/bin/env bash\n# viejo\n' >"$T/destino/vortex"
chmod 755 "$T/destino/vortex"
salida="$(env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_BIN_DESTINO="$T/destino" "$T/repo/bin/vortex" actualizar 2>&1)"; rc=$?
printf '%s\n' "$salida" >"$T/salida"
probar "actualizar termina bien y sigue con los paquetes" "((rc == 0)) && grep -q 'paru -Syu' '$T/paru.log'"
probar "copia los comandos que faltaban (vortex-otaku) y reemplaza los viejos (vortex)" \
  "cmp -s '$AQUI/bin/vortex-otaku' '$T/destino/vortex-otaku' && cmp -s '$AQUI/bin/vortex' '$T/destino/vortex' && [[ -x '$T/destino/vortex-otaku' ]]"
mtime_antes="$(stat -c %Y.%y "$T/destino/vortex-otaku")"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_BIN_DESTINO="$T/destino" "$T/repo/bin/vortex" actualizar >/dev/null 2>&1
probar "si no cambiaron, no los vuelve a copiar (ni pide sudo)" "[[ \"\$(stat -c %Y.%y '$T/destino/vortex-otaku')\" == '$mtime_antes' ]]"

# Paquetes propios: se reconstruye solo el que falta o tiene otra versión que su PKGBUILD.
for p in vortex-shell vortex-shop vortex-miku; do
  mkdir -p "$T/repo/packaging/$p"
  cp "$AQUI/packaging/$p/PKGBUILD" "$T/repo/packaging/$p/PKGBUILD"
done
v_shell="$(awk -F= '/^pkgver=/ {v = $2} /^pkgrel=/ {r = $2} END {print v "-" r}' "$AQUI/packaging/vortex-shell/PKGBUILD")"
v_shop="$(awk -F= '/^pkgver=/ {v = $2} /^pkgrel=/ {r = $2} END {print v "-" r}' "$AQUI/packaging/vortex-shop/PKGBUILD")"
# shell al día, tienda con el pkgrel viejo, Miku sin instalar
cat >"$T/bin/pacman" <<EOF
#!/usr/bin/env bash
case "\$2" in
  vortex-shell) echo "vortex-shell $v_shell" ;;
  vortex-shop) echo "vortex-shop ${v_shop%-*}-0" ;;
  *) exit 1 ;;
esac
EOF
printf '#!/usr/bin/env bash\nbasename "$PWD" >>"$T/makepkg.log"\n' >"$T/bin/makepkg"
# En una línea: «actualizar» reinicia el shell con un «hyprctl eval» de varias líneas, y cada línea
# suelta se contaría como un paquete reconstruido (pasaba al correr la prueba dentro de Hyprland).
printf '#!/usr/bin/env bash\necho "hyprctl ${*//$'"'"'\\n'"'"'/ }" >>"$T/makepkg.log"\n' >"$T/bin/hyprctl"
chmod +x "$T/bin/pacman" "$T/bin/makepkg" "$T/bin/hyprctl"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_BIN_DESTINO="$T/destino" "$T/repo/bin/vortex" actualizar >/dev/null 2>&1; rc=$?
probar "reconstruye la tienda (otro pkgrel) e instala Miku (faltaba), no el shell al día" \
  "((rc == 0)) && [[ \"\$(grep -v hyprctl '$T/makepkg.log' | sort | tr '\n' ' ')\" == 'vortex-miku vortex-shop ' ]]"
: >"$T/makepkg.log"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_BIN_DESTINO="$T/destino" "$T/repo/bin/vortex" actualizar --forzar >/dev/null 2>&1
probar "con --forzar reconstruye los tres" "[[ \"\$(grep -vc hyprctl '$T/makepkg.log')\" == 3 ]]"

# La config de Hyprland que copió el instalador se pone al día (con respaldo); sin ella no se crea.
mkdir -p "$T/repo/config/hypr" "$H/.config/hypr"
cp "$AQUI/config/hypr/hyprland.lua" "$AQUI/config/hypr/atajos.lua" "$T/repo/config/hypr/"
echo '-- vieja' >"$H/.config/hypr/hyprland.lua"
rm -f "$H/.config/hypr/atajos.lua"
env HOME="$H" PATH="$T/bin:$PATH" T="$T" VORTEX_RELEASE=/nonexistent VORTEX_BIN_DESTINO="$T/destino" "$T/repo/bin/vortex" actualizar >/dev/null 2>&1
# shellcheck disable=SC2034  # rc lo lee probar (eval)
rc=$?
probar "pone al día ~/.config/hypr/hyprland.lua y respalda la vieja" \
  "((rc == 0)) && cmp -s '$AQUI/config/hypr/hyprland.lua' '$H/.config/hypr/hyprland.lua' && grep -rqx -- '-- vieja' '$H/.config/vortex/'respaldo-*/.config/hypr/hyprland.lua"
probar "no crea atajos.lua si no lo tenías" "[[ ! -e '$H/.config/hypr/atajos.lua' ]]"

lista_instalador="$(sed -n 's/^for comando in \(.*\); do$/\1/p' "$AQUI/install/30-escritorio.sh")"
lista_vortex="$(sed -n 's/^COMANDOS_VORTEX=(\(.*\))$/\1/p' "$AQUI/bin/vortex")"
probar "la lista de bin/vortex es la misma que la de install/30-escritorio.sh" "[[ -n '$lista_instalador' && '$lista_instalador' == '$lista_vortex' ]]"

echo "== El sonido al encender (vortex otaku sonido)"
if command -v ffmpeg >/dev/null && command -v ffprobe >/dev/null; then
  cat >"$T/bin/pw-play" <<FALSO
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$T/pw-play.log"
FALSO
  chmod +x "$T/bin/pw-play"
  rm -f "$T/pw-play.log"
  ffmpeg -v error -nostdin -y -f lavfi -i "sine=frequency=440:duration=40" "$T/canción de prueba.mp3"
  SONIDO_OTAKU="$H/.local/state/vortex/sonidos/otaku.ogg"
  duracion() { ffprobe -v error -show_entries format=duration -of csv=p=0 "$SONIDO_OTAKU" 2>/dev/null | cut -d. -f1; }
  sonidos() { if [[ -f "$T/pw-play.log" ]]; then wc -l <"$T/pw-play.log"; else echo 0; fi; }
  esperar_sonido() { for _ in $(seq 20); do [[ "$(sonidos)" -ge "$1" ]] && return; sleep .1; done; }
  nueva_sesion
  # shellcheck disable=SC2034  # rc lo lee probar (eval)
  otaku sonido >/dev/null; rc=$?
  probar "«sonido» sin archivo dice cómo se usa" "((rc == 2))"
  otaku sonido "$T/canción de prueba.mp3" abc >/dev/null; rc=$?
  probar "un segundo inicial que no es número se rechaza" "((rc == 2)) && [[ ! -e '$SONIDO_OTAKU' ]]"
  otaku sonido "$T/no-existe.mp3" >/dev/null; rc=$?
  probar "un archivo que no existe se rechaza" "((rc == 1))"
  otaku sonido "$T/canción de prueba.mp3" >/dev/null; rc=$?
  probar "guarda solo 15 s del audio (con espacios y tilde en el nombre)" "((rc == 0)) && [[ \"\$(duracion)\" == 15 || \"\$(duracion)\" == 14 ]]"
  otaku sonido "$T/canción de prueba.mp3" 30 >/dev/null; rc=$?
  probar "desde el segundo 30 de 40 quedan 10 s" "((${rc} == 0)) && [[ \"\$(duracion)\" == 10 || \"\$(duracion)\" == 9 ]]"
  otaku sonido "$T/canción de prueba.mp3" 2 >/dev/null
  otaku on >/dev/null
  esperar_sonido 1
  probar "al encender suena ese audio" "[[ \"\$(sonidos)\" == 1 ]] && grep -qF '$SONIDO_OTAKU' '$T/pw-play.log'"
  otaku on >/dev/null
  sleep .3
  probar "un segundo «on» (ya encendido) no suena otra vez" "[[ \"\$(sonidos)\" == 1 ]]"
  otaku off >/dev/null
  sleep .3
  probar "al apagar no suena" "[[ \"\$(sonidos)\" == 1 ]]"
  otaku sonido off >/dev/null
  otaku on >/dev/null
  sleep .3
  probar "con «sonido off» ya no suena al encender" "[[ ! -e '$SONIDO_OTAKU' && \"\$(sonidos)\" == 1 ]]"
  otaku off >/dev/null
else
  echo "  (sin ffmpeg: se saltan)"
fi

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
