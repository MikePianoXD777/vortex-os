# shellcheck shell=bash
paso "Escritorio: Hyprland + el shell de Vortex (Serpantinum)"

# Comandos de Vortex para todos (también para los atajos de Hyprland). Son copias y no
# enlaces: lo que está en /usr/local/bin no debe poder cambiarlo el usuario sin sudo.
for comando in vortex vortex-bench vortex-mcp vortex-gestor vortex-otaku vortex-pantallas vortex-limpiar vortex-captura vortex-usb vortex-texto vortex-arreglar vortex-onedrive vortex-restore vortex-mods; do
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

# Vortexine's Shop: la tienda de apps (Flathub, se instalan para tu usuario sin contraseña).
# Igual que el shell: empaquetada desde apps/shop de este repo; sus dependencias son de los repos.
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run makepkg -sif --noconfirm "(en $VORTEX_DIR/packaging/vortex-shop)"
else
  if (cd "$VORTEX_DIR/packaging/vortex-shop" && con_reintentos makepkg -sif --noconfirm); then
    bien "Vortexine's Shop instalada (vortex-shop)"
  else
    aviso "No se pudo instalar Vortexine's Shop (packaging/vortex-shop): sigues sin la tienda de apps"
  fi
  rm -rf "$VORTEX_DIR/packaging/vortex-shop/"{pkg,src} "$VORTEX_DIR/packaging/vortex-shop/"*.pkg.tar.*
fi
# Luz nocturna (el único paquete del AUR que pide el shell).
instalar_aur wl-gammarelay-rs || aviso "No se pudo instalar wl-gammarelay-rs: la luz nocturna no funcionará"

# Lo de tu usuario (Hyprland, teclado, fondos, ajustes del shell, kitty, gestor): va en
# usuario.sh porque la ISO lo corre aparte, en el primer inicio de cada usuario nuevo.
# shellcheck source=install/usuario.sh
source "$VORTEX_DIR/install/usuario.sh"

# El panel del sistema cambia el perfil de energía (rendimiento/equilibrado/ahorro).
# Con «tuned» (archinstall) el perfil lo da tuned-ppd y no existe esa unidad.
if systemctl list-unit-files power-profiles-daemon.service 2>/dev/null | grep -q '^power-profiles-daemon'; then
  srun systemctl enable power-profiles-daemon.service
else
  info "power-profiles-daemon no está instalado (¿tuned-ppd?); se deja el control de energía como esté"
fi
