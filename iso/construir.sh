#!/usr/bin/env bash
# Construye la ISO de Vortex OS: un live que arranca en el escritorio de Vortex y se instala
# con un instalador gráfico (Calamares). Se corre como root en un Arch (o Vortex) con archiso
# y con los repos de CachyOS; antes, como usuario normal, iso/paquetes.sh arma el repo local.
#
#   iso/paquetes.sh                       # repo [vortex] en /var/tmp/vortex-repo
#   sudo iso/construir.sh [carpeta-de-salida]
#
# Del perfil oficial «releng» de archiso solo se usa el arranque (UEFI y BIOS) y el llavero
# del live; el sistema es el de Vortex: los paquetes de iso/paquetes.txt y lo mismo que deja
# install.sh en /etc y /usr/local. Lo que se instala con Calamares es una copia de esta imagen.
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"
SALIDA="$(realpath -m "${1:-$AQUI/out}")"
TRABAJO="${VORTEX_ISO_TRABAJO:-/var/tmp/vortex-iso}"
REPO="${VORTEX_REPO_LOCAL:-/var/tmp/vortex-repo}"

[[ $EUID -eq 0 ]] || { echo "Corre esto como root (sudo)." >&2; exit 1; }
command -v mkarchiso >/dev/null || pacman -S --needed --noconfirm archiso
command -v cc >/dev/null || { echo "Falta un compilador de C (base-devel)." >&2; exit 1; }
[[ -f "$REPO/vortex.db" ]] || { echo "No encuentro el repo local en $REPO: corre antes iso/paquetes.sh" >&2; exit 1; }
grep -q '^\[cachyos\]' /etc/pacman.conf || { echo "Este equipo no tiene el repo [cachyos] en /etc/pacman.conf." >&2; exit 1; }

rm -rf "$TRABAJO"
mkdir -p "$TRABAJO" "$SALIDA"
perfil="$TRABAJO/perfil"
releng=/usr/share/archiso/configs/releng
raiz="$perfil/airootfs"

# --- Perfil: el arranque de releng, el resto de Vortex ---
mkdir -p "$perfil"
cp -a "$releng/efiboot" "$releng/syslinux" "$releng/grub" "$releng/profiledef.sh" "$releng/bootstrap_packages" "$perfil/"
mkdir -p "$raiz/etc/systemd/system/multi-user.target.wants" "$raiz/etc/mkinitcpio.conf.d"
# El initramfs del live (hooks de archiso) y el llavero de pacman en RAM del live.
# Sin los hooks de arranque por red (PXE): un USB no los usa y piden paquetes que no van
# (nbd, mkinitcpio-nfs-utils); sin ellos mkinitcpio da errores.
sed -E 's/ archiso_pxe_[a-z]+//g' "$releng/airootfs/etc/mkinitcpio.conf.d/archiso.conf" >"$raiz/etc/mkinitcpio.conf.d/archiso.conf"
cp -a "$releng/airootfs/etc/systemd/system/pacman-init.service" \
  "$releng/airootfs/etc/systemd/system/etc-pacman.d-gnupg.mount" "$raiz/etc/systemd/system/"
ln -s ../pacman-init.service "$raiz/etc/systemd/system/multi-user.target.wants/pacman-init.service"
cp -a "$AQUI/airootfs/." "$raiz/"

# Paquetes: los de Vortex + lo que releng necesita para arrancar el USB.
{
  sed 's/#.*//; s/[[:space:]]*$//' "$AQUI/paquetes.txt" | grep -v '^$'
  printf '%s\n' syslinux memtest86+ memtest86+-efi arch-install-scripts
} | sort -u >"$perfil/packages.x86_64"

# --- pacman: [cachyos] genérico (la ISO debe arrancar en cualquier x86-64), multilib y [vortex] ---
repos_pacman() {
  local vortex_server="$1"
  cat <<EOF
[options]
HoldPkg     = pacman glibc
Architecture = auto
ParallelDownloads = 5
SigLevel    = Required DatabaseOptional
LocalFileSigLevel = Optional

[vortex]
SigLevel = Optional TrustAll
Server = $vortex_server

[cachyos]
Include = /etc/pacman.d/cachyos-mirrorlist

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist
EOF
}
repos_pacman "file://$REPO" >"$perfil/pacman.conf"
# El sistema instalado sigue viendo [vortex] (Vortex Studio, apps de Flatpak) sin internet.
mkdir -p "$raiz/usr/share/vortex/repo"
cp -a "$REPO/." "$raiz/usr/share/vortex/repo/"
repos_pacman "file:///usr/share/vortex/repo" >"$raiz/etc/pacman.conf"

