#!/usr/bin/env bash
# shellcheck disable=SC2015
# Prueba de bin/vortex-usb (1.8) sin memorias de verdad: programas falsos (udisksctl, notify-send,
# findmnt, lsblk, xdg-open) en un PATH temporal y eventos de udev escritos a mano.
#
#   pruebas/usb.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-usb-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }

mkdir -p "$T/bin"
# udisksctl: monta en /run/media/prueba/<etiqueta>; «unmount» falla si existe $T/ocupada.
cat >"$T/bin/udisksctl" <<EOF
#!/bin/sh
echo "udisksctl \$*" >>"$T/registro"
case "\$1" in
  mount) echo "Mounted \$3 at /run/media/prueba/MEMORIA." ;;
  unmount) [ -e "$T/ocupada" ] && { echo "target is busy" >&2; exit 1; } ; exit 0 ;;
  power-off) exit 0 ;;
esac
EOF
# notify-send: anota el aviso y, si trae botones, contesta lo que diga $T/respuesta.
cat >"$T/bin/notify-send" <<EOF
#!/bin/sh
echo "notify-send \$*" >>"$T/registro"
case "\$*" in *-A*) cat "$T/respuesta" 2>/dev/null ;; esac
EOF
printf '#!/bin/sh\nexit 0\n' >"$T/bin/findmnt"
printf '#!/bin/sh\ncase "$*" in *PKNAME*) echo sdb ;; *) echo "/dev/sdb"; echo "/dev/sdb1 /run/media/prueba/MEMORIA" ;; esac\n' >"$T/bin/lsblk"
printf '#!/bin/sh\necho "xdg-open $*" >>"%s/registro"\n' "$T" >"$T/bin/xdg-open"
chmod +x "$T/bin/"*

evento() { # ACCION BUS DEVNAME [extra...]
  printf 'UDEV  [1.0] %s /devices/x (block)\nACTION=%s\nSUBSYSTEM=block\nDEVNAME=%s\nID_BUS=%s\nID_FS_USAGE=filesystem\nID_FS_LABEL=MEMORIA\nID_MODEL=Kingston_DT\n' "$1" "$1" "$3" "$2"
  shift 3
  printf '%s\n' "$@"
  echo
}
correr() { : >"$T/registro"; PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex-usb" --eventos; }

echo "== vortex-usb"
evento add usb /dev/sdb1 | correr
grep -q "udisksctl mount -b /dev/sdb1" "$T/registro" && bien "monta la memoria USB" || mal "no la montó"
grep -q "Memoria conectada.*«MEMORIA»" "$T/registro" && bien "avisa con su nombre" || mal "no avisó: $(cat "$T/registro")"
grep -q -- "-A abrir=Abrir -A expulsar=Expulsar" "$T/registro" && bien "con botones Abrir y Expulsar" || mal "sin botones"

echo abrir >"$T/respuesta"
evento add usb /dev/sdb1 | correr
grep -q "xdg-open /run/media/prueba/MEMORIA" "$T/registro" && bien "«Abrir» abre la carpeta" || mal "Abrir no abrió: $(cat "$T/registro")"

echo expulsar >"$T/respuesta"
evento add usb /dev/sdb1 | correr
grep -q "udisksctl unmount -b /dev/sdb1" "$T/registro" && grep -q "udisksctl power-off -b /dev/sdb" "$T/registro" &&
  bien "«Expulsar» desmonta y apaga" || mal "Expulsar: $(cat "$T/registro")"
grep -q "Ya puedes sacar «MEMORIA»" "$T/registro" && bien "y avisa que ya se puede sacar" || mal "no avisó al expulsar"

touch "$T/ocupada"
evento add usb /dev/sdb1 | correr
grep -q "No se puede expulsar" "$T/registro" && ! grep -q "power-off" "$T/registro" &&
  bien "si está en uso no la apaga y lo dice" || mal "en uso: $(cat "$T/registro")"
rm -f "$T/ocupada" "$T/respuesta"

evento add ata /dev/sda1 ID_PATH=pci-0000:00:17.0-ata-2.0 | correr
[[ ! -s "$T/registro" ]] && bien "un disco interno (SATA) no se toca" || mal "tocó un disco interno: $(cat "$T/registro")"
evento remove usb /dev/sdb1 | correr
[[ ! -s "$T/registro" ]] && bien "al sacarla no hace nada" || mal "hizo algo al sacarla"
printf 'ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdb\nID_BUS=usb\nID_PART_TABLE_TYPE=dos\n\n' | correr
[[ ! -s "$T/registro" ]] && bien "el disco entero (sin sistema de archivos) no se monta" || mal "montó el disco entero"
evento add "" /dev/mmcblk0p1 | correr
grep -q "udisksctl mount -b /dev/mmcblk0p1" "$T/registro" && bien "tarjeta SD en el lector" || mal "no montó la SD"
printf 'ACTION=add\nSUBSYSTEM=block\nDEVNAME=/dev/sdc1\nID_BUS=usb\nID_FS_USAGE=filesystem\nID_FS_LABEL=MI USB\n\n' | correr
grep -q "«MI USB»" "$T/registro" && bien "etiqueta con espacios" || mal "etiqueta con espacios: $(cat "$T/registro")"

echo
((FALLOS)) && { echo "$FALLOS pruebas fallaron."; exit 1; }
echo "Todas las pruebas pasaron."
