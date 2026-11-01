#!/usr/bin/env bash
# Prueba de «Bajar con copiar el enlace» (2.5, bin/vortex-enlace). Qué enlaces cuentan; que el aviso sale
# una vez, no en modo juego y no con texto que solo trae un enlace; y la bajada de verdad con el yt-dlp del
# sistema contra un servidor web local con un video hecho por ffmpeg (no baja nada de internet). HOME y XDG_*
# temporales; notify-send, xdg-open y xdg-user-dir son falsos: anotan lo que les piden y contestan el botón
# que diga el archivo de respuestas.
#
#   pruebas/enlace.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VE="$AQUI/bin/vortex-enlace"
FALLOS=0
bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }
esperar() { for _ in $(seq 100); do eval "$1" && return 0; sleep 0.1; done; return 1; }

T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-enlace.XXXXXX")"
SERVIDOR=""
trap '[[ -n "$SERVIDOR" ]] && kill "$SERVIDOR" 2>/dev/null; rm -rf "$T"' EXIT
mkdir -p "$T/home" "$T/estado" "$T/bin" "$T/web"
export HOME="$T/home" XDG_STATE_HOME="$T/estado" XDG_RUNTIME_DIR="$T" AVISOS="$T/avisos" RESPUESTAS="$T/respuestas"
export PATH="$T/bin:$PATH"
: >"$AVISOS"; : >"$RESPUESTAS"
# notify-send falso: anota título|cuerpo|-r|valor del avance; con -p da un id y luego el botón de turno
cat >"$T/bin/notify-send" <<'EOF'
#!/usr/bin/env bash
r=""; v=""; p=0; a=0
while (($# > 2)); do
  case "$1" in -r) r="$2"; shift ;; -h) v="${2##*:}"; shift ;; -p) p=1 ;; -A) a=1; shift ;; -a|-i|-t) shift ;; esac
  shift
done
printf '%s|%s|r=%s|v=%s\n' "$1" "$2" "$r" "$v" >>"$AVISOS"
((p)) && echo 7
if ((a)); then resp="$(head -1 "$RESPUESTAS")"; sed -i 1d "$RESPUESTAS"; [[ -n "$resp" ]] && echo "$resp"; fi
exit 0
EOF
printf '#!/usr/bin/env bash\nprintf "%%s\\n" "$1" >>"%s/abiertos"\n' "$T" >"$T/bin/xdg-open"
printf '#!/usr/bin/env bash\ncase "$1" in MUSIC) echo "%s/home/Música" ;; VIDEOS) echo "%s/home/Videos" ;; *) echo "%s/home" ;; esac\n' "$T" "$T" "$T" >"$T/bin/xdg-user-dir"
chmod +x "$T/bin/"*