# --- Lo que install.sh deja en el sistema ---
cfg="$RAIZ/config/sistema"
install -Dm644 "$cfg/sysctl.conf" "$raiz/etc/sysctl.d/90-vortex.conf"
install -Dm644 "$cfg/thp.conf" "$raiz/etc/tmpfiles.d/thp.conf"
install -Dm644 "$cfg/mglru.conf" "$raiz/etc/tmpfiles.d/90-vortex-mglru.conf"
install -Dm644 "$cfg/journald.conf" "$raiz/etc/systemd/journald.conf.d/90-vortex.conf"
install -Dm644 "$cfg/scx_loader.toml" "$raiz/etc/scx_loader.toml"
# greetd: entra con vortex-sesion (configura a cada usuario nuevo en su primer inicio).
sed 's|start-hyprland </dev/null|vortex-sesion|' "$cfg/greetd.toml" | install -Dm644 /dev/stdin "$raiz/etc/greetd/config.toml"
install -Dm644 "$RAIZ/config/systemd/vortex-gestor.service" "$raiz/etc/systemd/user/vortex-gestor.service"
install -Dm644 "$RAIZ/config/systemd/vortex-flatpaks.service" "$raiz/etc/systemd/system/vortex-flatpaks.service"
for c in vortex vortex-bench vortex-mcp vortex-gestor vortex-flatpaks vortex-sesion; do
  install -Dm755 "$RAIZ/bin/$c" "$raiz/usr/local/bin/$c"
done
cc -O2 -s -o "$raiz/usr/local/bin/vortex-eventos" "$RAIZ/src/vortex-eventos.c"

# Servicios encendidos (en el live y en lo instalado): los mismos que deja install.sh. Van como
# presets de systemd: al instalarse systemd en la imagen se aplican los presets («preset-all») y
# los enlaces sueltos se pierden. networkd y homed se apagan (chocarían con NetworkManager).
mkdir -p "$raiz/etc/systemd/system-preset"
cat >"$raiz/etc/systemd/system-preset/10-vortex.preset" <<'PRESET'
# Vortex OS: lo que deja encendido install.sh.
enable NetworkManager.service
enable NetworkManager-wait-online.service
enable NetworkManager-dispatcher.service
enable systemd-resolved.service
enable greetd.service
enable scx_loader.service
enable power-profiles-daemon.service
enable vortex-flatpaks.service
disable systemd-networkd.service
disable systemd-networkd.socket
disable systemd-networkd-wait-online.service
disable systemd-homed.service
disable ananicy-cpp.service
PRESET
# Solo del live (vortex-post-instalacion lo borra).
cat >"$raiz/etc/systemd/system-preset/05-vortex-live.preset" <<'PRESET'
enable vortex-live.service
enable pacman-init.service
PRESET
# Sin preguntas de primer arranque (systemd-firstboot) en el live: la imagen no tiene machine-id
# a propósito. En lo instalado no hace falta: Calamares pone idioma, zona y machine-id.
ln -sfn /dev/null "$raiz/etc/systemd/system/systemd-firstboot.service"
echo "LANG=C.UTF-8" >"$raiz/etc/locale.conf"
echo "KEYMAP=us" >"$raiz/etc/vconsole.conf"
ln -sfn /usr/share/zoneinfo/UTC "$raiz/etc/localtime"

quiere() { # quiere OBJETIVO UNIDAD [RUTA]
  local d="$raiz/etc/systemd/system/$1.wants"
  mkdir -p "$d"
  ln -sfn "${3:-/usr/lib/systemd/system/$2}" "$d/$2"
}
quiere multi-user NetworkManager.service
quiere multi-user systemd-resolved.service
quiere multi-user scx_loader.service
quiere multi-user power-profiles-daemon.service
quiere multi-user vortex-flatpaks.service /etc/systemd/system/vortex-flatpaks.service
quiere network-online NetworkManager-wait-online.service
ln -sfn /usr/lib/systemd/system/greetd.service "$raiz/etc/systemd/system/display-manager.service"
ln -sfn /usr/lib/systemd/system/NetworkManager-dispatcher.service "$raiz/etc/systemd/system/dbus-org.freedesktop.nm-dispatcher.service"
ln -sfn /usr/lib/systemd/system/systemd-resolved.service "$raiz/etc/systemd/system/dbus-org.freedesktop.resolve1.service"

# Solo para probar la ISO en una VM: VORTEX_ISO_LLAVE_PRUEBA=archivo.pub deja entrar por SSH como
# root (en el live y en lo que se instale con ella) y la ISO se llama «vortex-os-prueba».
# La ISO que se publica NUNCA lleva esto.
prueba=0
if [[ -n "${VORTEX_ISO_LLAVE_PRUEBA:-}" ]]; then
  prueba=1
  echo openssh >>"$perfil/packages.x86_64"
  install -Dm600 "$VORTEX_ISO_LLAVE_PRUEBA" "$raiz/root/.ssh/authorized_keys"
  quiere multi-user sshd.service
  echo "enable sshd.service" >>"$raiz/etc/systemd/system-preset/05-vortex-live.preset"
  echo "AVISO: ISO DE PRUEBA, con una llave SSH de root. No la publiques." >&2
fi

# El código de Vortex (para «vortex configurar-usuario» y «vortex actualizar»), sin .git ni el
# dueño de quien construye.
copia="$raiz/usr/share/vortex-os"
mkdir -p "$copia"
tar -C "$RAIZ" --exclude=./.git --exclude=./iso/out --exclude=./vortex-shell --exclude=./.claude -cf - . |
  tar -C "$copia" --no-same-owner -xf -

