# shellcheck shell=bash
if ((!VORTEX_CLAUDE)) || ! preguntar "¿Conectar Vortex con Claude (Claude Code, hablarle con el micrófono y ver tu pantalla con permiso)?" s; then
  info "Claude: omitido"
  return 0
fi

paso "Claude"

# El instalador oficial lo deja en ~/.local/bin, que puede no estar en el PATH todavía.
if tiene claude || [[ -x "$HOME/.local/bin/claude" ]]; then
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

# Hablarle a Claude con el micrófono: el dictado de Claude Code (/voice) en modo «toque»
# (Espacio para empezar a hablar, Espacio otra vez para enviar) y en español. La voz se
# transcribe en los servidores de Anthropic con tu cuenta de Claude.ai y no gasta tokens.
# SoX es el respaldo para grabar si el módulo de audio de Claude Code no carga en Linux.
# Si ya tenías ajustes de voz o de idioma, se respetan.
instalar_pacman sox
ajustes_claude="$HOME/.claude/settings.json"
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run "activar el dictado por voz en español en $ajustes_claude"
else
  mkdir -p "$(dirname "$ajustes_claude")"
  [[ -s "$ajustes_claude" ]] || echo '{}' >"$ajustes_claude"
  if jq -e . "$ajustes_claude" >/dev/null 2>&1; then
    tmp_ajustes="$(mktemp)"
    jq '.voice //= {"enabled": true, "mode": "tap"} | .language //= "spanish"' "$ajustes_claude" >"$tmp_ajustes" \
      && mv "$tmp_ajustes" "$ajustes_claude"
    bien "Voz activada: en Claude, toca Espacio, habla y toca Espacio otra vez para enviar"
  else
    aviso "No pude leer $ajustes_claude (¿JSON roto?); activa la voz en Claude con /voice tap"
  fi
fi

info "Super+A abre Claude. Puede ver tu pantalla y ventanas solo cuando se lo pides,"
info "y te avisa con una notificación cada vez. Para cortarle el acceso: vortex claude pausa"
