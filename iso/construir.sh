#!/usr/bin/env bash
# Construye la ISO de Vortex OS con archiso. Se corre como root en un Arch con «archiso».
#
#   sudo iso/construir.sh [carpeta-de-salida]
#
# Parte del perfil oficial «releng» de Arch y le agrega Vortex encima:
# el código de Vortex en /root/vortex-os, el comando «vortex-instalar» y su mensaje de bienvenida.
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"
SALIDA="$(realpath -m "${1:-$AQUI/out}")"
TRABAJO="${VORTEX_ISO_TRABAJO:-/var/tmp/vortex-iso}"

[[ $EUID -eq 0 ]] || { echo "Corre esto como root (sudo)." >&2; exit 1; }
command -v mkarchiso >/dev/null || pacman -S --needed --noconfirm archiso

rm -rf "$TRABAJO"
mkdir -p "$TRABAJO" "$SALIDA"
perfil="$TRABAJO/perfil"
cp -a /usr/share/archiso/configs/releng "$perfil"

# Encima del perfil oficial: archivos de airootfs, paquetes extra y nombre de la ISO.
cp -a "$AQUI/airootfs/." "$perfil/airootfs/"
cat "$AQUI/paquetes-extra.txt" >>"$perfil/packages.x86_64"

# El código de Vortex viaja dentro de la ISO para instalar aunque GitHub no responda.
# Se copia el árbol de trabajo SIN .git (ni el de quien construye, que trae su identidad,
# reflogs y hooks) y sin conservar el dueño de los archivos.
copia="$perfil/airootfs/root/vortex-os"
mkdir -p "$copia"
tar -C "$RAIZ" --exclude=./.git --exclude=./iso/out --exclude=./vortex-shell -cf - . |
  tar -C "$copia" --no-same-owner -xf -

# Para que «vortex actualizar» pueda hacer «git pull» en equipos instalados desde la ISO, la copia
# lleva un repo limpio: un clon de lo que tiene commit, con origin público (HTTPS) y sin la config
# local de quien construye. Solo se incluye si el árbol coincide con HEAD (si está sucio, un
# «git pull» fallaría por «cambios locales») y si ese commit ya está publicado en origin (si no,
# el pull fallaría o pediría credenciales): en otro caso la ISO sale sin .git y se avisa.
URL_PUBLICO="https://github.com/MikePianoXD777/vortex-os.git"
git_raiz() { git -c safe.directory="$RAIZ" -C "$RAIZ" --no-optional-locks "$@"; }
if ! command -v git >/dev/null || ! git_raiz rev-parse --git-dir >/dev/null 2>&1; then
  echo "Aviso: no hay git o $RAIZ no es un repo; la ISO sale sin .git y «vortex actualizar» no podrá actualizar Vortex." >&2
elif [[ -n "$(git_raiz status --porcelain)" ]]; then
  echo "Aviso: el árbol tiene cambios sin commit; la ISO sale sin .git (haz commit y reconstruye para incluirlo)." >&2
elif ! rama="$(git_raiz symbolic-ref --short HEAD 2>/dev/null)"; then
  echo "Aviso: HEAD está desprendido (sin rama); la ISO sale sin .git." >&2
else
  # ¿El commit de HEAD ya está en el repo público? Sin pedir credenciales ni quedarse esperando.
  cabeza="$(git_raiz rev-parse HEAD)"
  remoto="$(GIT_TERMINAL_PROMPT=0 timeout 30 git ls-remote "$URL_PUBLICO" "refs/heads/$rama" 2>/dev/null | cut -f1 || true)"
  if [[ -z "$remoto" ]]; then
    echo "Aviso: no pude consultar $URL_PUBLICO (¿sin red o sin publicar?); la ISO sale sin .git." >&2
  elif [[ "$remoto" != "$cabeza" ]] && ! git_raiz merge-base --is-ancestor "$cabeza" "$remoto" 2>/dev/null; then
    echo "Aviso: el commit ${cabeza:0:7} no está publicado en la rama $rama de origin; la ISO sale sin .git (haz push y reconstruye)." >&2
  else
    clon="$TRABAJO/clon-git"
    # --no-local copia solo lo alcanzable desde la rama (nada de ramas sin publicar ni objetos
    # colgantes de quien construye); --no-tags evita traer etiquetas locales.
    git -c safe.directory="$RAIZ" clone -q --no-checkout --no-local --no-tags --single-branch \
      --branch "$rama" --template= "$RAIZ" "$clon"
    mv "$clon/.git" "$copia/.git"
    rm -rf "$clon"
    # Siempre el URL público, aunque el origin del repo local sea SSH o un fork.
    git -C "$copia" remote set-url origin "$URL_PUBLICO"
    git -C "$copia" config branch."$rama".remote origin
    git -C "$copia" config branch."$rama".merge "refs/heads/$rama"
    # El chmod recursivo de abajo deja todo en 755: que git no lo cuente como cambio.
    git -C "$copia" config core.fileMode false
    # Sin checkout el índice está vacío: se arma desde HEAD (el árbol ya está en su sitio).
    git -C "$copia" reset -q
    # Los reflogs anotan el usuario y la ruta de quien construye: fuera.
    rm -rf "$copia/.git/logs" "$copia/.git/ORIG_HEAD" "$copia/.git/FETCH_HEAD"
  fi
fi

sed -i \
  -e 's/^iso_name=.*/iso_name="vortex-os"/' \
  -e 's/^iso_label=.*/iso_label="VORTEX_$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y%m)"/' \
  -e 's|^iso_publisher=.*|iso_publisher="Vortex OS <https://github.com/MikePianoXD777/vortex-os>"|' \
  -e 's/^iso_application=.*/iso_application="Vortex OS Live"/' \
  "$perfil/profiledef.sh"
# Permisos de los scripts agregados. mkarchiso copia airootfs sin conservar el modo (todo 0644) y
# solo hace chmod recursivo si la ruta termina en «/»: sin la barra final se pierde el +x de
# install.sh y bin/*.
sed -i 's|^file_permissions=(|file_permissions=(\n  ["/usr/local/bin/vortex-instalar"]="0:0:755"\n  ["/usr/local/bin/vortex-preparar"]="0:0:755"\n  ["/root/vortex-os/"]="0:0:755"|' \
  "$perfil/profiledef.sh"

# Título de los menús de arranque.
grep -rl 'Arch Linux install medium' "$perfil"/efiboot "$perfil"/syslinux "$perfil"/grub 2>/dev/null |
  xargs -r sed -i 's/Arch Linux install medium/Vortex OS/g'

mkarchiso -v -w "$TRABAJO/work" -o "$SALIDA" "$perfil"
rm -rf "$TRABAJO"
ls -lh "$SALIDA"/vortex-os-*.iso
