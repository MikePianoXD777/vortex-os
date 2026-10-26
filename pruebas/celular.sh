#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de «Tu celular» (1.8): celular.sh (lo que lee el panel) y «vortex celular on|off», con
# kdeconnect-cli, busctl, pgrep, ufw y compañía falsos en un PATH temporal y un HOME temporal. No
# arranca KDE Connect ni toca el cortafuegos de verdad.
#
#   pruebas/celular.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$AQUI/shell/src/quickshell/celular/celular.sh"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-celular-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home/.config"
# kdeconnectd «corre» si existe $T/corriendo.
cat >"$T/bin/pgrep" <<FIN
#!/bin/sh
[ -e "$T/corriendo" ]
FIN
cat >"$T/bin/kdeconnect-cli" <<FIN
#!/bin/sh
echo "kdeconnect-cli \$*" >>"$T/registro"
case "\$*" in
  "-l --id-only") printf 'abc123\nxyz789\n' ;;
  "-a --id-only") printf 'abc123\nold_1\n' ;;
esac
exit 0
FIN
# Tres celulares: abc123 vinculado y conectado (KDE Connect 24+, con batería), xyz789 que pide
# vincularse, y old_1 de un KDE Connect viejo (sin pairState), vinculado pero lejos.
cat >"$T/bin/busctl" <<'FIN'
#!/bin/sh
ruta="$4"; nombre="$6"
case "${ruta##*/devices/}:$nombre" in
  abc123:name) echo 's "Pixel 7"' ;;
  abc123:type) echo 's "smartphone"' ;;
  abc123:isReachable) echo 'b true' ;;
  abc123:pairState) echo 'i 3' ;;
  abc123/battery:charge) echo 'i 15' ;;
  abc123/battery:isCharging) echo 'b false' ;;
  xyz789:name) echo 's "Tablet de Ana"' ;;
  xyz789:type) echo 's "tablet"' ;;
  xyz789:isReachable) echo 'b true' ;;
  xyz789:pairState) echo 'i 2' ;;
  old_1:name) echo 's "Moto G"' ;;
  old_1:isReachable) echo 'b false' ;;
  old_1:isPaired) echo 'b true' ;;
  old_1:isPairRequestedByPeer) echo 'b false' ;;
  *) echo "Failed to get property" >&2; exit 1 ;;
esac
FIN
printf '#!/bin/sh\nprintf "%%s\\n" "%s/foto.jpg" "%s/no-existe.txt"\n' "$T" "$T" >"$T/bin/zenity"
printf '#!/bin/sh\necho "notify-send $*" >>"%s/registro"\n' "$T" >"$T/bin/notify-send"
printf '#!/bin/sh\nexit 0\n' >"$T/bin/systemctl"
printf '#!/bin/sh\necho "ufw $*" >>"%s/registro"\n' "$T" >"$T/bin/ufw"
printf '#!/bin/sh\necho "pkexec $*" >>"%s/registro"\n' "$T" >"$T/bin/pkexec"
printf '#!/bin/sh\necho kdeconnectd >>"%s/registro"\n' "$T" >"$T/bin/kdeconnectd"
printf '#!/bin/sh\necho "pkill $*" >>"%s/registro"\n' "$T" >"$T/bin/pkill"
chmod +x "$T/bin/"*
touch "$T/foto.jpg"
celular() { PATH="$T/bin:$PATH" bash "$SCRIPT" "$@"; }
vortex() {
  HOME="$T/home" XDG_CONFIG_HOME="$T/home/.config" XDG_STATE_HOME="$T/home/.local/state" \
    PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@"
}

echo "== celular.sh"
salida="$(celular estado)"
[[ "$salida" == '{"activo": false, "dispositivos": []}' ]] && bien "apagado: activo false y sin celulares" || mal "apagado: $salida"

touch "$T/corriendo"
salida="$(celular estado)"
jq -e . >/dev/null <<<"$salida" && bien "encendido: JSON válido" || mal "JSON: $salida"
[[ "$(jq -r '[.dispositivos[].id] | join(",")' <<<"$salida")" == "old_1,abc123,xyz789" ]] &&
  bien "los vinculados primero, por nombre, sin repetir" || mal "orden: $(jq -c '[.dispositivos[].id]' <<<"$salida")"
