# shellcheck shell=bash
# Funciones comunes del instalador de Vortex OS.

VORTEX_VERSION="0.1.0"
VORTEX_DRY_RUN="${VORTEX_DRY_RUN:-0}"
VORTEX_SI="${VORTEX_SI:-0}"
VORTEX_LOG="${VORTEX_LOG:-${XDG_STATE_HOME:-$HOME/.local/state}/vortex/instalacion.log}"

if [[ -t 1 ]]; then
  C_AZUL=$'\e[1;34m' C_VERDE=$'\e[1;32m' C_AMARILLO=$'\e[1;33m' C_ROJO=$'\e[1;31m' C_GRIS=$'\e[2m' C_FIN=$'\e[0m'
else
  C_AZUL='' C_VERDE='' C_AMARILLO='' C_ROJO='' C_GRIS='' C_FIN=''
fi

mkdir -p "$(dirname "$VORTEX_LOG")"

_log() { printf '%s %s\n' "$(date '+%F %T')" "$*" >>"$VORTEX_LOG"; }
paso()  { _log "== $*"; printf '\n%s==> %s%s\n' "$C_AZUL" "$*" "$C_FIN"; }
info()  { _log "   $*"; printf '    %s\n' "$*"; }
bien()  { _log "ok $*"; printf '    %s✓ %s%s\n' "$C_VERDE" "$*" "$C_FIN"; }
aviso() { _log "!! $*"; printf '    %s! %s%s\n' "$C_AMARILLO" "$*" "$C_FIN" >&2; }
error() { _log "XX $*"; printf '%s✗ %s%s\n' "$C_ROJO" "$*" "$C_FIN" >&2; exit 1; }

# Ejecuta un comando, o solo lo muestra en modo de prueba (--prueba).
run() {
  _log "\$ $*"
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    printf '    %s$ %s%s\n' "$C_GRIS" "$*" "$C_FIN"
    return 0
  fi
  "$@"
}

# Como run, pero con sudo.
srun() { run sudo "$@"; }

# Escribe la entrada estándar en un archivo del sistema (con sudo).
escribir_sistema() {
  local destino="$1"
  local contenido
  contenido="$(cat)"
  _log "> $destino"
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    printf '    %s> %s%s\n' "$C_GRIS" "$destino" "$C_FIN"
    return 0
  fi
  sudo install -Dm644 /dev/stdin "$destino" <<<"$contenido"
}

# Pregunta sí/no. Con --si, devuelve la respuesta por defecto.
preguntar() {
  local pregunta="$1" defecto="${2:-s}" r
  if [[ "$VORTEX_SI" == 1 || ! -t 0 ]]; then
    [[ "$defecto" == s ]]
    return
  fi
  local opciones="[S/n]"
  [[ "$defecto" == n ]] && opciones="[s/N]"
  read -r -p "    $pregunta $opciones " r
  r="${r:-$defecto}"
  [[ "${r,,}" == s* || "${r,,}" == y* ]]
}

tiene() { command -v "$1" >/dev/null 2>&1; }

# Lee una lista de paquetes (ignora comentarios y líneas vacías).
leer_paquetes() {
  sed -e 's/#.*//' -e 's/[[:space:]]*$//' "$1" | grep -v '^$'
}

# Los espejos a veces van atrasados y dan 404: se reintenta refrescando las bases de datos.
con_reintentos() {
  local intento
  for intento in 1 2 3; do
    "$@" && return 0
    [[ "$VORTEX_DRY_RUN" == 1 ]] && return 0
    aviso "Falló la descarga (intento $intento de 3). Reintento en 10 s…"
    sleep 10
    sudo pacman -Syy >/dev/null || true
  done
  return 1
}

instalar_pacman() {
  (($#)) || return 0
  con_reintentos srun pacman -S --needed --noconfirm "$@"
}

instalar_aur() {
  (($#)) || return 0
  con_reintentos run paru -S --needed --noconfirm "$@"
}

# Copia un archivo de config del usuario, guardando antes el que hubiera.
copiar_config() {
  local origen="$1" destino="$2"
  if [[ -e "$destino" ]] && ! cmp -s "$origen" "$destino"; then
    local respaldo
    respaldo="$VORTEX_RESPALDO/${destino#"$HOME"/}"
    run mkdir -p "$(dirname "$respaldo")"
    run cp -a "$destino" "$respaldo"
    info "respaldo: ~/${respaldo#"$HOME"/}"
  fi
  run install -Dm644 "$origen" "$destino"
}

# --- Detección de hardware ---

ram_mb() { awk '/MemTotal/ {print int($2 / 1024)}' /proc/meminfo; }

es_laptop() { compgen -G '/sys/class/power_supply/BAT*' >/dev/null; }

tiene_bluetooth() { [[ -d /sys/class/bluetooth ]] && [[ -n "$(ls -A /sys/class/bluetooth 2>/dev/null)" ]]; }

gpus() { lspci -mm 2>/dev/null | grep -Ei '"(VGA|3D|Display)' || true; }

# Perfil gráfico por defecto según el equipo.
perfil_sugerido() {
  local ram
  ram="$(ram_mb)"
  if ((ram < 6000)); then
    echo ligero
  elif gpus | grep -qiE 'nvidia|radeon rx|amd.*navi'; then
    echo equilibrado
  else
    echo ligero
  fi
}
