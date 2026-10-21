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
      vortex perfil equilibrado  cambia el perfil gráfico
      Super+A                    habla con Claude
      Super+G                    modo juego a mano
EOF

if [[ "$VORTEX_DRY_RUN" != 1 ]] && preguntar "¿Reiniciar ahora?" s; then
  sudo systemctl reboot
fi
