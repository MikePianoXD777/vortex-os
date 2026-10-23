# shellcheck shell=bash
if ((!VORTEX_CLAUDE)) || ! preguntar "¿Conectar Vortexine's OS con Claude (necesita tu cuenta de Claude; también se activa después desde Ajustes)?" n; then
  info "Claude: omitido"
  return 0
fi

paso "Claude"

# SoX es el respaldo para grabar la voz si el módulo de audio de Claude Code no carga en Linux.
instalar_pacman sox

# Claude Code para tu usuario, el servidor MCP «vortex» y la voz en español: install/claude.sh,
# lo mismo que corre «vortex claude instalar» (y la ISO, en el primer inicio de cada usuario).
# shellcheck source=install/claude.sh
source "$VORTEX_DIR/install/claude.sh"
claude_instalar_usuario && { mkdir -p "${XDG_STATE_HOME:-$HOME/.local/state}/vortex"; : >"${XDG_STATE_HOME:-$HOME/.local/state}/vortex/claude-listo"; } || aviso "Claude quedó a medias; cuando tengas internet, corre: vortex claude instalar"

info "Super+A abre Claude. Puede ver tu pantalla y ventanas solo cuando se lo pides,"
info "y te avisa con una notificación cada vez. Para cortarle el acceso: vortex claude pausa"
