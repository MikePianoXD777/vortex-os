# shellcheck shell=bash
paso "Escritorio: Hyprland + el shell de Vortex (Serpantinum)"

# Comandos de Vortex para todos (también para los atajos de Hyprland). Son copias y no
# enlaces: lo que está en /usr/local/bin no debe poder cambiarlo el usuario sin sudo.
for comando in vortex vortex-bench vortex-mcp vortex-gestor; do
  srun install -Dm755 "$VORTEX_DIR/bin/$comando" "/usr/local/bin/$comando"
done

# Gestor de recursos automático: prioridad a la ventana enfocada, modo juego solo al abrir
# un juego y respuesta a la presión de memoria. Su ayudante en C (escucha a Hyprland y al
# kernel sin despertar la CPU) se compila aquí; el compilador viene en base-devel (paso 10).
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run cc -O2 -s -o /usr/local/bin/vortex-eventos "$VORTEX_DIR/src/vortex-eventos.c"
else
  tmp_eventos="$(mktemp -d)"
  if cc -O2 -s -o "$tmp_eventos/vortex-eventos" "$VORTEX_DIR/src/vortex-eventos.c"; then
    srun install -Dm755 "$tmp_eventos/vortex-eventos" /usr/local/bin/vortex-eventos
  else
    aviso "No se pudo compilar vortex-eventos: el gestor de recursos automático no funcionará"
  fi
  rm -rf "$tmp_eventos"
fi
escribir_sistema /etc/systemd/user/vortex-gestor.service <"$VORTEX_DIR/config/systemd/vortex-gestor.service"
# Habilitado para este usuario: el mismo enlace que crea «systemctl --user enable», hecho a
# mano para no depender de que haya una sesión de systemd (el instalador de la ISO no la tiene).
# Lo arranca Hyprland al entrar (config/hypr/hyprland.lua); «vortex gestor apagar» lo quita.
run mkdir -p "$HOME/.config/systemd/user/graphical-session.target.wants"
run ln -sfn /etc/systemd/user/vortex-gestor.service \
  "$HOME/.config/systemd/user/graphical-session.target.wants/vortex-gestor.service"
if [[ "$VORTEX_DRY_RUN" != 1 ]]; then
  # Si ya estaba corriendo (reinstalación), que tome la versión nueva.
  systemctl --user daemon-reload 2>/dev/null && systemctl --user try-restart vortex-gestor.service 2>/dev/null || true
fi
bien "Gestor de recursos automático instalado (vortex gestor estado)"

# Audio con PipeWire: si otro paquete trae jack2 primero, choca con pipewire-jack y pacman
# no lo resuelve solo con --noconfirm.
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

