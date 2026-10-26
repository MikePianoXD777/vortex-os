#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de vortex-arreglar (1.9, «Algo no funciona») con un equipo falso: pactl, nmcli, systemctl,
# timedatectl, df, pgrep, rfkill y pkexec de mentira en un PATH temporal, un /sys falso (VORTEX_SYS)
# y el candado de pacman en una raíz falsa (VORTEX_RAIZ). No toca el sonido, la red ni nada de verdad.
#
#   pruebas/arreglar.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-arreglar-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/home" "$T/sys/class/bluetooth/hci0" "$T/sys/class/ieee80211/phy0" \
  "$T/sys/class/rfkill/rfkill0" "$T/sys/class/rfkill/rfkill1" "$T/raiz/var/lib/pacman"
echo bluetooth >"$T/sys/class/rfkill/rfkill0/type"
echo wlan >"$T/sys/class/rfkill/rfkill1/type"

# El estado del equipo falso: variables que leen los programas de mentira.
normal() {
  cat >"$T/estado" <<'FIN'
PACTL_OK=1 SINK=alsa_output.pci MUTE=no VOL=80 SRC=alsa_input.usb SRC_MUTE=no SRC_VOL=100
RED=enabled CONEX=full DISPOSITIVO="ethernet:connected:Cable" WIFI=enabled
BT_ACTIVO=1 USO=40 NTP=yes PACMAN_CORRE=0 FALLAS=""
FIN
  echo 0 >"$T/sys/class/rfkill/rfkill0/soft"; echo 0 >"$T/sys/class/rfkill/rfkill0/hard"
  echo 0 >"$T/sys/class/rfkill/rfkill1/soft"; echo 0 >"$T/sys/class/rfkill/rfkill1/hard"
  rm -f "$T/raiz/var/lib/pacman/db.lck"
}
cambiar() { echo "$*" >>"$T/estado"; }

falso() { # NOMBRE CUERPO
  printf '#!/usr/bin/env bash\n. "%s/estado"\necho "%s $*" >>"%s/registro"\n%s\n' "$T" "$1" "$T" "$2" >"$T/bin/$1"
  chmod +x "$T/bin/$1"
}
falso pactl '
case "$1" in
  info) ((PACTL_OK)) ;;
  get-default-sink) echo "$SINK" ;;
  get-default-source) echo "$SRC" ;;
  get-sink-mute) echo "Mute: $MUTE" ;;
  get-source-mute) echo "Mute: $SRC_MUTE" ;;
  get-sink-volume) echo "Volume: front-left: 1 /  $VOL% / 0 dB" ;;
  get-source-volume) echo "Volume: front-left: 1 /  $SRC_VOL% / 0 dB" ;;
  -f) echo "[{\"name\": \"$SINK\", \"description\": \"Bocinas de prueba\"}]" ;;
esac'
falso nmcli '
case "$*" in
  networking) echo "$RED" ;;
  "networking connectivity check") echo "$CONEX" ;;
  "-t -f TYPE,STATE,CONNECTION device") echo "$DISPOSITIVO" ;;
  "radio wifi") echo "$WIFI" ;;
esac'
falso systemctl '
case "$*" in
  "is-active --quiet bluetooth") ((BT_ACTIVO)); exit ;;
  "--failed --no-legend --plain") [[ -n "$FALLAS" ]] && echo "$FALLAS loaded failed failed algo" ;;
esac
exit 0'
falso timedatectl 'echo "$NTP"'
falso df '[[ "$*" == *--output=avail* ]] && { echo Avail; echo 1000000; exit 0; }
echo "Filesystem 1024-blocks Used Available Capacity Mounted"; echo "/dev/x 100000000 1 2097152 $USO% /"'
falso pgrep '((PACMAN_CORRE))'
falso rfkill 'exit 0'
falso pkexec 'exit 0'
falso flatpak 'exit 0'
falso sleep 'exit 0'

arreglar() {
  HOME="$T/home" XDG_CACHE_HOME="$T/home/.cache" VORTEX_SYS="$T/sys" VORTEX_RAIZ="$T/raiz" PATH="$T/bin:$PATH" \
    bash "$AQUI/bin/vortex-arreglar" "$@"
}
estado_de() { arreglar revisar | jq -r --arg id "$1" '.[] | select(.id == $id) | "\(.estado) \(.arreglo)"'; }

