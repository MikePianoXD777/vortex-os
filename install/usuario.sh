# shellcheck shell=bash
# Config de Vortex para el usuario actual: Hyprland, teclado, fondos, ajustes del shell, kitty y
# el gestor de recursos. No instala paquetes ni toca el sistema (salvo fijar la distribución
# X11 del teclado si solo se conoce la de la consola).
#
# La carga install/30-escritorio.sh, y «vortex configurar-usuario» la corre sola: así la ISO
# configura a cada usuario nuevo en su primer inicio, con su idioma (la carpeta de fondos es
# «Imágenes» o «Pictures» según el idioma) y su teclado.

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
# Los atajos de teclado (los de Mike en Caelestia, traducidos a Serpantinum) van en su propio archivo,
# que hyprland.lua carga: tienen que ir siempre juntos.
copiar_config "$VORTEX_DIR/config/hypr/atajos.lua" "$HOME/.config/hypr/atajos.lua"

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
      fi|fi-latin1|fi-latin9) layout="fi" ;;
      nl|nl2) layout="nl" ;;
      be-latin1) layout=be ;;
      tr_q-latin5) layout="tr" ;;
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
# Los .mp4 son fondos animados (Gargantua): se ven con Ajustes > Tema > Fondos animados.
for f in "$VORTEX_DIR"/config/fondos/*.{jpg,mp4}; do
  [[ -e "$f" && ! -e "$fondos/${f##*/}" ]] || continue
  run cp "$f" "$fondos/"
done

# El saludo de la terminal: «Vortexine's OS» en morado, el logo y los componentes (bash y fish).
run bash "$VORTEX_DIR/config/terminal/poner-saludo.sh"

# Dónde está el shell instalado (otra ruta solo para pruebas).
serpantinum="${VORTEX_SERPANTINUM:-/usr/share/serpantinum}"

# Idioma del shell: el del sistema, o sea el que se eligió al instalar (LANG en /etc/locale.conf),
# si Serpantinum lo trae traducido; si no, español. Se lee el archivo y no el entorno de la sesión:
# es lo que fijó el instalador y no depende de cómo se inició la sesión. El live usa
# LANG=C.UTF-8 (no es un idioma), así que ahí sale español. Los nombres son los de I18n.qml
# del shell: el código ISO 639-1 (en, pt, fr...), «zh_tw» para el chino de Taiwán, Hong Kong
# y Macao, y «ua» para el ucraniano (uk).
idioma_del_shell() {
  local conf="${VORTEX_LOCALE_CONF:-/etc/locale.conf}"
  local idiomas="$serpantinum/src/assets/languages"
  local lang idioma pais
  lang=$(sed -n 's/^LANG=//p' "$conf" 2>/dev/null | tail -n1 | tr -d "\"'")
  lang="${lang%%.*}"
  lang="${lang%%@*}"
  idioma="${lang%%_*}"
  pais="${lang#*_}"
  idioma="${idioma,,}"
  case "$idioma" in
    zh) case "${pais^^}" in TW | HK | MO) idioma=zh_tw ;; esac ;;
    uk) idioma=ua ;;
  esac
  # Solo letras y guion bajo (termina en un nombre de archivo) y que el shell tenga el archivo.
  if [[ "$idioma" =~ ^[a-z_]+$ && -f "$idiomas/$idioma.json" ]]; then
    printf '%s\n' "$idioma"
  else
    printf 'es\n'
  fi
}

# Ajustes: los de Serpantinum + la semilla de Vortex (carpeta de fondos, «No molestar» apagado:
# el settings.json de Serpantinum lo trae encendido y ninguna notificación saldría) y el idioma
# del sistema (la semilla trae «es» solo como respaldo). Sin los monitores del autor (trae
# «eDP-1» apagado). Solo si no existen: después se cambian desde la app de Ajustes y Vortex no
# los vuelve a tocar.
ajustes="$HOME/.config/serpantinum/settings.json"
if [[ -f "$ajustes" ]]; then
  info "se conservan tus ajustes del shell (~/.config/serpantinum/settings.json)"
else
  # PC de escritorio: se bloquea y apaga la pantalla, pero nunca se suspende.
  suspender=true
  es_laptop || suspender=false
  idioma="$(idioma_del_shell)"
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    run "crear $ajustes (semilla de Vortex, idioma=$idioma, suspender=$suspender)"
  else
    mkdir -p "$(dirname "$ajustes")"
    # En el USB (live) no hay inactividad: ni bloqueo ni pantalla apagada a mitad de la instalación.
    inactividad=true
    [[ -e "${VORTEX_MARCA_LIVE:-/etc/vortex/live}" ]] && inactividad=false
    jq -s --arg fondos "$fondos" --argjson suspender "$suspender" --arg idioma "$idioma" --argjson inactividad "$inactividad" \
      '(.[0] * .[1]) | .wallpaperDir = $fondos | .general.language = $idioma | .idle.actions.suspend.enabled = $suspender | (if $inactividad then . else .idle.enabled = false end) | .display.monitors = {}' \
      "$serpantinum/config/serpantinum/settings.json" "$VORTEX_DIR/config/serpantinum/semilla.json" \
      >"$ajustes"
  fi
  bien "Ajustes del shell creados (idioma: $idioma; suspender en inactividad: $suspender)"
fi

# Sin la bienvenida de Serpantinum (reproduce un audio de 44 MB); el primer fondo lo pone
# «vortex primer-inicio» al entrar por primera vez.
run mkdir -p "$HOME/.local/state/serpantinum"
run touch "$HOME/.local/state/serpantinum/first_launch.done"

# Letra de las apps GTK al 90 %, a tono con el shell más compacto (1.2). Si no hay sesión de
# D-Bus todavía, se salta sin error.
if command -v gsettings >/dev/null; then
  run gsettings set org.gnome.desktop.interface text-scaling-factor 0.9 2>/dev/null || true
fi

# Terminal (kitty) con los colores que genera matugen a partir del fondo.
[[ -e "$HOME/.config/kitty/kitty.conf" ]] || copiar_config "$serpantinum/config/kitty/kitty.conf" "$HOME/.config/kitty/kitty.conf"

# Habilitado para este usuario: el mismo enlace que crea «systemctl --user enable», hecho a
# mano para no depender de que haya una sesión de systemd (el instalador de la ISO no la tiene).
# Lo arranca Hyprland al entrar (config/hypr/hyprland.lua); «vortex gestor apagar» lo quita.
run mkdir -p "$HOME/.config/systemd/user/graphical-session.target.wants"
run ln -sfn /etc/systemd/user/vortex-gestor.service \
  "$HOME/.config/systemd/user/graphical-session.target.wants/vortex-gestor.service"

# mise (si está: lo trae lo de programación) activa la versión de node/python/go de cada
# proyecto. 50-dev.sh hace lo mismo al instalar; aquí es para los usuarios nuevos de la ISO.
if tiene mise && ! grep -qs 'mise activate' "$HOME/.bashrc"; then
  if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
    run "echo 'mise activate bash' >> $HOME/.bashrc"
  else
    printf '\n# Vortexine'\''s OS\ncommand -v mise >/dev/null && eval "$(mise activate bash)"\n' >>"$HOME/.bashrc"
  fi
fi
