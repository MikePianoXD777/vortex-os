#!/usr/bin/env bash
# Previews para promocionar Vortex OS (TikTok, etc.), grabadas en la VM de pruebas.
#
#   vm/preview.sh preparar                pone la VM a 1920x1080 e instala wf-recorder
#   vm/preview.sh foto NOMBRE             captura la pantalla de la VM
#   vm/preview.sh clip NOMBRE SEG [CMD]   graba SEG segundos; CMD (opcional) corre en la sesión
#                                         de la VM al empezar (p. ej. abrir un panel)
#   vm/preview.sh vertical ARCHIVO        versión 1080x1920 para TikTok (fondo desenfocado)
#   vm/preview.sh verticales              hace la versión vertical de todo lo que falte
#
# Todo queda en ~/Vídeos/Vortex OS previews/ (fotos/, clips/ y vertical/).
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SALIDA="${VORTEX_PREVIEWS:-$(xdg-user-dir VIDEOS 2>/dev/null || echo "$HOME/Videos")/Vortex OS previews}"
mkdir -p "$SALIDA/fotos" "$SALIDA/clips" "$SALIDA/vertical"

# Corre un comando dentro de la sesión gráfica de la VM (usuario vortex).
en_sesion() {
  VORTEX_VM_USUARIO=vortex "$AQUI/vm.sh" ssh "export XDG_RUNTIME_DIR=/run/user/1000 WAYLAND_DISPLAY=wayland-1 \
    HYPRLAND_INSTANCE_SIGNATURE=\$(ls -t /run/user/1000/hypr | head -1); $1"
}

traer() { VORTEX_VM_USUARIO=vortex "$AQUI/vm.sh" copiar ":$1" "$2"; }

nombre_valido() { [[ "$1" =~ ^[A-Za-z0-9._-]+$ ]] || { echo "Nombre inválido: $1 (solo letras, números, . _ -)" >&2; exit 1; }; }

case "${1:-}" in
  preparar)
    en_sesion 'command -v wf-recorder >/dev/null || sudo pacman -S --needed --noconfirm wf-recorder >/dev/null
      monitor=$(hyprctl -j monitors | jq -r ".[0].name")
      hyprctl eval "hl.monitor({ output = \"$monitor\", mode = \"1920x1080@60\", position = \"0x0\", scale = 1 })" >/dev/null
      hyprctl -j monitors | jq -r ".[0] | \"\(.name) \(.width)x\(.height)\""'
    ;;
  foto)
    nombre_valido "${2:?falta el nombre}"
    en_sesion "grim /tmp/preview-$2.png"
    traer "/tmp/preview-$2.png" "$SALIDA/fotos/$2.png"
    echo "$SALIDA/fotos/$2.png"
    ;;
  clip)
    nombre_valido "${2:?falta el nombre}"
    seg="${3:?faltan los segundos}"
    [[ "$seg" =~ ^[0-9]+$ ]] || { echo "Los segundos deben ser un número" >&2; exit 1; }
    accion="${4:-true}"
    en_sesion "rm -f /tmp/preview-$2.mp4
      timeout -s INT $((seg + 1)) wf-recorder -y -r 30 -c libx264 -p preset=veryfast -p crf=20 -f /tmp/preview-$2.mp4 >/dev/null 2>&1 &
      sleep 1; ($accion) >/dev/null 2>&1 & wait"
    traer "/tmp/preview-$2.mp4" "$SALIDA/clips/$2.mp4"
    echo "$SALIDA/clips/$2.mp4"
    ;;
  vertical)
    origen="${2:?falta el archivo}"
    base="$(basename "${origen%.*}")"
    # Fondo: la misma imagen ampliada y desenfocada; encima, la pantalla a lo ancho, centrada.
    filtro='[0:v]scale=-2:1920,crop=1080:1920,boxblur=30:3,eq=brightness=-0.08[fondo];[0:v]scale=1080:-2[frente];[fondo][frente]overlay=(W-w)/2:(H-h)/2'
    if [[ "$origen" == *.mp4 ]]; then
      ffmpeg -v error -y -i "$origen" -filter_complex "$filtro" -c:v libx264 -preset medium -crf 18 -pix_fmt yuv420p -an "$SALIDA/vertical/$base.mp4"
      echo "$SALIDA/vertical/$base.mp4"
    else
      ffmpeg -v error -y -i "$origen" -filter_complex "$filtro" -frames:v 1 "$SALIDA/vertical/$base.png"
      echo "$SALIDA/vertical/$base.png"
    fi
    ;;
  verticales)
    for f in "$SALIDA"/fotos/*.png "$SALIDA"/clips/*.mp4; do
      [[ -f "$f" ]] || continue
      b="$(basename "${f%.*}")"
      [[ -f "$SALIDA/vertical/$b.png" || -f "$SALIDA/vertical/$b.mp4" ]] || "$0" vertical "$f"
    done
    ;;
  *)
    sed -n '2,11p' "$0" | sed 's/^# \{0,1\}//'
    ;;
esac
