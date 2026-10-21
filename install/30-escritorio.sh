# shellcheck shell=bash
paso "Escritorio: Hyprland + Caelestia"

# Comandos de Vortex disponibles para todos (también para los atajos de Hyprland).
srun ln -sf "$VORTEX_DIR/bin/vortex" /usr/local/bin/vortex
srun ln -sf "$VORTEX_DIR/bin/vortex-bench" /usr/local/bin/vortex-bench
srun ln -sf "$VORTEX_DIR/bin/vortex-mcp" /usr/local/bin/vortex-mcp

# Audio con PipeWire antes que Caelestia: si otro paquete trae jack2 primero, choca con
# pipewire-jack y pacman no lo resuelve solo con --noconfirm.
if pacman -Qq jack2 >/dev/null 2>&1; then
  srun pacman -Rdd --noconfirm jack2
fi
# Lo mismo con PulseAudio (p. ej. archinstall con «Audio: pulseaudio»): pipewire-pulse choca
# con él y, con --noconfirm, pacman responde «no» a «¿Eliminar pulseaudio?».
mapfile -t pulse < <(pacman -Qq pulseaudio pulseaudio-bluetooth pulseaudio-alsa pulseaudio-jack \
  pulseaudio-zeroconf pulseaudio-lirc pulseaudio-rtp pulseaudio-equalizer 2>/dev/null || true)