echo "== Qué enlaces cuentan"
es() { python3 -c 'import importlib.util,importlib.machinery,sys
l=importlib.machinery.SourceFileLoader("ve",sys.argv[1]); s=importlib.util.spec_from_loader("ve",l); m=importlib.util.module_from_spec(s); l.exec_module(m)
print(m.sitio(sys.argv[2]))' "$VE" "$1"; }
for par in "https://www.youtube.com/watch?v=dQw4w9WgXcQ|YouTube" "https://youtu.be/dQw4w9WgXcQ?si=abc|YouTube" \
  "https://youtube.com/shorts/abcDEF12345|YouTube" "https://music.youtube.com/watch?v=abcDEF12345&list=RD|YouTube" \
  "https://www.tiktok.com/@alguien/video/7312345678901234567|TikTok" "https://vm.tiktok.com/ZMabc123/|TikTok" \
  "https://www.instagram.com/reel/C1aBcDeF/|Instagram" "https://www.instagram.com/p/C1aBcDeF/?igsh=x|Instagram" \
  "https://x.com/alguien/status/1850000000000000000|X" "  https://youtu.be/dQw4w9WgXcQ  |YouTube" \
  "mira esto https://youtu.be/dQw4w9WgXcQ|None" "https://www.youtube.com/@canal|None" "https://www.google.com/|None" \
  "https://www.instagram.com/alguien/|None" "hola|None"; do
  probar "«${par%|*}» → ${par##*|}" "[[ \"\$(es '${par%|*}')\" == '${par##*|}' ]]"
done

echo "== El aviso al copiar"
copiar() { printf '%s' "$1" | "$VE" revisar; }
copiar "https://youtu.be/dQw4w9WgXcQ"
esperar "grep -q 'Bajar este video de YouTube' '$AVISOS'"
probar "copiar un enlace de YouTube ofrece bajarlo" "grep -q '^¿Bajar este video de YouTube?|Copiaste su enlace.' '$AVISOS'"
copiar "https://youtu.be/dQw4w9WgXcQ"
sleep 0.5
probar "el mismo enlace otra vez no vuelve a avisar" "[[ \$(grep -c 'Bajar este video' '$AVISOS') == 1 ]]"
copiar "mira https://youtu.be/otroVideo12"
copiar "una receta de 1/2 taza"
sleep 0.5
probar "un texto que trae un enlace no avisa" "[[ \$(wc -l <'$AVISOS') == 1 ]]"
mkdir -p "$T/estado/vortex" && touch "$T/estado/vortex/modo-juego"
copiar "https://www.tiktok.com/@alguien/video/7312345678901234567"
sleep 0.5
probar "en modo juego no avisa" "! grep -q TikTok '$AVISOS'"
rm "$T/estado/vortex/modo-juego"
probar "wl-paste: con el candado tomado, otro «vortex-enlace» sale sin escuchar" \
  "python3 -c 'import fcntl,os,subprocess,sys
fd=os.open(sys.argv[1],os.O_CREAT|os.O_RDWR); fcntl.flock(fd,fcntl.LOCK_EX)
sys.exit(subprocess.run([sys.argv[2]],timeout=5).returncode)' '$T/vortex-enlace.lock' '$VE'"

echo "== Bajar (yt-dlp de verdad, servidor local)"
if ! command -v yt-dlp >/dev/null || ! command -v ffmpeg >/dev/null; then
  echo "  --  falta yt-dlp o ffmpeg: se salta"
else
  ffmpeg -v error -f lavfi -i testsrc=size=320x240:rate=10:duration=3 -f lavfi -i sine=frequency=440:duration=3 \
    -c:v libx264 -c:a aac -shortest "$T/web/Clase de piano.mp4"
  python3 -u -m http.server 0 --bind 127.0.0.1 --directory "$T/web" >"$T/servidor.log" 2>&1 &
  SERVIDOR=$!
  esperar "grep -q port '$T/servidor.log'"
  WEB="http://127.0.0.1:$(grep -oE 'port [0-9]+' "$T/servidor.log" | head -1 | cut -d' ' -f2)"
  : >"$AVISOS"; echo abrir >"$RESPUESTAS"
  "$VE" bajar video "$WEB/Clase%20de%20piano.mp4"
  V="$(ls "$T/home/Videos/"*.mp4 2>/dev/null | head -1)"
  esperar "[[ -s '$T/abiertos' ]]"
  cp "$AVISOS" "$T/avisos-video"
  probar "el video queda en Videos, como mp4" "[[ -s '$V' ]]"
  probar "avisa «Bajando el video…» y lo va poniendo al día (el mismo aviso)" \
    "grep -q '^Bajando el video…|' '$AVISOS' && grep -q '^Bajando el video… [0-9]* %|.*|r=7|v=' '$AVISOS'"
  probar "al final «Listo» en el mismo aviso, y «Abrir» lo abre" \
    "grep -q '^Listo: Clase de piano.*|Está en tu carpeta Videos.|r=7' '$T/avisos-video' && grep -qxF '$V' '$T/abiertos'"
  : >"$AVISOS"; echo carpeta >"$RESPUESTAS"
  "$VE" bajar audio "$WEB/Clase%20de%20piano.mp4"
  A="$(ls "$T/home/Música/"*.mp3 2>/dev/null | head -1)"
  esperar "grep -q Música '$T/abiertos'"
  probar "solo audio: un mp3 en Música (sin «NA -» si no hay artista)" "[[ \"\$(basename \"$A\")\" == 'Clase de piano.mp3' && -s '$A' ]] && ffprobe -v error -show_entries stream=codec_name -of csv=p=0 '$A' | grep -qx mp3"
  probar "«Ver en la carpeta» abre Música" "grep -qx '$T/home/Música' '$T/abiertos'"
  : >"$AVISOS"
  "$VE" bajar video "$WEB/no-existe.mp4"
  probar "si falla, lo dice en el mismo aviso y no deja nada" \
    "grep -q '^No se pudo bajar el video|El video ya no está disponible.|r=7' '$AVISOS' && [[ \$(ls '$T/home/Videos' | wc -l) == 1 ]]"
  probar "no deja archivos temporales" "! ls '${TMPDIR:-/tmp}'/vortex-enlace-* >/dev/null 2>&1 || [[ -z \"\$(find '${TMPDIR:-/tmp}' -maxdepth 1 -name 'vortex-enlace-*' -newer '$T/web' 2>/dev/null)\" ]]"
fi

if ((FALLOS)); then echo "--- avisos"; cat "$T/avisos-video" "$AVISOS" 2>/dev/null; cat "$T/abiertos" 2>/dev/null; fi
echo
((FALLOS == 0)) && echo "Todo bien" || { echo "FALLARON $FALLOS pruebas"; exit 1; }
