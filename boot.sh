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

# -Syu y no -Sy: un «pacman -Sy» suelto deja el sistema con bases de datos
# nuevas y paquetes viejos (actualización parcial, no soportada en Arch).
sudo pacman -Syu --needed --noconfirm git

if [[ -d "$DESTINO/.git" ]]; then
  git -C "$DESTINO" pull --ff-only
else
  rm -rf "$DESTINO"
  git clone --depth 1 --branch "$RAMA" "$REPO" "$DESTINO"
fi

# El instalador lee del teclado, así que se reconecta a la terminal. Sin
# terminal de control (ssh sin -t, cloud-init, CI) /dev/tty no se puede abrir:
# en ese caso se corre igual, y preguntar() usa los valores por defecto.
if { : </dev/tty; } 2>/dev/null; then
  exec "$DESTINO/install.sh" "$@" </dev/tty
else
  exec "$DESTINO/install.sh" "$@"
fi