# Para que «vortex actualizar» pueda hacer «git pull», la copia lleva un repo limpio: un clon de
# lo que tiene commit, con origin público (HTTPS) y sin la config local de quien construye. Solo
# si el árbol coincide con HEAD y ese commit ya está publicado; si no, sale sin .git y se avisa.
URL_PUBLICO="https://github.com/MikePianoXD777/vortex-os.git"
git_raiz() { git -c safe.directory="$RAIZ" -C "$RAIZ" --no-optional-locks "$@"; }
if ! command -v git >/dev/null || ! git_raiz rev-parse --git-dir >/dev/null 2>&1; then
  echo "Aviso: no hay git o $RAIZ no es un repo; la ISO sale sin .git y «vortex actualizar» no podrá actualizar Vortex." >&2
elif [[ -n "$(git_raiz status --porcelain)" ]]; then
  echo "Aviso: el árbol tiene cambios sin commit; la ISO sale sin .git (haz commit y reconstruye para incluirlo)." >&2
elif ! rama="$(git_raiz symbolic-ref --short HEAD 2>/dev/null)"; then
  echo "Aviso: HEAD está desprendido (sin rama); la ISO sale sin .git." >&2
else
  cabeza="$(git_raiz rev-parse HEAD)"
  remoto="$(GIT_TERMINAL_PROMPT=0 timeout 30 git ls-remote "$URL_PUBLICO" "refs/heads/$rama" 2>/dev/null | cut -f1 || true)"
  if [[ -z "$remoto" ]]; then
    echo "Aviso: no pude consultar $URL_PUBLICO (¿sin red o sin publicar?); la ISO sale sin .git." >&2
  elif [[ "$remoto" != "$cabeza" ]] && ! git_raiz merge-base --is-ancestor "$cabeza" "$remoto" 2>/dev/null; then
    echo "Aviso: el commit ${cabeza:0:7} no está publicado en la rama $rama de origin; la ISO sale sin .git (haz push y reconstruye)." >&2
  else
    clon="$TRABAJO/clon-git"
    git -c safe.directory="$RAIZ" clone -q --no-checkout --no-local --no-tags --single-branch \
      --branch "$rama" --template= "$RAIZ" "$clon"
    mv "$clon/.git" "$copia/.git"
    rm -rf "$clon"
    git -C "$copia" remote set-url origin "$URL_PUBLICO"
    git -C "$copia" config branch."$rama".remote origin
    git -C "$copia" config branch."$rama".merge "refs/heads/$rama"
    git -C "$copia" config core.fileMode false
    git -C "$copia" reset -q
    rm -rf "$copia/.git/logs" "$copia/.git/ORIG_HEAD" "$copia/.git/FETCH_HEAD"
  fi
fi

version="$(sed -n 's/^VORTEX_VERSION="\(.*\)"$/\1/p' "$RAIZ/install/lib.sh")"
printf 'NAME="Vortex OS"\nVERSION="%s"\nVORTEX_REPO="/usr/share/vortex-os"\n' "$version" >"$raiz/etc/vortex-release"
# Marca del live: vortex-sesion abre el instalador. vortex-post-instalacion la borra.
install -Dm644 /dev/null "$raiz/etc/vortex/live"
mkdir -p "$raiz/etc/vortex/flatpaks.d"
echo vortex-live >"$raiz/etc/hostname"

# --- Nombre, permisos y menús de arranque ---
sed -i \
  -e 's/^iso_name=.*/iso_name="vortex-os"/' \
  -e 's/^iso_label=.*/iso_label="VORTEX_$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y%m)"/' \
  -e 's|^iso_publisher=.*|iso_publisher="Vortex OS <https://github.com/MikePianoXD777/vortex-os>"|' \
  -e 's/^iso_application=.*/iso_application="Vortex OS Live"/' \
  -e '/^file_permissions=(/,/^)/d' \
  "$perfil/profiledef.sh"
cat >>"$perfil/profiledef.sh" <<'EOF'
file_permissions=(
  ["/usr/local/bin/"]="0:0:755"
  ["/usr/share/vortex-os/"]="0:0:755"
)
EOF
if ((prueba)); then
  sed -i 's/^iso_name=.*/iso_name="vortex-os-prueba"/' "$perfil/profiledef.sh"
  echo 'file_permissions+=(["/root"]="0:0:750" ["/root/.ssh"]="0:0:700" ["/root/.ssh/authorized_keys"]="0:0:600")' >>"$perfil/profiledef.sh"
fi
grep -rl 'Arch Linux install medium' "$perfil"/efiboot "$perfil"/syslinux "$perfil"/grub 2>/dev/null |
  xargs -r sed -i 's/Arch Linux install medium/Vortex OS/g'

mkarchiso -v -w "$TRABAJO/work" -o "$SALIDA" "$perfil"
rm -rf "$TRABAJO"
ls -la "$SALIDA"