[[ "$(jq -c '.dispositivos[] | select(.id == "abc123") | [.nombre, .alcanzable, .vinculado, .pide, .bateria, .cargando]' <<<"$salida")" == '["Pixel 7",true,true,false,15,false]' ]] &&
  bien "Pixel 7: vinculado, conectado, 15 % sin cargar" || mal "abc123: $(jq -c '.dispositivos[] | select(.id == "abc123")' <<<"$salida")"
[[ "$(jq -c '.dispositivos[] | select(.id == "xyz789") | [.tipo, .vinculado, .pide, .bateria]' <<<"$salida")" == '["tablet",false,true,-1]' ]] &&
  bien "la tablet pide vincularse (sin batería: -1)" || mal "xyz789: $(jq -c '.dispositivos[] | select(.id == "xyz789")' <<<"$salida")"
[[ "$(jq -c '.dispositivos[] | select(.id == "old_1") | [.tipo, .alcanzable, .vinculado]' <<<"$salida")" == '["smartphone",false,true]' ]] &&
  bien "KDE Connect viejo (isPaired) y sin tipo: celular" || mal "old_1: $(jq -c '.dispositivos[] | select(.id == "old_1")' <<<"$salida")"

: >"$T/registro"
celular sonar abc123; celular vincular xyz789; celular portapapeles abc123; celular desvincular old_1; celular buscar
[[ "$(grep -c kdeconnect-cli "$T/registro")" == 5 ]] &&
  grep -q -- "-d abc123 --ring" "$T/registro" && grep -q -- "-d xyz789 --pair" "$T/registro" &&
  grep -q -- "-d abc123 --send-clipboard" "$T/registro" && grep -q -- "-d old_1 --unpair" "$T/registro" &&
  grep -q -- "--refresh" "$T/registro" && bien "sonar, vincular, portapapeles, desvincular y buscar" || mal "acciones: $(cat "$T/registro")"

: >"$T/registro"
celular archivo abc123
grep -q -- "--share $T/foto.jpg" "$T/registro" && ! grep -q "no-existe" "$T/registro" &&
  grep -q "Enviando 1 archivo" "$T/registro" && bien "mandar archivos: solo los que existen, con aviso" || mal "archivo: $(cat "$T/registro")"

: >"$T/registro"
celular sonar 'x; rm -rf /' 2>/dev/null
[[ $? == 2 && ! -s "$T/registro" ]] && bien "un id raro no llega a kdeconnect-cli" || mal "id raro: $(cat "$T/registro")"

echo "== vortex celular"
rm -f "$T/corriendo"
[[ "$(vortex celular estado)" == apagado ]] && bien "de fábrica: apagado" || mal "estado: $(vortex celular estado)"
: >"$T/registro"
vortex celular on; sleep 0.3
[[ -e "$T/home/.config/vortex/celular" ]] && bien "on: queda marcado para el próximo inicio" || mal "sin marca"
grep -q "pkexec ufw allow kdeconnect" "$T/registro" && bien "abre los puertos en el cortafuegos" || mal "ufw: $(cat "$T/registro")"
grep -q "^kdeconnectd" "$T/registro" && bien "arranca kdeconnectd" || mal "no arrancó kdeconnectd"
: >"$T/registro"; touch "$T/corriendo"
vortex celular on; sleep 0.3
[[ ! -s "$T/registro" ]] && bien "un segundo «on» no repite el cortafuegos ni otro kdeconnectd" || mal "segundo on: $(cat "$T/registro")"
[[ "$(vortex celular estado)" == encendido ]] && bien "estado: encendido" || mal "estado tras on"
: >"$T/registro"
vortex celular off
[[ ! -e "$T/home/.config/vortex/celular" ]] && grep -q "pkill .*kdeconnectd" "$T/registro" &&
  bien "off: quita la marca y cierra kdeconnectd" || mal "off: $(cat "$T/registro")"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
