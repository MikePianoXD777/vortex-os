# shellcheck shell=bash
paso "Repositorios: multilib, CachyOS y AUR"

# multilib hace falta para Steam y las bibliotecas de 32 bits.
if ! grep -q '^\[multilib\]' /etc/pacman.conf; then
  srun sed -i '/^#\[multilib\]/,/^#Include/ s/^#//' /etc/pacman.conf
  bien "multilib activado"
fi

# Sincroniza y actualiza todo de una vez (multilib recién activado no tiene su base de datos).
# Nunca -Sy a secas: una actualización parcial puede dejar pacman o curl con bibliotecas mezcladas.
con_reintentos srun pacman -Syu --noconfirm
instalar_pacman base-devel git curl jq

if ((VORTEX_ES_CACHYOS)) || grep -q '^\[cachyos' /etc/pacman.conf; then
  bien "Repos de CachyOS ya presentes"
else
  # El script oficial elige el repo para tu CPU: znver4, v4 o v3. Si la CPU no
  # soporta ni v3 (sin AVX2), no agrega nada y sale sin error: más abajo se
  # completa con el repo genérico para que también sirva en PCs viejas.
  tmp="$(mktemp -d)"
  run curl -fsSL -o "$tmp/cachyos-repo.tar.xz" https://mirror.cachyos.org/cachyos-repo.tar.xz
  run tar -xf "$tmp/cachyos-repo.tar.xz" -C "$tmp"
  if [[ "$VORTEX_SI" == 1 ]]; then
    run bash -c "cd '$tmp/cachyos-repo' && yes | sudo ./cachyos-repo.sh --install"
  else
    run bash -c "cd '$tmp/cachyos-repo' && sudo ./cachyos-repo.sh --install"
  fi
  rm -rf "$tmp"

  # CPU sin x86-64-v3: el script dejó pacman.conf intacto (aunque sí instaló la
  # lista de espejos). Se agrega a mano el repo genérico, antes de [core].
  if [[ "$VORTEX_DRY_RUN" != 1 ]] && ! grep -q '^\[cachyos' /etc/pacman.conf; then
    aviso "Tu CPU no soporta x86-64-v3: se usa el repo genérico de CachyOS"
    nueva="$(awk '/^\[core\]/ && !hecho { print "[cachyos]"; print "Include = /etc/pacman.d/cachyos-mirrorlist"; print ""; hecho = 1 } { print }' /etc/pacman.conf)"
    escribir_sistema /etc/pacman.conf <<<"$nueva"
    grep -q '^\[cachyos\]' /etc/pacman.conf || error "No se pudo agregar el repo de CachyOS a /etc/pacman.conf"
  fi
  bien "Repos de CachyOS agregados"
fi

# Los primeros espejos de la lista de CachyOS a veces van atrasados (404 en paquetes nuevos).
# Fuera de CachyOS se ponen primero los espejos oficiales, que siempre están al día.
if ((!VORTEX_ES_CACHYOS)); then
  for lista in /etc/pacman.d/cachyos*-mirrorlist; do
    [[ -f "$lista" ]] || continue
    grep -q '^Server = https://cdn.cachyos.org' "$lista" && continue
    ruta="$(grep -m1 -oP '^Server = https://[a-z0-9]+\.cachyos\.org\K/.*' "$lista")" || continue
    nueva="$(awk -v r="$ruta" '/^Server/ && !hecho { print "Server = https://cdn.cachyos.org" r; print "Server = https://mirror.cachyos.org" r; hecho = 1 } { print }' "$lista")"
    escribir_sistema "$lista" <<<"$nueva"
  done
  bien "Espejos oficiales de CachyOS primero"
fi

con_reintentos srun pacman -Syu --noconfirm

# paru viene ya compilado en el repo de CachyOS.
tiene paru || instalar_pacman paru
bien "paru listo para el AUR"

# paru debe borrar las dependencias de compilación: «vortex actualizar» y quien use paru
# a mano lo llaman sin el flag --removemake de lib.sh y, sin esto, dejarían makedeps instalados.
# Si la línea ya está activa o no existe, sed no cambia nada y no falla.
srun sed -i 's/^#RemoveMake$/RemoveMake/' /etc/paru.conf || aviso "No se pudo activar RemoveMake en /etc/paru.conf"
