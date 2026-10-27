#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de vortex-onedrive (1.10) sin cuenta de Microsoft ni internet: rclone, pkexec, pacman,
# systemctl, mountpoint, fusermount3, notify-send y xdg-open falsos en un PATH temporal y un HOME
# temporal. El rclone falso «inicia sesión» escribiendo una config con token.
#
#   pruebas/onedrive.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-onedrive-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

H="$T/home"
mkdir -p "$T/bin" "$T/disponible" "$H/.config/rclone"
echo "[mio]" >"$H/.config/rclone/rclone.conf"
registro() { printf '#!/usr/bin/env bash\necho "%s $*" >>"%s/registro"\n%s\n' "$1" "$T" "${2:-}" >"$T/bin/$1"; chmod +x "$T/bin/$1"; }

# rclone falso: «config create» escribe la cuenta (o falla si existe $T/login-falla); «about» da el espacio.
cat >"$T/disponible/rclone" <<FIN
#!/usr/bin/env bash
echo "rclone \$*" >>"$T/registro"
conf="\$2"
case "\$3 \$4" in
  "config create")
    [[ -e "$T/login-falla" ]] && exit 1
    printf '[onedrive]\ntype = onedrive\ntoken = {"access_token":"x"}\ndrive_id = abc\ndrive_type = personal\n' >"\$conf" ;;
  "about onedrive:") echo '{"total": 1099511627776, "used": 53687091200, "free": 1045824536576}' ;;
esac
FIN
chmod +x "$T/disponible/rclone"
registro pkexec '[[ "$*" == *rclone* ]] && cp "'"$T"'/disponible/rclone" "'"$T"'/bin/rclone"; exit 0'
# systemctl falso: «restart» de la unidad «monta» la carpeta (marca $T/montado); «disable --now» la desmonta.
registro systemctl 'case "$*" in *"restart vortex-onedrive.service"*) touch "'"$T"'/montado" ;; *"disable --now"*) rm -f "'"$T"'/montado" ;; esac; exit 0'
registro mountpoint '[[ -e "'"$T"'/montado" ]]'
registro fusermount3 'rm -f "'"$T"'/montado"'
registro notify-send ''
registro xdg-open ''
registro sleep ''

od() {
  HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_STATE_HOME="$H/.local/state" XDG_CACHE_HOME="$H/.cache" \
    PATH="$T/bin:/usr/bin:/bin" bash "$AQUI/bin/vortex-onedrive" "$@"
}
campo() { od estado | jq -r "$1"; }
UNIDAD="$H/.config/systemd/user/vortex-onedrive.service"
RC="$H/.config/vortex/onedrive/rclone.conf"

echo "== vortex-onedrive"
[[ "$(campo '"\(.rclone) \(.conectado) \(.montado) \(.conectando)"')" == "false false false false" ]] &&
  bien "de fábrica: sin rclone ni cuenta" || mal "estado inicial: $(od estado)"

: >"$T/registro"
od conectar
grep -q "pkexec pacman -S --needed --noconfirm rclone" "$T/registro" && bien "instala rclone si falta (con pkexec)" || mal "instalar: $(cat "$T/registro")"
grep -q "rclone --config $RC config create onedrive onedrive config_is_local=true" "$T/registro" &&
  bien "inicia sesión en el navegador (config aparte, no la de rclone de la persona)" || mal "login: $(grep 'rclone' "$T/registro")"
[[ "$(stat -c %a "$RC" 2>/dev/null)" == 600 ]] && bien "la cuenta queda en un archivo 0600" || mal "permisos: $(stat -c %a "$RC" 2>&1)"
[[ "$(cat "$H/.config/rclone/rclone.conf")" == "[mio]" ]] && bien "no toca ~/.config/rclone/rclone.conf" || mal "tocó el rclone de la persona"
grep -q "^ExecStart=/usr/bin/rclone mount onedrive: \"$H/OneDrive\" --config \"$RC\"" "$UNIDAD" 2>/dev/null &&
  grep -q -- "--vfs-cache-mode full" "$UNIDAD" && grep -q "^WantedBy=default.target" "$UNIDAD" &&
  bien "unidad de usuario: monta ~/OneDrive con caché, en cada inicio de sesión" || mal "unidad: $(cat "$UNIDAD" 2>&1)"
grep -q "systemctl --user enable vortex-onedrive.service" "$T/registro" && grep -q "systemctl --user restart vortex-onedrive.service" "$T/registro" &&
  bien "la enciende" || mal "systemctl: $(grep systemctl "$T/registro")"
grep -qxF "file://$H/OneDrive OneDrive" "$H/.config/gtk-3.0/bookmarks" 2>/dev/null && bien "aparece en el panel lateral de Thunar" || mal "marcador"
sleep 0.3
[[ "$(campo '"\(.conectado) \(.montado) \(.conectando) \(.tipo) \(.usado) \(.total)"')" == "true true false personal 53687091200 1099511627776" ]] &&
  bien "estado: conectado, montado, cuenta personal y el espacio" || mal "estado: $(od estado)"

: >"$T/registro"
od conectar
! grep -q "config create" "$T/registro" && ! grep -q pkexec "$T/registro" && bien "otra vez «conectar»: no vuelve a pedir la sesión ni a instalar" || mal "reconectar: $(cat "$T/registro")"

echo "== desconectar"
: >"$T/registro"
od desconectar
grep -q "systemctl --user disable --now vortex-onedrive.service" "$T/registro" && [[ ! -e "$UNIDAD" ]] &&
  bien "apaga y borra la unidad" || mal "unidad al desconectar"
[[ ! -e "$RC" ]] && bien "olvida la cuenta" || mal "quedó la cuenta"
[[ ! -d "$H/OneDrive" ]] && bien "quita la carpeta vacía" || mal "quedó la carpeta"
! grep -q OneDrive "$H/.config/gtk-3.0/bookmarks" && bien "y el acceso de Thunar" || mal "marcador al desconectar"
[[ "$(campo '"\(.conectado) \(.montado)"')" == "false false" ]] && bien "estado: sin conectar" || mal "estado tras desconectar"

echo "== errores"
touch "$T/login-falla"
od conectar 2>/dev/null; rc=$?
((rc != 0)) && [[ "$(campo .error)" == "No se completó el inicio de sesión"* ]] && [[ "$(campo .conectando)" == false ]] &&
  bien "si no termina de iniciar sesión: lo dice y no se queda «conectando»" || mal "login fallido: $(od estado)"
[[ ! -e "$UNIDAD" ]] && bien "y no monta nada" || mal "montó sin cuenta"
rm -f "$T/login-falla"
mkdir -p "$H/OneDrive" && echo hola >"$H/OneDrive/mio.txt"
od conectar 2>/dev/null
[[ "$(campo .error)" == *"Ya tienes una carpeta"* && "$(cat "$H/OneDrive/mio.txt")" == hola && ! -e "$T/montado" ]] &&
  bien "no monta encima de una carpeta OneDrive con archivos" || mal "carpeta ocupada: $(od estado)"
rm -rf "$H/OneDrive"
od conectar 2>/dev/null
[[ "$(campo '"\(.montado) \(.error)"')" == "true " ]] && bien "al volver a intentar, se borra el error" || mal "error viejo: $(od estado)"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
