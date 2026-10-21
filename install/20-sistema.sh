# shellcheck shell=bash
paso "Sistema base: kernel CachyOS, drivers y ajustes"

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/sistema.txt"; leer_paquetes "$VORTEX_DIR/packages/extra.txt")
instalar_pacman "${pkgs[@]}"

# --- Gestor de arranque ---
# El kernel original de Arch se queda como respaldo; el de CachyOS va primero.
if ((VORTEX_ES_CACHYOS)); then
  bien "CachyOS ya arranca con su kernel"
elif [[ -d /boot/grub ]] && tiene grub-mkconfig; then
  srun grub-mkconfig -o /boot/grub/grub.cfg
  bien "GRUB actualizado"
elif tiene bootctl && sudo bootctl is-installed >/dev/null 2>&1; then
  entradas="$(sudo bootctl -p 2>/dev/null)/loader/entries"
  if ! sudo ls "$entradas" 2>/dev/null | grep -q cachyos; then
    base="$(sudo ls "$entradas" 2>/dev/null | grep -v fallback | head -1)"
    if [[ -n "$base" ]]; then
      # Copia la entrada actual y le cambia el kernel por linux-cachyos.
      run sudo sh -c "sed -e 's/^title.*/title   Vortex OS/' \
        -e 's/vmlinuz-linux[^ ]*/vmlinuz-linux-cachyos/' \
        -e 's/initramfs-linux[^ ]*\\.img/initramfs-linux-cachyos.img/' \
        '$entradas/$base' > '$entradas/vortex-os.conf'"
      srun bootctl set-default vortex-os.conf
      bien "Entrada de systemd-boot «Vortex OS» creada"
    else
      aviso "No encontré entradas de systemd-boot; agrega linux-cachyos a mano."
    fi
  fi
elif tiene limine-update || tiene limine-mkinitcpio; then
  bien "Limine se actualiza solo con el hook de mkinitcpio"
else
  aviso "No reconocí tu gestor de arranque: agrega el kernel linux-cachyos a mano."
fi

# --- Drivers de GPU ---
# chwd (de CachyOS) detecta NVIDIA/AMD/Intel, híbridas y portátiles.
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run sudo chwd -a
else
  sudo chwd -a || aviso "chwd no pudo instalar los drivers: revisa con «chwd --list»."
fi

# --- Ajustes propios (encima de cachyos-settings) ---
escribir_sistema /etc/sysctl.d/90-vortex.conf <"$VORTEX_DIR/config/sistema/sysctl.conf"
escribir_sistema /etc/systemd/journald.conf.d/90-vortex.conf <"$VORTEX_DIR/config/sistema/journald.conf"
escribir_sistema /etc/scx_loader.toml <"$VORTEX_DIR/config/sistema/scx_loader.toml"

# --- Pantalla de inicio: greetd + tuigreet ---
escribir_sistema /etc/greetd/config.toml <"$VORTEX_DIR/config/sistema/greetd.toml"
dm_actual="$(basename "$(readlink -f /etc/systemd/system/display-manager.service 2>/dev/null)" 2>/dev/null || true)"
if [[ -z "$dm_actual" || "$dm_actual" == greetd.service || "$dm_actual" == display-manager.service ]]; then
  srun systemctl enable greetd.service
elif preguntar "Ya usas ${dm_actual%.service}. ¿Cambiarlo por greetd (más ligero)?" n; then
  srun systemctl disable "$dm_actual"
  srun systemctl enable greetd.service
else
  info "Se queda ${dm_actual%.service} como pantalla de inicio."
fi

# --- Servicios: solo lo que hace falta ---
srun systemctl enable NetworkManager.service scx_loader.service
if tiene_bluetooth; then srun systemctl enable bluetooth.service; fi
# Servicios que otras distros traen encendidos. En una instalación nueva no existen;
# si ya estaban, se pregunta (cups = impresoras, avahi = descubrir equipos en la red).
for s in cups.service avahi-daemon.service ModemManager.service; do
  if systemctl is-enabled "$s" >/dev/null 2>&1 && preguntar "¿Desactivar ${s%.service}?" n; then
    srun systemctl disable --now "$s"
  fi
done
bien "Servicios mínimos activados"
