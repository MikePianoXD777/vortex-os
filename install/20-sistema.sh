# shellcheck shell=bash
paso "Sistema base: kernel CachyOS, drivers y ajustes"

mapfile -t pkgs < <(leer_paquetes "$VORTEX_DIR/packages/sistema.txt"; leer_paquetes "$VORTEX_DIR/packages/extra.txt")
instalar_pacman "${pkgs[@]}"

# Las laptops Intel desde ~2019 usan SOF: sin su firmware no hay tarjeta de sonido.
if tiene lspci && lspci -nn 2>/dev/null | grep -Ei '(audio device|multimedia audio)' | grep -q '\[8086:'; then
  instalar_pacman sof-firmware
fi

# --- Gestor de arranque ---
# El kernel original de Arch se queda como respaldo; el de CachyOS va primero.
# /boot suele ser 0700 root (la ESP de archinstall), así que todo se revisa con sudo.
# En --prueba no se pide contraseña: sin sudo ya autenticado, no se puede revisar.
puedo_revisar=1
if [[ "$VORTEX_DRY_RUN" == 1 ]] && ! sudo -n true 2>/dev/null; then puedo_revisar=0; fi

grub_dir=""
if ((!VORTEX_ES_CACHYOS && puedo_revisar)) && tiene grub-mkconfig; then
  for d in /boot/grub /efi/grub /boot/efi/grub; do
    if sudo test -d "$d"; then grub_dir="$d"; break; fi
  done
fi

if ((VORTEX_ES_CACHYOS)); then
  bien "CachyOS ya arranca con su kernel"
elif ((!puedo_revisar)); then
  info "Prueba sin sudo activo: no reviso el gestor de arranque (en la instalación real sí)."
elif [[ -n "$grub_dir" ]]; then
  srun grub-mkconfig -o "$grub_dir/grub.cfg"
  bien "GRUB actualizado"
elif tiene bootctl && sudo bootctl is-installed >/dev/null 2>&1; then
  # Las entradas están en la ESP o, si hay una partición XBOOTLDR, en esa.
  entradas="" base="" ya_hay=0
  for raiz in "$(sudo bootctl -x 2>/dev/null || true)" "$(sudo bootctl -p 2>/dev/null || true)"; do
    [[ -n "$raiz" ]] || continue
    # La entrada que crea este mismo paso (vortex-os.conf) también cuenta: así es idempotente.
    if sudo ls "$raiz/loader/entries" 2>/dev/null | grep -qE 'cachyos|^vortex-os\.conf$'; then ya_hay=1; break; fi
    if [[ -z "$base" ]]; then
      candidata="$(sudo ls "$raiz/loader/entries" 2>/dev/null | grep -v -e fallback -e '^vortex-os\.conf$' | head -1 || true)"
      if [[ -n "$candidata" ]]; then entradas="$raiz/loader/entries" base="$candidata"; fi
    fi
  done
  if ((ya_hay)); then
    bien "systemd-boot ya tiene una entrada con linux-cachyos"
  elif [[ -n "$base" ]]; then
    # Copia la entrada actual y le cambia el kernel por linux-cachyos.
    run sudo sh -c "sed -e 's/^title.*/title   Vortex OS/' \
      -e 's/vmlinuz-linux[^ ]*/vmlinuz-linux-cachyos/' \
      -e 's/initramfs-linux[^ ]*\\.img/initramfs-linux-cachyos.img/' \
      '$entradas/$base' > '$entradas/vortex-os.conf'"
    srun bootctl set-default vortex-os.conf
    bien "Entrada de systemd-boot «Vortex OS» creada"
  else
    aviso "No encontré entradas de systemd-boot (¿imagen unificada?); agrega linux-cachyos a mano."
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
greetd_conf="$(cat "$VORTEX_DIR/config/sistema/greetd.toml")"
if ((VORTEX_AUTOLOGIN)); then
  # Primera sesión de cada arranque sin contraseña; al cerrar sesión vuelve tuigreet.
  printf -v greetd_conf '%s\n\n[initial_session]\ncommand = "start-hyprland"\nuser = "%s"' "$greetd_conf" "$USER"
fi
escribir_sistema /etc/greetd/config.toml <<<"$greetd_conf"
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
# bluez no viene en las listas: sin él, «enable bluetooth.service» abortaría el instalador.
if tiene_bluetooth; then
  instalar_pacman bluez bluez-utils
  srun systemctl enable bluetooth.service
fi
# Servicios que otras distros traen encendidos. En una instalación nueva no existen;
# si ya estaban, se pregunta (cups = impresoras, avahi = descubrir equipos en la red).
for s in cups.service avahi-daemon.service ModemManager.service; do
  if systemctl is-enabled "$s" >/dev/null 2>&1 && preguntar "¿Desactivar ${s%.service}?" n; then
    srun systemctl disable --now "$s"
  fi
done

# cachyos-settings enciende servicios que aquí sobran (no se quita el paquete).
# En una CachyOS que ya usabas se pregunta; en una instalación nueva se apagan.
apagar_de_cachyos() {
  local servicio="$1" motivo="$2"
  systemctl is-enabled "$servicio" >/dev/null 2>&1 || return 0
  if ((VORTEX_ES_CACHYOS)) && ! preguntar "¿Desactivar ${servicio%.service} ($motivo)?" n; then
    return 0
  fi
  srun systemctl disable --now "$servicio"
  info "Desactivado ${servicio%.service}: $motivo"
}
# scx_lavd ya reparte la CPU; las reglas de nice de ananicy sobran (~15 MB de RAM).
apagar_de_cachyos ananicy-cpp.service "scx_lavd ya planifica"
# systemd-resolved NO se apaga: cachyos-settings configura NetworkManager con
# dns=systemd-resolved (resolv.conf apunta a 127.0.0.53); sin él no habría DNS.
# oomd sí ayuda con poca RAM: se queda en equipos de menos de 8 GB.
if (($(ram_mb) >= 8000)); then
  apagar_de_cachyos systemd-oomd.service "RAM de sobra"
fi
bien "Servicios mínimos activados"
