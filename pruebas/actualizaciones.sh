#!/usr/bin/env bash
# Pruebas del aviso de actualizaciones al entrar: «vortex primer-inicio» lo enciende la primera vez
# (así llega a los sistemas recién instalados desde la ISO, que nunca corrieron «vortex actualizar»),
# no lo vuelve a encender y respeta «vortex actualizaciones off». Con un systemctl falso y un HOME
# temporal: no toca tu systemd ni tu config.
#
#   pruebas/actualizaciones.sh
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-actualizaciones-prueba.XXXXXX")"
trap 'rm -rf "$T"' EXIT
FALLOS=0

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

if [[ -e /etc/vortex/live ]]; then
  echo "En el live el aviso no se enciende: estas pruebas no aplican."
  exit 0
fi

# systemctl falso: anota lo que le piden; con $T/systemctl-falla, falla (systemd --user no responde).
mkdir -p "$T/bin"
cat >"$T/bin/systemctl" <<FALSO
#!/usr/bin/env bash
echo "\$*" >>"$T/systemctl.log"
[[ "\$*" == *is-enabled* && -e "$T/deshabilitado" ]] && exit 1
[[ ! -e "$T/systemctl-falla" ]]
FALSO
chmod +x "$T/bin/systemctl"

# Ya hay fondo: «primer-inicio» no espera al shell.
mkdir -p "$T/cache/serpantinum/wallpaper"
: >"$T/cache/serpantinum/wallpaper/current_prueba"

vortex() {
  env -u XDG_CONFIG_HOME -u XDG_STATE_HOME HOME="$T/casa" XDG_CACHE_HOME="$T/cache" \
    PATH="$T/bin:$PATH" bash "$AQUI/bin/vortex" "$@" >/dev/null 2>&1
}
marca="$T/casa/.local/state/vortex/actualizaciones-configurado"
timer="$T/casa/.config/systemd/user/vortex-actualizaciones.timer"
encendidas() { grep -c "enable --now vortex-actualizaciones.timer" "$T/systemctl.log" 2>/dev/null || true; }

echo "== Primer inicio sin systemd --user =="
: >"$T/systemctl-falla"
vortex primer-inicio
probar "no deja la marca (se reintenta al entrar otra vez)" "[[ ! -e '$marca' ]]"
rm -f "$T/systemctl-falla" "$T/systemctl.log"

echo "== Primer inicio =="
vortex primer-inicio
probar "escribe el temporizador" "[[ -f '$timer' ]]"
probar "lo enciende (enable --now)" '[[ "$(encendidas)" == 1 ]]'
probar "deja la marca" "[[ -e '$marca' ]]"

echo "== Segundo inicio =="
vortex primer-inicio
probar "no lo vuelve a encender" '[[ "$(encendidas)" == 1 ]]'

echo "== Apagado antes del primer inicio =="
rm -rf "$T/casa" "$T/systemctl.log"
vortex actualizaciones off
vortex primer-inicio
probar "«off» se respeta: no lo enciende al entrar" '[[ "$(encendidas)" == 0 ]]'

echo "== Unidades de antes de la 1.7.2 (cada 6 h) =="
unidades="$T/casa/.config/systemd/user"
viejas() {
  rm -rf "$T/casa" "$T/systemctl.log"
  mkdir -p "$unidades" "$T/casa/.local/state/vortex"
  : >"$marca"
  printf '[Service]\nKillMode=process\n' >"$unidades/vortex-actualizaciones.service"
  printf '[Timer]\nOnStartupSec=10min\nOnUnitActiveSec=6h\n' >"$timer"
}
viejas
vortex primer-inicio
probar "el temporizador pasa a cada 10 min" "grep -q '^OnUnitActiveSec=10min' '$timer'"
probar "…y empieza a los 2 min de entrar" "grep -q '^OnStartupSec=2min' '$timer'"
probar "…y se reinicia con el ritmo nuevo" "grep -q 'restart vortex-actualizaciones.timer' '$T/systemctl.log'"
viejas
: >"$T/deshabilitado"
vortex primer-inicio
probar "apagado: se reescribe pero no se arranca" "grep -q '^OnUnitActiveSec=10min' '$timer' && ! grep -q 'restart' '$T/systemctl.log'"
rm -f "$T/deshabilitado"
: >"$T/systemctl.log"
vortex primer-inicio
probar "ya al día: no se toca otra vez" "! grep -q 'restart' '$T/systemctl.log'"

echo "== Revisar: Vortexine's cada vez, el sistema cada 6 h, con su sonido =="
rm -rf "$T/casa"
cat >"$T/bin/checkupdates" <<FALSO
#!/usr/bin/env bash
echo x >>"$T/checkupdates.log"
echo "linux 7.3.0-1 -> 7.3.0-2"
FALSO
cat >"$T/bin/flatpak" <<'FALSO'
#!/usr/bin/env bash
exit 0
FALSO
cat >"$T/bin/git" <<FALSO
#!/usr/bin/env bash
if [[ "\$1" == ls-remote ]]; then cat "$T/remoto" 2>/dev/null; exit 0; fi
exec /usr/bin/git "\$@"
FALSO
cat >"$T/bin/notify-send" <<FALSO
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$T/notify.log"
FALSO
chmod +x "$T/bin/"*
llamadas() { if [[ -f "$T/$1" ]]; then wc -l <"$T/$1"; else echo 0; fi; }
esperar_aviso() { for _ in $(seq 20); do [[ "$(llamadas notify.log)" -ge "$1" ]] && return; sleep .1; done; }
printf '%s\trefs/heads/main\n' "$(git -C "$AQUI" rev-parse HEAD)" >"$T/remoto"
vortex actualizaciones revisar
esperar_aviso 1
probar "pregunta al sistema la primera vez" '[[ "$(llamadas checkupdates.log)" == 1 ]]'
probar "avisa de los paquetes del sistema" "grep -q '1 paquetes del sistema' '$T/notify.log'"
probar "con el sonido de Vortex (no el de las notificaciones)" "grep -q 'sound-file:$AQUI/config/sonidos/actualizacion.wav' '$T/notify.log'"
probar "el sonido existe" "[[ -s '$AQUI/config/sonidos/actualizacion.wav' ]]"
printf 'cafe\trefs/heads/main\n' >"$T/remoto"
vortex actualizaciones revisar
esperar_aviso 2
probar "a los 10 min ve la versión nueva de Vortexine's" "tail -1 '$T/notify.log' | grep -q 'versión nueva'"
probar "…sin volver a bajar las listas del sistema" '[[ "$(llamadas checkupdates.log)" == 1 ]]'
vortex actualizaciones revisar
sleep .5
probar "lo mismo no se avisa dos veces" '[[ "$(llamadas notify.log)" == 2 ]]'
touch -d '7 hours ago' "$T/casa/.local/state/vortex/actualizaciones-sistema"
vortex actualizaciones revisar
probar "pasadas 6 h, pregunta al sistema otra vez" '[[ "$(llamadas checkupdates.log)" == 2 ]]'
mkdir -p "$T/casa/.config/vortex"
: >"$T/mio.ogg"
printf 'SONIDO=%q\n' "$T/mio.ogg" >"$T/casa/.config/vortex/actualizaciones.conf"
printf 'beef\trefs/heads/main\n' >"$T/remoto"
vortex actualizaciones revisar
esperar_aviso 3
probar "el sonido que elegiste manda" "tail -1 '$T/notify.log' | grep -q 'sound-file:$T/mio.ogg'"

if ((FALLOS)); then
  echo "$FALLOS pruebas fallaron"
  exit 1
fi
echo "Todo bien"
