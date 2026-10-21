# shellcheck shell=bash
if ((!VORTEX_CLAUDE)) || ! preguntar "¿Conectar Vortex con Claude (Claude Code + ver tu pantalla con permiso)?" s; then
  info "Claude: omitido"
  return 0
fi

paso "Claude"

if tiene claude; then
  bien "Claude Code ya está instalado"
else
  # Instalador oficial de Anthropic: deja «claude» en ~/.local/bin.
  run bash -c 'curl -fsSL https://claude.ai/install.sh | bash'
fi

claude_bin="$(command -v claude || echo "$HOME/.local/bin/claude")"

# Registra el servidor MCP de Vortex para todos tus proyectos (alcance «user»).
if [[ "$VORTEX_DRY_RUN" != 1 ]] && "$claude_bin" mcp get vortex >/dev/null 2>&1; then
  bien "El servidor MCP «vortex» ya estaba registrado"
else
  run "$claude_bin" mcp add --scope user vortex -- /usr/local/bin/vortex-mcp
  bien "Servidor MCP «vortex» registrado"
fi

info "Super+A abre Claude. Puede ver tu pantalla y ventanas solo cuando se lo pides,"
info "y te avisa con una notificación cada vez. Para cortarle el acceso: vortex claude pausa"