if ((${#pulse[@]})); then
  info "Se reemplaza PulseAudio por PipeWire: ${pulse[*]}"
  srun pacman -Rdd --noconfirm "${pulse[@]}"
fi
instalar_pacman pipewire pipewire-pulse pipewire-alsa pipewire-jack wireplumber

# Caelestia pide quickshell-git. El repo de CachyOS trae noctalia-qs, un fork que dice
# «proveer» quickshell-git, y paru lo prefiere por venir de un repo; pero se compila contra
# otra versión de Qt y el shell no arranca. Por eso se pide el del AUR por nombre completo.
if pacman -Qq noctalia-qs >/dev/null 2>&1; then
  srun pacman -Rdd --noconfirm noctalia-qs
fi
instalar_aur aur/quickshell-git
instalar_aur caelestia-shell caelestia-cli

run mkdir -p "$HOME/.config/vortex"

# Si ya había una Caelestia configurada (monitores, variables, ajustes del shell), Vortex
# solo carga lo que esté en ~/.config/vortex; se migra antes de que algo lo reemplace.
# Los archivos de serie de Caelestia no cuentan: hypr-user.lua trae solo comentarios y
# hypr-vars.lua se crea con «return {}» al arrancar Hyprland.
migrar_config() {
  local origen="$1" destino="$2" nuevo="$3" util
  [[ -f "$origen" && ! -e "$destino" ]] || return 0
  util=$(grep -vE '^[[:space:]]*(--.*)?$' "$origen" || true)
  [[ -n "$util" ]] || return 0
  if ! grep -qvE '^[[:space:]]*return[[:space:]]*\{[[:space:]]*\}[[:space:]]*;?[[:space:]]*$' <<<"$util"; then
    return 0
  fi
  if [[ -n "${4:-}" ]] && cmp -s "$origen" "$4"; then return 0; fi
  run cp -a "$origen" "$destino"
  info "se conservó tu ${origen##*/} como ~/.config/vortex/$nuevo"
}
if [[ ! -e "$HOME/.config/vortex/perfil" ]]; then
  migrar_config "$HOME/.config/caelestia/hypr-user.lua" "$HOME/.config/vortex/hypr-mio.lua" hypr-mio.lua \
    "$VORTEX_DIR/config/caelestia/hypr-user.lua"
  migrar_config "$HOME/.config/caelestia/hypr-vars.lua" "$HOME/.config/vortex/hypr-vars-mios.lua" hypr-vars-mios.lua \
    "$VORTEX_DIR/config/caelestia/hypr-vars.lua"
  migrar_config "$HOME/.config/caelestia/shell.json" "$HOME/.config/vortex/shell-mio.json" shell-mio.json
fi

# «caelestia install --noconfirm» copia TODO ~/.config a ~/.config.bak y se cae si hay un
# socket o un archivo ilegible. Se hace un respaldo propio de lo que Caelestia despliega y se
# deja un ~/.config.bak vacío para que el suyo se omita («ya existe, ¿sobrescribir?» = no).
run mkdir -p "$VORTEX_RESPALDO/.config"
for d in hypr fish foot fastfetch btop Thunar starship.toml caelestia; do
  if [[ -e "$HOME/.config/$d" ]]; then
    run cp -a "$HOME/.config/$d" "$VORTEX_RESPALDO/.config/" || aviso "No se pudo respaldar ~/.config/$d por completo"
  fi
done
respaldo_vacio=0
if [[ ! -e "$HOME/.config.bak" ]]; then
  run mkdir "$HOME/.config.bak"
  respaldo_vacio=1
fi

# El CLI de Caelestia instala Hyprland, foot, fish, PipeWire, NetworkManager, temas, etc.
# Se quita «micro» porque Vortex ya trae editores. Es repetible (--needed), así que se reintenta.
if ! con_reintentos run caelestia install --aur-helper paru --disable-components micro --noconfirm; then
  if ((respaldo_vacio)); then rmdir "$HOME/.config.bak" 2>/dev/null || true; fi
  error "«caelestia install» falló tras 3 intentos (red o espejos); vuelve a ejecutar el instalador"
fi
if ((respaldo_vacio)); then run rmdir "$HOME/.config.bak" 2>/dev/null || true; fi
bien "Caelestia instalado"

copiar_config "$VORTEX_DIR/config/caelestia/hypr-vars.lua" "$HOME/.config/caelestia/hypr-vars.lua"
copiar_config "$VORTEX_DIR/config/caelestia/hypr-user.lua" "$HOME/.config/caelestia/hypr-user.lua"
# Teclado: Caelestia fija «us» en Hyprland, así que se guarda la distribución del sistema
# (formato XKB) en ~/.config/vortex/teclado.lua; hypr-user.lua la carga. Se toma el «X11
# Layout» de localectl y, si no hay, el KEYMAP de la consola traducido a XKB. Si no hay
# equivalencia no se escribe nada y queda el «us» de Caelestia. Un teclado.lua que ya
# exista (p. ej. cambiado a mano) no se toca.
teclado_xkb() {
  local layout="" variant="" keymap=""
  if tiene localectl; then
    layout=$(localectl status 2>/dev/null | awk -F': *' '/X11 Layout/ {print $2; exit}' || true)
    variant=$(localectl status 2>/dev/null | awk -F': *' '/X11 Variant/ {print $2; exit}' || true)
  fi
  [[ "$layout" == "(unset)" || "$layout" == "n/a" ]] && layout="" variant=""
  if [[ -z "$layout" && -r /etc/vconsole.conf ]]; then
    keymap=$(sed -n 's/^KEYMAP=//p' /etc/vconsole.conf | tail -n1 | tr -d "\"'")
    # Primero la tabla oficial de systemd (la que usa localed): trae también las variantes
    # (us-acentos = us/intl, de-latin1-nodeadkeys = de/nodeadkeys...). «-» es «sin variante».
    if [[ -n "$keymap" && -r /usr/share/systemd/kbd-model-map ]]; then
      read -r layout variant < <(awk -v k="$keymap" '$1 == k && $2 != "" {print $2, $4; exit}' \
        /usr/share/systemd/kbd-model-map 2>/dev/null || true) || true
      [[ "$variant" == "-" ]] && variant=""
    fi
    # Respaldo para los que la tabla no tenga (o si no existe el archivo).
    [[ -n "$layout" ]] || case "$keymap" in
      us|us-acentos) layout=us ;;
      la-latin1) layout=latam ;;
      es|es-*) layout=es ;;
      de|de-latin1|de-latin1-nodeadkeys) layout=de ;;
      uk) layout=gb ;;
      fr|fr-latin1|fr-latin9|fr-pc) layout=fr ;;
      it|it2) layout=it ;;
      pt-latin1|pt-latin9) layout=pt ;;
      br-abnt2|br-abnt) layout=br ;;
      ru|ru4) layout=ru ;;
      pl|pl2) layout=pl ;;
      cz|cz-qwertz|cz-lat2) layout=cz ;;
      sv-latin1) layout=se ;;
      no|no-latin1) layout=no ;;
      dk|dk-latin1) layout=dk ;;
      fi|fi-latin1|fi-latin9) layout=fi ;;
      nl|nl2) layout=nl ;;
      be-latin1) layout=be ;;
      tr_q-latin5) layout=tr ;;
      cf) layout=ca ;;
      dvorak) layout=us variant=dvorak ;;
      colemak) layout=us variant=colemak ;;
    esac
  fi
  # Solo caracteres válidos de XKB: el valor termina dentro de un archivo Lua.
  [[ "$layout" =~ ^[A-Za-z0-9_,]+$ ]] || return 1
  [[ "$variant" =~ ^[A-Za-z0-9_,-]*$ ]] || variant=""
  TECLADO_LAYOUT="$layout" TECLADO_VARIANTE="$variant" TECLADO_DE_CONSOLA="$keymap"
}
if [[ -e "$HOME/.config/vortex/teclado.lua" ]]; then
  info "se conserva tu ~/.config/vortex/teclado.lua"
