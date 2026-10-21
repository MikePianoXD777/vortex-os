#!/usr/bin/env bash
# Construye la ISO de Vortex OS con archiso. Se corre como root en un Arch con «archiso».
#
#   sudo iso/construir.sh [carpeta-de-salida]
#
# Parte del perfil oficial «releng» de Arch y le agrega Vortex encima:
# el código de Vortex en /root/vortex-os, el comando «vortex-instalar» y su mensaje de bienvenida.
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"
SALIDA="$(realpath -m "${1:-$AQUI/out}")"
TRABAJO="${VORTEX_ISO_TRABAJO:-/var/tmp/vortex-iso}"

[[ $EUID -eq 0 ]] || { echo "Corre esto como root (sudo)." >&2; exit 1; }
command -v mkarchiso >/dev/null || pacman -S --needed --noconfirm archiso

rm -rf "$TRABAJO"
mkdir -p "$TRABAJO" "$SALIDA"
perfil="$TRABAJO/perfil"
cp -a /usr/share/archiso/configs/releng "$perfil"

# Encima del perfil oficial: archivos de airootfs, paquetes extra y nombre de la ISO.
cp -a "$AQUI/airootfs/." "$perfil/airootfs/"
cat "$AQUI/paquetes-extra.txt" >>"$perfil/packages.x86_64"

# El código de Vortex viaja dentro de la ISO para instalar aunque GitHub no responda.
mkdir -p "$perfil/airootfs/root/vortex-os"
tar -C "$RAIZ" --exclude=./iso/out --exclude=./vortex-shell --exclude=./.git -cf - . |
  tar -C "$perfil/airootfs/root/vortex-os" -xf -

sed -i \
  -e 's/^iso_name=.*/iso_name="vortex-os"/' \
  -e 's/^iso_label=.*/iso_label="VORTEX_$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y%m)"/' \
  -e 's|^iso_publisher=.*|iso_publisher="Vortex OS <https://github.com/MikePianoXD777/vortex-os>"|' \
  -e 's/^iso_application=.*/iso_application="Vortex OS Live"/' \
  "$perfil/profiledef.sh"
# Permisos de los scripts agregados.
sed -i 's|^file_permissions=(|file_permissions=(\n  ["/usr/local/bin/vortex-instalar"]="0:0:755"\n  ["/usr/local/bin/vortex-preparar"]="0:0:755"\n  ["/root/vortex-os"]="0:0:755"|' \
  "$perfil/profiledef.sh"

# Título de los menús de arranque.
grep -rl 'Arch Linux install medium' "$perfil"/efiboot "$perfil"/syslinux "$perfil"/grub 2>/dev/null |
  xargs -r sed -i 's/Arch Linux install medium/Vortex OS/g'

mkarchiso -v -w "$TRABAJO/work" -o "$SALIDA" "$perfil"
rm -rf "$TRABAJO"
ls -lh "$SALIDA"/vortex-os-*.iso
