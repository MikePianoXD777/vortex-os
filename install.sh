#!/usr/bin/env bash
# Instalador de Vortexine's OS: convierte un Arch Linux recién instalado en Vortexine's OS.
#
#   ./install.sh                  instalación normal (pregunta lo opcional)
#   ./install.sh --prueba         muestra lo que haría, sin tocar nada
#   ./install.sh --si             acepta todo lo que viene por defecto
#   ./install.sh --sin-juegos --sin-dev --sin-claude --sin-miku
#   ./install.sh --inicio-automatico   entra al escritorio sin pedir contraseña
set -Eeuo pipefail

VORTEX_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export VORTEX_DIR

VORTEX_JUEGOS=1
VORTEX_DEV=1
VORTEX_CLAUDE=1
VORTEX_MIKU=1
VORTEX_AUTOLOGIN=0

uso() { sed -n '2,8p' "$0" | sed 's/^# \{0,1\}//'; }

while (($#)); do
  case "$1" in
    --prueba | --dry-run) export VORTEX_DRY_RUN=1 ;;
    --si | -y) export VORTEX_SI=1 ;;
    --sin-juegos) VORTEX_JUEGOS=0 ;;
    --sin-dev) VORTEX_DEV=0 ;;
    --sin-claude) VORTEX_CLAUDE=0 ;;
    --sin-miku) VORTEX_MIKU=0 ;;
    --inicio-automatico) VORTEX_AUTOLOGIN=1 ;;
    -h | --ayuda | --help) uso; exit 0 ;;
    *) echo "Opción desconocida: $1" >&2; uso; exit 1 ;;
  esac
  shift
done

# shellcheck source=install/lib.sh
source "$VORTEX_DIR/install/lib.sh"

VORTEX_RESPALDO="$HOME/.config/vortex/respaldo-$(date +%Y%m%d-%H%M%S)"
export VORTEX_RESPALDO VORTEX_JUEGOS VORTEX_DEV VORTEX_CLAUDE VORTEX_MIKU VORTEX_AUTOLOGIN

# Dónde falló: archivo y línea, y la línea del paso que lo originó, buscándola en
# la pila de llamadas (el fallo puede estar dentro de una función de lib.sh,
# como run o instalar_pacman).
donde() {
  local i
  # Si el fallo ya está en el propio paso, el archivo y la línea bastan.
  [[ -n "${paso_script:-}" && "${1:-}" == "$paso_script" ]] && return 0
  for ((i = 1; i < ${#BASH_SOURCE[@]}; i++)); do
    if [[ "${BASH_SOURCE[i]}" == "${paso_script:-}" ]]; then
      printf ' (%s:%s)' "${paso_script##*/}" "${BASH_LINENO[i-1]}"
      return
    fi
  done
  [[ -n "${paso_script:-}" ]] && printf ' (paso %s)' "${paso_script##*/}"
  return 0
}

# Recibe el código de salida, el comando, la línea y el archivo del fallo (se
# pasan desde la trampa porque los pasos y lib.sh se cargan con source, así que
# el número de línea solo no basta). Si el comando es el «"$@"» de run, el
# comando real está en la última línea «$» del log.
al_fallar() {
  local rc="$1" cmd="$2" linea="$3" archivo="$4" nota=""
  if [[ "$cmd" == *'"$@"'* ]]; then
    nota=" (el comando completo está en la última línea «\$» del log)"
  fi
  error "Falló «$cmd»$nota (código $rc) en ${archivo##*/}:$linea$(donde "$archivo"). Revisa $VORTEX_LOG"
}
trap 'al_fallar "$?" "$BASH_COMMAND" "$LINENO" "${BASH_SOURCE[0]}"' ERR

printf '%s' "$C_AZUL"
cat <<'EOF'
 __     __         _              ___  ____
 \ \   / /__  _ __| |_ _____  __ / _ \/ ___|
  \ \ / / _ \| '__| __/ _ \ \/ /| | | \___ \
   \ V / (_) | |  | ||  __/>  < | |_| |___) |
    \_/ \___/|_|   \__\___/_/\_\ \___/|____/
EOF
printf '%s  v%s  ·  ligero, para jugar y programar\n' "$C_FIN" "$VORTEX_VERSION"
[[ "$VORTEX_DRY_RUN" == 1 ]] && aviso "MODO PRUEBA: no se instala ni se cambia nada."

for paso_script in "$VORTEX_DIR"/install/[0-9][0-9]-*.sh; do
  # shellcheck source=/dev/null
  source "$paso_script"
done
