# shellcheck shell=bash
# Funciones comunes del instalador de Vortexine's OS.

# shellcheck disable=SC2034  # la usan los pasos que cargan lib.sh
VORTEX_VERSION="2.1.0"
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

# Marca como instalados «a propósito» los paquetes pedidos. Con --needed, pacman no cambia
# el motivo de instalación de uno que ya estaba como dependencia (p. ej. uv, que otro
# paquete AUR pide para compilar), y una limpieza de huérfanos lo borraría. Los grupos o
# nombres que no son un paquete instalado se ignoran.
marcar_explicitos() {
  local p
  for p in "$@"; do
    p="${p##*/}"   # quita el prefijo de repo («aur/quickshell-git»)
    srun pacman -D --asexplicit "$p" >/dev/null 2>&1 || true
  done
}

instalar_pacman() {
  (($#)) || return 0
  local pkgs
  mapfile -t pkgs < <(sin_variantes_instaladas "$@")
  ((${#pkgs[@]})) || return 0
  set -- "${pkgs[@]}"
  cambiar_jre_por_jdk "$@"
  con_reintentos srun pacman -S --needed --noconfirm "$@"
  marcar_explicitos "$@"
}

# jdk-openjdk ya trae el Java para correr programas y choca con jre-openjdk(-headless): con
# --noconfirm pacman contesta «no» a quitar el que choca y la instalación falla. Si se va a
# instalar el JDK y está el JRE de la misma versión, se cambia uno por otro (--ask 4: «sí» solo a
# quitar los paquetes en conflicto). Lo que use Java sigue funcionando: el JDK lo provee.
cambiar_jre_por_jdk() {
  local p hay=0
  for p in "$@"; do [[ "$p" == jdk-openjdk ]] && hay=1; done
  ((hay)) || return 0
  pacman -Qq jdk-openjdk >/dev/null 2>&1 && return 0
  pacman -Qq jre-openjdk >/dev/null 2>&1 || pacman -Qq jre-openjdk-headless >/dev/null 2>&1 || return 0
  info "Cambiando jre-openjdk por jdk-openjdk (el JDK trae el mismo Java y más)"
  con_reintentos srun pacman -S --needed --noconfirm --ask 4 jdk-openjdk
}

# --removemake: borra las dependencias de compilación al terminar de construir (cmake, meson,
# ninja, headers…), para no dejar cientos de MiB de herramientas que ya no hacen falta.
# Si ya tienes la app en otra variante (heroic-games-launcher en vez de -bin, foo-git...), se deja
# la tuya: con --noconfirm, pacman no quitaría la instalada y la instalación fallaría.
sin_variantes_instaladas() {
  local p base v
  for p in "$@"; do
    base="${p%-bin}"; base="${base%-git}"
    for v in "$base" "$base-bin" "$base-git"; do
      if [[ "$v" != "$p" ]] && pacman -Qq "$v" >/dev/null 2>&1; then
        info "Ya tienes $v: se queda (no se instala $p)" >&2
        continue 2
      fi
    done
    printf '%s\n' "$p"
  done
}

instalar_aur() {
  (($#)) || return 0
  local pkgs
  mapfile -t pkgs < <(sin_variantes_instaladas "$@")
  ((${#pkgs[@]})) || return 0
  set -- "${pkgs[@]}"
  con_reintentos run paru -S --needed --noconfirm --removemake "$@"
  marcar_explicitos "$@"
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