echo "== vortex-arreglar revisar"
normal
salida="$(arreglar revisar)"
jq -e 'type == "array"' >/dev/null <<<"$salida" && bien "JSON válido" || mal "JSON: $salida"
[[ "$(jq -r '[.[] | select(.estado != "bien")] | length' <<<"$salida")" == 0 ]] &&
  bien "un equipo sano: todo bien ($(jq -r '[.[].id] | join(", ")' <<<"$salida"))" || mal "algo salió mal: $salida"
[[ "$(jq -r '.[] | select(.id == "sonido") | .detalle' <<<"$salida")" == *"Bocinas de prueba"* ]] &&
  bien "dice por dónde sale el sonido" || mal "detalle del sonido"

normal; cambiar MUTE=yes
[[ "$(estado_de sonido)" == "mal sonido" ]] && bien "sonido silenciado → Arreglar" || mal "silenciado: $(estado_de sonido)"
normal; cambiar VOL=0
[[ "$(estado_de sonido)" == "mal sonido" ]] && bien "volumen en 0 → Arreglar" || mal "volumen 0"
normal; cambiar PACTL_OK=0
[[ "$(estado_de sonido)" == "mal sonido" ]] && bien "PipeWire caído → Arreglar" || mal "pipewire caído"
normal; cambiar SINK=auto_null
[[ "$(estado_de sonido)" == "mal sonido" ]] && bien "sin salida de sonido (auto_null) → Arreglar" || mal "auto_null"
normal; cambiar SRC=alsa_output.pci.monitor
[[ "$(estado_de microfono)" == "aviso " ]] && bien "sin micrófono: solo aviso" || mal "sin micrófono: $(estado_de microfono)"
normal; cambiar SRC_MUTE=yes
[[ "$(estado_de microfono)" == "mal microfono" ]] && bien "micrófono silenciado → Arreglar" || mal "micrófono"

normal; cambiar RED=disabled
[[ "$(estado_de internet)" == "mal red" ]] && bien "red apagada → Arreglar" || mal "red apagada: $(estado_de internet)"
normal; cambiar CONEX=limited
[[ "$(estado_de internet)" == "mal red" ]] && bien "conectado sin internet → Arreglar (reconecta)" || mal "limited"
normal; cambiar CONEX=portal
[[ "$(estado_de internet)" == "aviso portal" ]] && bien "red con inicio de sesión → Abrir" || mal "portal"
normal; cambiar CONEX=none DISPOSITIVO="wifi:disconnected:" WIFI=disabled
[[ "$(estado_de internet)" == "mal wifi" ]] && bien "Wi-Fi apagado → Arreglar" || mal "wifi apagado: $(estado_de internet)"
normal; cambiar CONEX=none DISPOSITIVO="wifi:disconnected:"; echo 1 >"$T/sys/class/rfkill/rfkill1/hard"
[[ "$(estado_de internet)" == "mal " ]] && bien "Wi-Fi apagado con la tecla: lo explica (no se puede desde aquí)" || mal "hard block: $(estado_de internet)"
normal; cambiar CONEX=none DISPOSITIVO="wifi:disconnected:"
[[ "$(estado_de internet)" == "mal elegir-red" ]] && bien "sin red elegida → Abrir el panel de red" || mal "sin red: $(estado_de internet)"
normal; cambiar CONEX=none DISPOSITIVO="wifi:unavailable:" WIFI=disabled
echo 1 >"$T/sys/class/rfkill/rfkill1/soft"; cambiar 'DISPOSITIVO="ethernet:connected:Cable"' CONEX=full
[[ "$(estado_de internet)" == "bien " ]] && bien "con cable e internet, el Wi-Fi apagado no es problema" || mal "cable: $(estado_de internet)"

normal; echo 1 >"$T/sys/class/rfkill/rfkill0/soft"
[[ "$(estado_de bluetooth)" == "mal bluetooth" ]] && bien "Bluetooth apagado → Arreglar" || mal "bt soft: $(estado_de bluetooth)"
normal; cambiar BT_ACTIVO=0
[[ "$(estado_de bluetooth)" == "mal bluetooth" ]] && bien "servicio de Bluetooth apagado → Arreglar" || mal "bt servicio"
normal; rm -rf "$T/sys/class/bluetooth/hci0"
[[ -z "$(estado_de bluetooth)" ]] && bien "sin Bluetooth en el equipo: no se muestra" || mal "sin bt: $(estado_de bluetooth)"
mkdir -p "$T/sys/class/bluetooth/hci0"