elif teclado_xkb; then
  if [[ -n "$TECLADO_VARIANTE" ]]; then
    run install -Dm644 /dev/stdin "$HOME/.config/vortex/teclado.lua" \
      <<<"return { layout = \"$TECLADO_LAYOUT\", variant = \"$TECLADO_VARIANTE\" }"
  else
    run install -Dm644 /dev/stdin "$HOME/.config/vortex/teclado.lua" <<<"return \"$TECLADO_LAYOUT\""
  fi
  bien "Teclado de Hyprland: ${TECLADO_LAYOUT}${TECLADO_VARIANTE:+ ($TECLADO_VARIANTE)}"
  # Si solo se supo por la consola, se fija también en X11 para tuigreet y las apps X11.
  # --no-convert: sin eso localectl reescribe el KEYMAP de /etc/vconsole.conf, que ya está bien.
  if [[ -n "$TECLADO_DE_CONSOLA" ]] && tiene localectl; then
    srun localectl --no-convert set-x11-keymap "$TECLADO_LAYOUT" "" "$TECLADO_VARIANTE" \
      || aviso "No se pudo fijar la distribución X11 del teclado (localectl)"
  fi
else
  info "no se reconoció la distribución del teclado; Hyprland queda con «us» (cámbiala en ~/.config/vortex/teclado.lua)"
fi

# «vortex perfil» reescribe shell.json; el que hubiera se guarda en el respaldo.
if [[ -f "$HOME/.config/caelestia/shell.json" && ! -f "$HOME/.config/vortex/shell-mio.json" ]]; then
  run mkdir -p "$VORTEX_RESPALDO/.config/caelestia"
  run cp -a "$HOME/.config/caelestia/shell.json" "$VORTEX_RESPALDO/.config/caelestia/shell.json"
fi
run "$VORTEX_DIR/bin/vortex" perfil "$VORTEX_PERFIL" --sin-recargar
bien "Perfil «$VORTEX_PERFIL» aplicado (cámbialo con: vortex perfil ligero|equilibrado|bonito)"

if es_laptop; then
  # Con «tuned» (archinstall) el perfil de energía lo da tuned-ppd y no existe esa unidad.
  if systemctl list-unit-files power-profiles-daemon.service 2>/dev/null | grep -q '^power-profiles-daemon'; then
    srun systemctl enable power-profiles-daemon.service
  else
    info "power-profiles-daemon no está instalado (¿tuned-ppd?); se deja el control de energía como esté"
  fi
fi
