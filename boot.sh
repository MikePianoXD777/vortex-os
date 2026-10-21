#!/usr/bin/env bash
# Instalación con un solo comando, al estilo Omarchy:
#   curl -fsSL https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/boot.sh | bash
# Descarga Vortex OS en ~/.local/share/vortex-os y corre el instalador.
set -euo pipefail

REPO="${VORTEX_REPO:-https://github.com/MikePianoXD777/vortex-os.git}"
RAMA="${VORTEX_RAMA:-main}"
DESTINO="${XDG_DATA_HOME:-$HOME/.local/share}/vortex-os"

if [[ $EUID -eq 0 ]]; then
  echo "Corre esto con tu usuario normal, no como root." >&2
  exit 1
fi

sudo pacman -Sy --needed --noconfirm git

if [[ -d "$DESTINO/.git" ]]; then
  git -C "$DESTINO" pull --ff-only
else
  rm -rf "$DESTINO"
  git clone --depth 1 --branch "$RAMA" "$REPO" "$DESTINO"
fi

# El instalador lee del teclado, así que se reconecta a la terminal.
exec "$DESTINO/install.sh" "$@" </dev/tty
