# shellcheck shell=bash
paso "Repositorios: multilib, CachyOS y AUR"

# multilib hace falta para Steam y las bibliotecas de 32 bits.
if ! grep -q '^\[multilib\]' /etc/pacman.conf; then
  srun sed -i '/^#\[multilib\]/,/^#Include/ s/^#//' /etc/pacman.conf
  bien "multilib activado"
fi

# Sincroniza las bases de datos (multilib recién activado no tiene la suya).
srun pacman -Sy
instalar_pacman base-devel git curl jq

if ((VORTEX_ES_CACHYOS)) || grep -q '^\[cachyos' /etc/pacman.conf; then
  bien "Repos de CachyOS ya presentes"
else
  # El script oficial elige solo el repo para tu CPU: znver4, v4, v3 o genérico.
  # Así el mismo instalador sirve en PCs viejas (genérico) y nuevas (v3/v4).
  tmp="$(mktemp -d)"
  run curl -fsSL -o "$tmp/cachyos-repo.tar.xz" https://mirror.cachyos.org/cachyos-repo.tar.xz
  run tar -xf "$tmp/cachyos-repo.tar.xz" -C "$tmp"
  if [[ "$VORTEX_SI" == 1 ]]; then
    run bash -c "cd '$tmp/cachyos-repo' && yes | sudo ./cachyos-repo.sh --install"
  else
    run bash -c "cd '$tmp/cachyos-repo' && sudo ./cachyos-repo.sh --install"
  fi
  rm -rf "$tmp"
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
