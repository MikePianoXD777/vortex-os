#!/usr/bin/env bash
# Arma el repo local [vortex] con los paquetes propios de Vortex OS (los que no están en los
# repos de Arch ni de CachyOS). Se corre como usuario normal (makepkg no corre como root).
#
#   iso/paquetes.sh [carpeta]     # por defecto /var/tmp/vortex-repo
#
# Los del AUR (Calamares tarda ~10 min en compilar) no se vuelven a compilar si el repo ya tiene
# esa misma versión: se compara la del PKGBUILD del AUR con lo que hay en la carpeta. Para
# compilarlos de todos modos: VORTEX_PAQUETES_FORZAR=1 iso/paquetes.sh
#
# Después: sudo iso/construir.sh
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"

# Los PKGBUILD de este repo (siempre se vuelven a armar: su código puede cambiar sin cambiar la
# versión) y los paquetes del AUR. La luz nocturna la pide el shell. Calamares (el instalador
# gráfico) se compila aquí porque los de CachyOS están enlazados a un boost viejo (no arrancan
# con el de Arch).
PROPIOS=(vortex-shell vortex-studio vortex-flatpaks vortex-claude)
AUR=(wl-gammarelay-rs calamares)
AUR_URL="${VORTEX_AUR_URL:-https://aur.archlinux.org}"

# ¿Los ejecutables y bibliotecas del paquete encuentran todas las bibliotecas que piden? (0 = sí)
# Se leen los NEEDED de cada ELF del paquete (readelf -d) y se buscan en el sistema (ldconfig -p)
# o dentro del propio paquete. No se pueden sacar del .PKGINFO: con un makepkg normal (sin devtools;
# /etc/makepkg.conf trae !autodeps) solo lleva lo que el PKGBUILD declara a mano, nombres de paquete
# sin soname (p. ej. boost-libs), así que no dice nada de un soname que el sistema ya cambió.
# Si no se puede saber (sin ldconfig, el paquete no se abre) devuelve 1: se compila.
enlaza_bien() {
  local f="$1" d e lib rc=0
  local -A sistema=() propias=()
  while IFS= read -r lib; do
    [[ -z "$lib" ]] || sistema["$lib"]=1
  done < <(LC_ALL=C ldconfig -p 2>/dev/null | awk '/^[[:space:]]/ {print $1}')
  ((${#sistema[@]})) || return 1
  d="$(mktemp -d "${TMPDIR:-/tmp}/vortex-enlaza.XXXXXX")" || return 1
  bsdtar -xf "$f" -C "$d" 2>/dev/null || { rm -rf "$d"; return 1; }
  # Lo que el propio paquete trae (sus bibliotecas privadas, p. ej. libcalamares.so).
  while IFS= read -r lib; do
    [[ -z "$lib" ]] || propias["$lib"]=1
  done < <(find "$d" \( -type f -o -type l \) -printf '%f\n')
  while IFS= read -r -d '' e; do
    # ¿Es un ELF? (\x7fELF, en hexadecimal: bash no traga los bytes nulos de otros archivos)
    [[ "$(head -c4 "$e" 2>/dev/null | od -An -tx1 | tr -d ' \n')" == 7f454c46 ]] || continue
    while IFS= read -r lib; do
      if [[ -n "$lib" && -z "${sistema[$lib]:-}" && -z "${propias[$lib]:-}" ]]; then rc=1; fi
    done < <(LC_ALL=C readelf -d "$e" 2>/dev/null | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
  done < <(find "$d" -type f -print0)
  rm -rf "$d"
  return "$rc"
}

# ¿El repo ya tiene los paquetes que armaría este PKGBUILD, en esa misma versión? (0 = sí, no
# hace falta compilar). Con `makepkg --packagelist` salen los nombres de archivo con la versión
# del PKGBUILD (epoch:pkgver-pkgrel), y con PKGDEST apuntando al repo se comparan ahí.
# Se compila de todos modos si:
#  - el PKGBUILD tiene pkgver() (paquetes -git: la versión real solo se sabe al compilar);
#  - falta algún archivo del paquete en el repo;
#  - el paquete quedó enlazado a una biblioteca que el sistema ya cambió (p. ej. boost subió su
#    soname): un Calamares viejo contra un boost nuevo no arranca, que es justo lo que pasó con el
#    de CachyOS. Se revisa con las versiones mínimas que el PKGBUILD declare (pacman -T sobre los
#    «depend =» del .PKGINFO) y, sobre todo, con los NEEDED de sus binarios (enlaza_bien).
ya_en_repo() {
  local dir="$1" repo="$2" lista=() f pkg deps=()
  [[ -z "${VORTEX_PAQUETES_FORZAR:-}" ]] || return 1
  [[ -f "$dir/PKGBUILD" ]] || return 1
  ! grep -q '^[[:space:]]*pkgver[[:space:]]*()' "$dir/PKGBUILD" || return 1
  mapfile -t lista < <(cd "$dir" && PKGDEST="$repo" makepkg --packagelist 2>/dev/null) || return 1
  ((${#lista[@]})) || return 1
  for f in "${lista[@]}"; do
    # El nombre sin carpeta, tal como lo deja makepkg (la extensión depende de PKGEXT).
    pkg="${f##*/}"
    pkg="${pkg%.pkg.tar*}"
    f="$(compgen -G "$repo/$pkg.pkg.tar*" | grep -v '\.sig$' | head -n1 || true)"
    [[ -n "$f" ]] || return 1
    mapfile -t deps < <(bsdtar -xOf "$f" .PKGINFO 2>/dev/null | sed -n 's/^depend = //p')
    ((${#deps[@]} == 0)) || pacman -T "${deps[@]}" >/dev/null 2>&1 || return 1
    enlaza_bien "$f" || return 1
  done
  return 0
}

# Compila cada paquete del AUR que haga falta (ver ya_en_repo).
construir_aur() {
  local repo="$1" aur p
  aur="$(mktemp -d)"
  # shellcheck disable=SC2064  # se expande ahora a propósito: $aur es local de esta función
  trap "rm -rf '$aur'" EXIT
  for p in "${AUR[@]}"; do
    echo "==> $p (AUR)"
    git clone -q --depth 1 "$AUR_URL/$p.git" "$aur/$p"
    if ya_en_repo "$aur/$p" "$repo"; then
      echo "    ya está en $repo con la misma versión del AUR: no se vuelve a compilar"
      continue
    fi
    (cd "$aur/$p" && makepkg -s --noconfirm --needed -f)
  done
}

principal() {
  local REPO d
  REPO="$(realpath -m "${1:-/var/tmp/vortex-repo}")"
  [[ $EUID -ne 0 ]] || { echo "Corre esto como usuario normal (makepkg no corre como root)." >&2; exit 1; }
  mkdir -p "$REPO"
  export PKGDEST="$REPO"

  for d in "${PROPIOS[@]}"; do
    echo "==> $d"
    (cd "$RAIZ/packaging/$d" && makepkg -s --noconfirm --needed -f)
    rm -rf "$RAIZ/packaging/$d/"{pkg,src}
  done

  construir_aur "$REPO"

  # Solo la última versión de cada paquete, y la base de datos del repo.
  cd "$REPO"
  rm -f vortex.db* vortex.files*
  repo-add -q -R vortex.db.tar.gz ./*.pkg.tar.zst
  ls -la "$REPO"
}

# Se puede cargar con source (pruebas/iso.sh prueba ya_en_repo) sin armar nada.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  principal "$@"
fi
