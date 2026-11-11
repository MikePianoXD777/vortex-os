#!/usr/bin/env bash
# Pruebas del ajuste del Wi-Fi MediaTek (3.3.4, wifi_mediatek de bin/vortex): con un /sys falso (VORTEX_SYS),
# sudo y nmcli falsos y la carpeta de NetworkManager en un temporal (VORTEX_NM_CONF). No toca tu sistema.
# Comprueba: sin tarjeta MediaTek no hace nada; un MediaTek que no es de red (USB, audio) tampoco; con una
# tarjeta de red MediaTek PCIe (MT7902, sin driver) o SDIO pone el archivo y recarga NetworkManager; si ya
# está igual no vuelve a pedir sudo; y «vortex actualizar» e install.sh lo llaman.
#
#   pruebas/wifi-mediatek.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-wifi-mediatek-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

mkdir -p "$T/bin"
cat >"$T/bin/sudo" <<FALSO
#!/usr/bin/env bash
echo "\$*" >>"$T/sudo.log"
"\$@"
FALSO
cat >"$T/bin/nmcli" <<FALSO
#!/usr/bin/env bash
echo "\$*" >>"$T/nmcli.log"
FALSO
chmod +x "$T/bin/sudo" "$T/bin/nmcli"
export PATH="$T/bin:$PATH"

# Un dispositivo en el /sys falso: BUS NOMBRE VENDOR [CLASE]
dispositivo() {
  mkdir -p "$T/sys/bus/$1/devices/$2"
  echo "$3" >"$T/sys/bus/$1/devices/$2/vendor"
  [[ -n "${4:-}" ]] && echo "$4" >"$T/sys/bus/$1/devices/$2/class"
  return 0
}
limpiar() { rm -rf "$T/sys" "$T/nm" "$T/sudo.log" "$T/nmcli.log"; mkdir -p "$T/sys/bus/pci/devices"; }

# Los lee wifi_mediatek (sacada de bin/vortex con eval).
export VORTEX_DIR="$AQUI" VORTEX_SYS="$T/sys" VORTEX_NM_CONF="$T/nm"
aviso() { echo "aviso: $*" >>"$T/avisos.log"; }
eval "$(sed -n '/^tiene_wifi_mediatek() {/,/^}/p; /^wifi_mediatek() {/,/^}/p' "$AQUI/bin/vortex")"
ARCHIVO="$T/nm/90-vortex-wifi-mediatek.conf"

echo "== Sin tarjeta MediaTek"
limpiar
dispositivo pci 0000:00:14.3 0x8086 0x028000   # Wi-Fi Intel
dispositivo pci 0000:01:00.0 0x10de 0x030000   # tarjeta de video
wifi_mediatek
probar "no pone nada ni pide sudo" "[[ ! -e '$ARCHIVO' && ! -e '$T/sudo.log' ]]"

echo "== MediaTek que no es de red"
limpiar
dispositivo pci 0000:03:00.0 0x14c3 0x0c0330   # un controlador USB MediaTek
wifi_mediatek
probar "no pone nada" "[[ ! -e '$ARCHIVO' ]]"

echo "== MT7902 PCIe (14c3:7902), aunque no tenga driver"
limpiar
dispositivo pci 0000:00:14.3 0x8086 0x060400
dispositivo pci 0000:02:00.0 0x14c3 0x028000
wifi_mediatek
probar "pone el archivo igual al de config/sistema" "cmp -s '$AQUI/config/sistema/wifi-mediatek.conf' '$ARCHIVO'"
probar "apaga el ahorro de energía (wifi.powersave = 2)" "grep -qx 'wifi.powersave = 2' '$ARCHIVO'"
probar "recarga la config de NetworkManager" "grep -qx 'general reload conf' '$T/nmcli.log'"
rm -f "$T/sudo.log"
wifi_mediatek
probar "si ya está igual, no vuelve a pedir sudo" "[[ ! -e '$T/sudo.log' ]]"
echo "viejo" >"$ARCHIVO"
wifi_mediatek
probar "si cambió, lo pone al día" "cmp -s '$AQUI/config/sistema/wifi-mediatek.conf' '$ARCHIVO'"

echo "== MediaTek por SDIO (037a)"
limpiar
dispositivo sdio mmc1:0001:1 0x037a
wifi_mediatek
probar "pone el archivo" "[[ -f '$ARCHIVO' ]]"

echo "== Quién lo llama"
probar "«vortex actualizar» llama a wifi_mediatek" "sed -n '/^cmd_actualizar() {/,/^}/p' '$AQUI/bin/vortex' | grep -qx '  wifi_mediatek'"
probar "install.sh lo pone en las PCs nuevas" "grep -q 'config/sistema/wifi-mediatek.conf' '$AQUI/install/20-sistema.sh' && grep -q '/etc/NetworkManager/conf.d/90-vortex-wifi-mediatek.conf' '$AQUI/install/20-sistema.sh'"
probar "sin avisos de error" "[[ ! -e '$T/avisos.log' ]]"

echo
if ((FALLOS)); then
  echo "FALLARON $FALLOS pruebas"
  exit 1
fi
echo "Todo bien"