normal; cambiar USO=96
[[ "$(estado_de disco)" == "mal disco" ]] && bien "disco al 96 % → Arreglar" || mal "disco: $(estado_de disco)"
normal; cambiar USO=91
[[ "$(estado_de disco)" == "aviso disco" ]] && bien "disco al 91 %: aviso" || mal "disco 91"
normal; cambiar NTP=no
[[ "$(estado_de hora)" == "mal hora" ]] && bien "hora que no se pone sola → Arreglar" || mal "hora"
normal; touch "$T/raiz/var/lib/pacman/db.lck"
[[ "$(estado_de pacman)" == "mal pacman" ]] && bien "actualización trabada → Arreglar" || mal "candado: $(estado_de pacman)"
cambiar PACMAN_CORRE=1
[[ "$(estado_de pacman)" == "aviso " ]] && bien "con pacman corriendo: solo «espera»" || mal "pacman corriendo"
normal; cambiar 'FALLAS=cups.service'
[[ "$(arreglar revisar | jq -r '.[] | select(.id == "servicios") | "\(.estado) \(.detalle)"')" == "aviso Fallaron al arrancar: cups."* ]] &&
  bien "servicios que fallaron: aviso con su nombre" || mal "servicios"

echo "== vortex-arreglar arreglar"
hizo() { : >"$T/registro"; arreglar arreglar "$1" >/dev/null 2>&1; grep -q -- "$2" "$T/registro"; }
normal; cambiar MUTE=yes VOL=0
hizo sonido "pactl set-sink-mute @DEFAULT_SINK@ 0" && grep -q "pactl set-sink-volume @DEFAULT_SINK@ 50%" "$T/registro" &&
  ! grep -q "systemctl --user restart" "$T/registro" && bien "sonido: quita el silencio y sube a 50 % (sin reiniciar PipeWire)" || mal "arreglar sonido: $(cat "$T/registro")"
normal; cambiar PACTL_OK=0
hizo sonido "systemctl --user restart wireplumber pipewire pipewire-pulse" && bien "sonido caído: reinicia PipeWire" || mal "reinicio pipewire"
normal; hizo microfono "pactl set-source-mute @DEFAULT_SOURCE@ 0" && bien "micrófono: quita el silencio" || mal "micrófono"
normal; hizo red "nmcli networking on" && bien "red: la apaga y la prende" || mal "red"
normal; echo 1 >"$T/sys/class/rfkill/rfkill1/soft"
hizo wifi "nmcli radio wifi on" && grep -q "rfkill unblock wlan" "$T/registro" && bien "Wi-Fi: lo desbloquea y lo enciende" || mal "wifi: $(cat "$T/registro")"
normal; cambiar BT_ACTIVO=0
hizo bluetooth "pkexec systemctl enable --now bluetooth" && bien "Bluetooth: enciende el servicio (con pkexec)" || mal "bluetooth"
normal; hizo hora "pkexec timedatectl set-ntp true" && bien "hora: la pone automática (con pkexec)" || mal "hora"
normal; touch "$T/raiz/var/lib/pacman/db.lck"
hizo pacman "pkexec rm -f $T/raiz/var/lib/pacman/db.lck" && bien "actualización trabada: quita el candado (con pkexec)" || mal "candado"
cambiar PACMAN_CORRE=1
hizo pacman "pkexec" && mal "quitó el candado con pacman corriendo" || bien "con pacman corriendo no toca el candado"
normal; mkdir -p "$T/home/.cache/thumbnails/normal"
hizo disco "pkexec sh -c" && [[ ! -e "$T/home/.cache/thumbnails" ]] && grep -q "flatpak uninstall --unused" "$T/registro" &&
  bien "disco: miniaturas, Flatpak sin usar y caché de pacman" || mal "disco: $(cat "$T/registro")"
arreglar arreglar inventado >/dev/null 2>&1; [[ $? == 2 ]] && bien "un arreglo que no existe: error" || mal "arreglo inventado"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
