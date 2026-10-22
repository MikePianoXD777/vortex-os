# shellcheck shell=bash
paso "Últimos detalles"

if [[ "$VORTEX_DRY_RUN" != 1 ]]; then
  mkdir -p "$HOME/.config/vortex"
  echo "$VORTEX_VERSION" >"$HOME/.config/vortex/version"
fi

[[ -d "$VORTEX_RESPALDO" ]] && info "Tus configs anteriores quedaron en ~/${VORTEX_RESPALDO#"$HOME"/}"
info "Registro completo: $VORTEX_LOG"

printf '\n%s¡Vortex OS instalado!%s\n' "$C_VERDE" "$C_FIN"
cat <<'EOF'
    Después de reiniciar:
      vortex bench               mide RAM, servicios y arranque
      vortex gestor estado       qué está priorizando el gestor de recursos
      vortex fondo ARCHIVO       cambia el fondo (y los colores)
      Super+A                    habla con Claude
      Super+G                    modo juego a mano
EOF

# Sin preguntar (--si, curl | bash o sin terminal) no se reinicia solo: el equipo puede
# estar en uso y perder trabajo abierto. El reinicio automático solo se ofrece con terminal.
reiniciar_defecto=s
[[ "$VORTEX_SI" == 1 || ! -t 0 ]] && reiniciar_defecto=n
if [[ "$VORTEX_DRY_RUN" != 1 ]] && preguntar "¿Reiniciar ahora?" "$reiniciar_defecto"; then
  sudo systemctl reboot
elif [[ "$reiniciar_defecto" == n ]]; then
  info "Reinicia cuando puedas para entrar a Vortex OS."
fi
