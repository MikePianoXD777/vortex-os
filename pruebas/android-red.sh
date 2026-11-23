#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de la red de Android con ufw (4.3): CachyOS trae ufw prendido y bloqueaba el DHCP y el DNS de
# Waydroid (Android sin IP ni internet). «vortex-android firewall-root» deja pasar solo la interfaz
# waydroid0; «preparar» lo hace al preparar, y «vortex actualizar» (android_red) una sola vez a quien ya
# tenía Android preparado.
#
# Con ufw, sudo y vortex-android falsos, y root de mentiras (unshare -r): no toca tu firewall.
#
#   pruebas/android-red.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-android-red.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin" "$T/var" "$T/estado"
cat >"$T/bin/ufw" <<EOS
#!/bin/sh
echo "\$*" >>"$T/ufw.log"
[ -e "$T/ufw-falla" ] && exit 1
exit 0
EOS
chmod +x "$T/bin/ufw"

echo "== vortex-android firewall-root"
if ! unshare -r true 2>/dev/null; then
  echo "  --  no hay unshare -r: se salta esta parte"
else
  : >"$T/ufw.log"
  salida="$(VORTEX_ANDROID_UFW="$T/bin/ufw" unshare -r bash "$AQUI/bin/vortex-android" firewall-root 2>&1)"
  codigo=$?
  [[ $codigo -eq 0 ]] && bien "termina bien" || mal "código $codigo: $salida"
  [[ "$(wc -l <"$T/ufw.log")" -eq 4 ]] && bien "pone 4 reglas" || mal "reglas: $(cat "$T/ufw.log")"
  grep -q '^allow in on waydroid0 to any port 67 proto udp' "$T/ufw.log" && bien "DHCP (67/udp) en waydroid0" || mal "sin DHCP"
  grep -q '^allow in on waydroid0 to any port 53 ' "$T/ufw.log" && bien "DNS (53) en waydroid0" || mal "sin DNS"
  grep -q '^route allow in on waydroid0' "$T/ufw.log" && grep -q '^route allow out on waydroid0' "$T/ufw.log" &&
    bien "salida a internet (route in y out)" || mal "sin route"
  ! grep -v 'waydroid0' "$T/ufw.log" | grep -q . && bien "nada fuera de waydroid0" || mal "regla de más: $(cat "$T/ufw.log")"

  : >"$T/ufw-falla"
  salida="$(VORTEX_ANDROID_UFW="$T/bin/ufw" unshare -r bash "$AQUI/bin/vortex-android" firewall-root 2>&1)"
  codigo=$?
  rm -f "$T/ufw-falla"
  [[ $codigo -ne 0 ]] && grep -q 'sin internet' <<<"$salida" && bien "si ufw falla, lo dice y sale con error" ||
    mal "código $codigo: $salida"

  salida="$(VORTEX_ANDROID_UFW=/nonexistent unshare -r bash "$AQUI/bin/vortex-android" firewall-root 2>&1)"
  [[ $? -eq 0 ]] && bien "sin ufw instalado no hace nada" || mal "sin ufw: $salida"
fi
salida="$(VORTEX_ANDROID_UFW="$T/bin/ufw" bash "$AQUI/bin/vortex-android" firewall-root 2>&1)"
[[ $? -ne 0 ]] && grep -q 'root' <<<"$salida" && bien "sin root no lo intenta" || mal "sin root: $salida"

echo "== vortex actualizar (android_red)"
cat >"$T/bin/sudo" <<EOS
#!/bin/sh
echo "sudo \$*" >>"$T/sudo.log"
[ -e "$T/sudo-falla" ] && exit 1
exit 0
EOS
printf '#!/bin/sh\nexit 0\n' >"$T/bin/vortex-android"
chmod +x "$T/bin/sudo" "$T/bin/vortex-android"
FUNCION="$(sed -n '/^android_red() {/,/^}/p' "$AQUI/bin/vortex")"
[[ -n "$FUNCION" ]] || mal "no encontré android_red en bin/vortex"
red() {
  PATH="$T/bin:$PATH" AVISOS_ESTADO="$T/estado" VORTEX_ANDROID_VAR="$T/var" bash -c \
    "aviso() { printf 'vortex: %s\n' \"\$*\" >&2; }; $FUNCION; android_red" 2>&1
}

: >"$T/sudo.log"
red >/dev/null
[[ ! -s "$T/sudo.log" ]] && bien "sin Android preparado no pide nada" || mal "pidió: $(cat "$T/sudo.log")"

: >"$T/var/waydroid.cfg"
: >"$T/sudo-falla"
salida="$(red)"
rm -f "$T/sudo-falla"
grep -q 'a mano: sudo vortex-android firewall-root' <<<"$salida" && [[ ! -e "$T/estado/android-red-ufw" ]] &&
  bien "si sudo falla, avisa y lo vuelve a intentar la próxima vez" || mal "salida: $salida"

: >"$T/sudo.log"
red >/dev/null
grep -qx 'sudo vortex-android firewall-root' "$T/sudo.log" && bien "con Android preparado abre la red" ||
  mal "sudo: $(cat "$T/sudo.log")"
[[ -e "$T/estado/android-red-ufw" ]] && bien "deja la marca" || mal "sin marca"

: >"$T/sudo.log"
red >/dev/null
[[ ! -s "$T/sudo.log" ]] && bien "la segunda vez ya no pide contraseña" || mal "otra vez: $(cat "$T/sudo.log")"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS"
  exit 1
fi
echo "Todo bien"
