#!/usr/bin/env bash
# Instalador de Vortex OS: convierte un Arch Linux recién instalado en Vortex OS.
#
#   ./install.sh                  instalación normal (pregunta lo opcional)
#   ./install.sh --prueba         muestra lo que haría, sin tocar nada
#   ./install.sh --si             acepta todo lo que viene por defecto
#   ./install.sh --perfil ligero  ligero | equilibrado | bonito
#   ./install.sh --sin-juegos --sin-dev --sin-claude
#   ./install.sh --inicio-automatico   entra al escritorio sin pedir contraseña
set -Eeuo pipefail

VORTEX_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export VORTEX_DIR

VORTEX_JUEGOS=1
VORTEX_DEV=1
VORTEX_CLAUDE=1
VORTEX_PERFIL=""
VORTEX_AUTOLOGIN=0

uso() { sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; }

while (($#)); do
  case "$1" in
    --prueba | --dry-run) export VORTEX_DRY_RUN=1 ;;
    --si | -y) export VORTEX_SI=1 ;;
    --perfil) VORTEX_PERFIL="${2:?falta el perfil}"; shift ;;
    --sin-juegos) VORTEX_JUEGOS=0 ;;
    --sin-dev) VORTEX_DEV=0 ;;
    --sin-claude) VORTEX_CLAUDE=0 ;;
    --inicio-automatico) VORTEX_AUTOLOGIN=1 ;;
    -h | --ayuda | --help) uso; exit 0 ;;
    *) echo "Opción desconocida: $1" >&2; uso; exit 1 ;;
  esac
  shift
done

# shellcheck source=install/lib.sh
source "$VORTEX_DIR/install/lib.sh"

VORTEX_RESPALDO="$HOME/.config/vortex/respaldo-$(date +%Y%m%d-%H%M%S)"
export VORTEX_RESPALDO VORTEX_JUEGOS VORTEX_DEV VORTEX_CLAUDE VORTEX_PERFIL VORTEX_AUTOLOGIN

trap 'error "Falló en la línea $LINENO. Revisa $VORTEX_LOG"' ERR

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
