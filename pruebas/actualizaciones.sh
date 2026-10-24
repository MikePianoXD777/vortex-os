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

if ((FALLOS)); then
  echo "$FALLOS pruebas fallaron"
  exit 1
fi
echo "Todo bien"
