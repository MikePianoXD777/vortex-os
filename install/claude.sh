# shellcheck shell=bash
# Claude Code para el usuario actual (sin sudo ni root): el instalador oficial en ~/.local/bin,
# el servidor MCP «vortex» y el dictado por voz en español.
#
# Lo comparten install/60-claude.sh (install.sh) y «vortex claude instalar»: la ISO lo corre en
# segundo plano en el primer inicio de cada usuario que eligió Claude en el instalador gráfico
# (bin/vortex-sesion). Se carga con source después de install/lib.sh, como install/usuario.sh.
# Es idempotente: lo que ya estaba hecho se respeta, y se puede repetir hasta que salga todo.
#
# Para probarlo sin red: VORTEX_CLAUDE_INSTALADOR=file:///ruta/install.sh

VORTEX_CLAUDE_INSTALADOR="${VORTEX_CLAUDE_INSTALADOR:-https://claude.ai/install.sh}"

# ¿Hay red para bajar a Claude? (llega al instalador oficial)
claude_hay_red() {
  curl -fsS -o /dev/null --connect-timeout 8 --max-time 15 "$VORTEX_CLAUDE_INSTALADOR" 2>/dev/null
}

# Devuelve 0 si Claude quedó instalado y conectado con Vortex; 1 si no se pudo (sin red, el
# instalador falló...). Nunca aborta quien la llama: cada paso se revisa a mano porque, llamada
# con «||», bash ignora set -e dentro de la función.
claude_instalar_usuario() {
  local claude_bin tmp ajustes tmp_ajustes

  # El instalador oficial lo deja en ~/.local/bin, que puede no estar en el PATH todavía.
  if tiene claude || [[ -x "$HOME/.local/bin/claude" ]]; then
    bien "Claude Code ya está instalado"
  else
    # Instalador oficial de Anthropic: deja «claude» en ~/.local/bin. Se baja a un archivo y
    # luego se corre (no «curl | bash»): si la descarga se corta, no se ejecuta nada a medias.
    tmp="$(mktemp "${TMPDIR:-/tmp}/vortex-claude.XXXXXX")" || return 1
    if run curl -fsSL --connect-timeout 15 --max-time 120 -o "$tmp" "$VORTEX_CLAUDE_INSTALADOR" \
      && run timeout 900 bash "$tmp"; then
      rm -f "$tmp"
    else
      rm -f "$tmp"
      aviso "No pude instalar Claude Code (¿sin internet?); se puede repetir con: vortex claude instalar"
      return 1
    fi
  fi

  claude_bin="$(command -v claude || echo "$HOME/.local/bin/claude")"
  if [[ "$VORTEX_DRY_RUN" != 1 && ! -x "$claude_bin" ]]; then
    aviso "El instalador de Claude terminó, pero no encuentro «claude» en ~/.local/bin"
    return 1
  fi

  # Registra el servidor MCP de Vortex para todos tus proyectos (alcance «user»).
  if [[ "$VORTEX_DRY_RUN" != 1 ]] && "$claude_bin" mcp get vortex >/dev/null 2>&1; then
    bien "El servidor MCP «vortex» ya estaba registrado"
  elif run "$claude_bin" mcp add --scope user vortex -- /usr/local/bin/vortex-mcp; then
    bien "Servidor MCP «vortex» registrado"
  else
    aviso "No pude registrar el servidor MCP «vortex»; se puede repetir con: vortex claude instalar"
    return 1
  fi

  # Hablarle a Claude con el micrófono: el dictado de Claude Code (/voice) en modo «toque»
  # (Espacio para empezar a hablar, Espacio otra vez para enviar) y en español. La voz se
  # transcribe en los servidores de Anthropic con tu cuenta de Claude.ai y no gasta tokens.
  # SoX (lo instala quien llama: install/60-claude.sh o el paquete vortex-claude) es el respaldo
  # para grabar si el módulo de audio de Claude Code no carga en Linux.
  # Si ya tenías ajustes de voz o de idioma, se respetan.
  ajustes="$HOME/.claude/settings.json"
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    run "activar el dictado por voz en español en $ajustes"
  else
    mkdir -p "$(dirname "$ajustes")"
    [[ -s "$ajustes" ]] || echo '{}' >"$ajustes"
    if jq -e . "$ajustes" >/dev/null 2>&1; then
      tmp_ajustes="$(mktemp "${TMPDIR:-/tmp}/vortex-claude-ajustes.XXXXXX")" || return 1
      if jq '.voice //= {"enabled": true, "mode": "tap"} | .language //= "spanish"' "$ajustes" >"$tmp_ajustes"; then
        cat "$tmp_ajustes" >"$ajustes"   # cat y no mv: conserva permisos y enlaces del archivo
        bien "Voz activada: en Claude, toca Espacio, habla y toca Espacio otra vez para enviar"
      else
        aviso "No pude cambiar $ajustes; activa la voz en Claude con /voice tap"
      fi
      rm -f "$tmp_ajustes"
    else
      aviso "No pude leer $ajustes (¿JSON roto?); activa la voz en Claude con /voice tap"
    fi
  fi
  return 0
}
