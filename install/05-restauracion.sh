# shellcheck shell=bash
paso "Punto de restauración (antes de cambiar nada)"

# Con btrfs las fotos son instantáneas y al principio no ocupan espacio: guardan el estado
# exacto de ahora. Si algo sale mal, se vuelve atrás sin perder nada.
if [[ "$(findmnt -no FSTYPE / 2>/dev/null)" != btrfs ]]; then
  aviso "Tu disco no usa btrfs: no hay punto de restauración automático. Respalda lo importante antes."
  if ! preguntar "¿Seguir sin punto de restauración?" s; then
    error "Instalación cancelada. Respalda tus archivos y vuelve a correr el instalador."
  fi
else
  # El sistema (/), con snapper si está configurado (CachyOS lo trae así).
  if tiene snapper && snapper list-configs 2>/dev/null | grep -q '^root '; then
    srun snapper -c root create -t single -c important -d "Antes de Vortexine's OS"
    bien "Foto del sistema con snapper: «Antes de Vortexine's OS» (se ve con: snapper list)"
  else
    info "snapper no está configurado para /: no se toma foto del sistema"
  fi

  # Tus archivos (/home): Steam, Minecraft y sus mods, Claude, configs... El instalador no
  # los toca, pero queda una copia de solo lectura por si acaso.
  if [[ "$(findmnt -no FSTYPE /home 2>/dev/null)" == btrfs && "$(stat -c %i /home)" == 256 ]]; then
    foto_home="/home/.vortex-antes-$(date +%Y%m%d-%H%M%S)"
    srun btrfs subvolume snapshot -r /home "$foto_home"
    bien "Copia instantánea de /home en $foto_home"
    info "Cuando todo te funcione, bórrala para liberar espacio: sudo btrfs subvolume delete $foto_home"
  fi
fi