# Vortex 0.1 usaba Caelestia: se quita (y su quickshell-git o noctalia-qs) para que
# entre el quickshell de los repos que usa Serpantinum.
mapfile -t viejos < <(pacman -Qq caelestia-shell caelestia-cli quickshell-git noctalia-qs 2>/dev/null || true)
if ((${#viejos[@]})); then
  info "Se quita el shell anterior: ${viejos[*]}"
  srun pacman -Rdd --noconfirm "${viejos[@]}"
fi

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/escritorio.txt")
instalar_pacman "${pkgs[@]}"

# El shell: Serpantinum con los parches de Vortex, empaquetado desde shell/ de este repo.
# makepkg instala solo las dependencias (todas de los repos oficiales).
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run makepkg -sif --noconfirm "(en $VORTEX_DIR/packaging/vortex-shell)"
else
  (cd "$VORTEX_DIR/packaging/vortex-shell" && con_reintentos makepkg -sif --noconfirm) \
    || error "No se pudo construir el shell de Vortex (packaging/vortex-shell)"
  rm -rf "$VORTEX_DIR/packaging/vortex-shell/"{pkg,src} "$VORTEX_DIR/packaging/vortex-shell/"*.pkg.tar.*
fi
bien "Shell de Vortex instalado"
# Luz nocturna (el único paquete del AUR que pide el shell).
instalar_aur wl-gammarelay-rs || aviso "No se pudo instalar wl-gammarelay-rs: la luz nocturna no funcionará"

run mkdir -p "$HOME/.config/vortex"
run xdg-user-dirs-update

# --- Config de Hyprland ---
# Caelestia dejaba ~/.config/hypr como enlace a sus dots: se guarda en el respaldo.
if [[ -L "$HOME/.config/hypr" ]]; then
  run mkdir -p "$VORTEX_RESPALDO/.config"
  run mv "$HOME/.config/hypr" "$VORTEX_RESPALDO/.config/hypr"
  info "respaldo: tu ~/.config/hypr anterior (enlace) quedó en el respaldo"
fi
copiar_config "$VORTEX_DIR/config/hypr/hyprland.lua" "$HOME/.config/hypr/hyprland.lua"

# Teclado: la config de Serpantinum fija «us» en Hyprland, así que se guarda la distribución del sistema
# (formato XKB) en ~/.config/vortex/teclado.lua; ~/.config/hypr/hyprland.lua la carga. Se toma el «X11
# Layout» de localectl y, si no hay, el KEYMAP de la consola traducido a XKB. Si no hay
# equivalencia no se escribe nada y queda el «us» de Serpantinum. Un teclado.lua que ya
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

# --- Fondos y ajustes del shell ---
fondos="$(xdg-user-dir PICTURES 2>/dev/null || echo "$HOME/Pictures")/Fondos"
run mkdir -p "$fondos"
for f in "$VORTEX_DIR"/config/fondos/*.jpg; do
  [[ -e "$fondos/${f##*/}" ]] || run cp "$f" "$fondos/"
done

# Ajustes: los de Serpantinum + la semilla de Vortex (español, carpeta de fondos). Solo si
# no existen: después se cambian desde la app de Ajustes y Vortex no los vuelve a tocar.
ajustes="$HOME/.config/serpantinum/settings.json"
if [[ -f "$ajustes" ]]; then
  info "se conservan tus ajustes del shell (~/.config/serpantinum/settings.json)"
else
  # PC de escritorio: se bloquea y apaga la pantalla, pero nunca se suspende.
  suspender=true
  es_laptop || suspender=false
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    run "crear $ajustes (semilla de Vortex, suspender=$suspender)"
  else
    mkdir -p "$(dirname "$ajustes")"
    jq -s --arg fondos "$fondos" --argjson suspender "$suspender" \
      '(.[0] * .[1]) | .wallpaperDir = $fondos | .idle.actions.suspend.enabled = $suspender' \
      /usr/share/serpantinum/config/serpantinum/settings.json "$VORTEX_DIR/config/serpantinum/semilla.json" \
      >"$ajustes"
  fi
  bien "Ajustes del shell creados (español; suspender en inactividad: $suspender)"
fi

# Sin la bienvenida de Serpantinum (reproduce un audio de 44 MB); el primer fondo lo pone
# «vortex primer-inicio» al entrar por primera vez.
run mkdir -p "$HOME/.local/state/serpantinum"
run touch "$HOME/.local/state/serpantinum/first_launch.done"

# Terminal (kitty) con los colores que genera matugen a partir del fondo.
[[ -e "$HOME/.config/kitty/kitty.conf" ]] || copiar_config /usr/share/serpantinum/config/kitty/kitty.conf "$HOME/.config/kitty/kitty.conf"

# El panel del sistema cambia el perfil de energía (rendimiento/equilibrado/ahorro).
# Con «tuned» (archinstall) el perfil lo da tuned-ppd y no existe esa unidad.
if systemctl list-unit-files power-profiles-daemon.service 2>/dev/null | grep -q '^power-profiles-daemon'; then
  srun systemctl enable power-profiles-daemon.service
else
  info "power-profiles-daemon no está instalado (¿tuned-ppd?); se deja el control de energía como esté"
fi
