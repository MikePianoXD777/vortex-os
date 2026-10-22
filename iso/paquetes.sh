#!/usr/bin/env bash
# Arma el repo local [vortex] con los paquetes propios de Vortex OS (los que no están en los
# repos de Arch ni de CachyOS). Se corre como usuario normal (makepkg no corre como root).
#
#   iso/paquetes.sh [carpeta]     # por defecto /var/tmp/vortex-repo
#
# Después: sudo iso/construir.sh
set -euo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"
REPO="$(realpath -m "${1:-/var/tmp/vortex-repo}")"

[[ $EUID -ne 0 ]] || { echo "Corre esto como usuario normal (makepkg no corre como root)." >&2; exit 1; }
mkdir -p "$REPO"
export PKGDEST="$REPO"

# Los PKGBUILD de este repo.
for d in vortex-shell vortex-studio vortex-flatpaks; do
  echo "==> $d"
  (cd "$RAIZ/packaging/$d" && makepkg -s --noconfirm --needed -f)
  rm -rf "$RAIZ/packaging/$d/"{pkg,src}
done

# Del AUR: la luz nocturna que pide el shell.
aur="$(mktemp -d)"
trap 'rm -rf "$aur"' EXIT
for p in wl-gammarelay-rs; do
  echo "==> $p (AUR)"
  git clone -q --depth 1 "https://aur.archlinux.org/$p.git" "$aur/$p"
  (cd "$aur/$p" && makepkg -s --noconfirm --needed -f)
done

# Solo la última versión de cada paquete, y la base de datos del repo.
cd "$REPO"
rm -f vortex.db* vortex.files*
repo-add -q -R vortex.db.tar.gz ./*.pkg.tar.zst
ls -la "$REPO"
