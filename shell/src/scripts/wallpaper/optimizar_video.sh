#!/usr/bin/env bash
# Vortex: capa de optimización de los fondos animados. Hace una copia del video al tamaño del monitor,
# a 30 fps como mucho, en H.264 (lo decodifica la GPU en casi cualquier PC) y sin audio, y la guarda en
# la caché; un fondo de 4K a 60 fps pasa a gastar una fracción. Imprime en la última línea qué archivo
# reproducir: la copia, o el original si ya es ligero o si no se pudo optimizar.
#
#   optimizar_video.sh VIDEO ANCHO ALTO CARPETA_CACHE
#
# Corre con prioridad baja (nice/ionice) y una sola vez por video y tamaño (flock): dos monitores con
# el mismo fondo esperan a la misma copia. Las copias que nadie usa en 30 días se borran.
set -uo pipefail

entrada="$1" ancho="$2" alto="$3" cache="$4"
[[ -f "$entrada" && "$ancho" =~ ^[0-9]+$ && "$alto" =~ ^[0-9]+$ ]] || { printf '%s\n' "$entrada"; exit 0; }
command -v ffmpeg >/dev/null && command -v ffprobe >/dev/null || { printf '%s\n' "$entrada"; exit 0; }
mkdir -p "$cache" || { printf '%s\n' "$entrada"; exit 0; }
find "$cache" -maxdepth 1 -name '*.mp4' -atime +30 -delete 2>/dev/null || true

# La clave cambia si el archivo cambia (tamaño y fecha) o se mueve.
clave="$(stat -c '%s-%Y' "$entrada")-$(printf '%s' "$entrada" | sha1sum | cut -c1-16)"
salida="$cache/$clave-${ancho}x${alto}.mp4"
if [[ -s "$salida" ]]; then
  touch -a "$salida" 2>/dev/null || true
  printf '%s\n' "$salida"
  exit 0
fi

# ¿Ya es ligero? H.264, no más grande que el monitor (con un 10 % de margen) y a 31 fps o menos.
IFS=, read -r codec w h fps < <(ffprobe -v error -select_streams v:0 \
  -show_entries stream=codec_name,width,height,avg_frame_rate -of csv=p=0 "$entrada" 2>/dev/null)
fps_num=0
if [[ "${fps:-}" =~ ^([0-9]+)/([0-9]+)$ ]] && ((BASH_REMATCH[2] > 0)); then
  fps_num=$((BASH_REMATCH[1] / BASH_REMATCH[2]))
fi
if [[ "${codec:-}" == h264 && "${w:-0}" =~ ^[0-9]+$ && "${h:-0}" =~ ^[0-9]+$ ]] &&
  ((w * 10 <= ancho * 11 && h * 10 <= alto * 11 && fps_num > 0 && fps_num <= 31)); then
  printf '%s\n' "$entrada"
  exit 0
fi

exec 9>"$salida.lock" || { printf '%s\n' "$entrada"; exit 0; }
flock 9
if [[ ! -s "$salida" ]]; then
  tmp="$salida.parcial.mp4"
  # Cubre el monitor (como PreserveAspectCrop) y recorta lo que sobra; dimensiones pares para yuv420p.
  if nice -n 19 ionice -c 3 ffmpeg -nostdin -hide_banner -loglevel error -y -i "$entrada" -an \
    -vf "scale=${ancho}:${alto}:force_original_aspect_ratio=increase,crop=${ancho}:${alto},fps=fps='min(source_fps,30)',format=yuv420p" \
    -c:v libx264 -preset veryfast -crf 24 -tune film -movflags +faststart "$tmp"; then
    mv -f "$tmp" "$salida"
  else
    rm -f "$tmp"
  fi
fi
rm -f "$salida.lock"
if [[ -s "$salida" ]]; then printf '%s\n' "$salida"; else printf '%s\n' "$entrada"; fi
