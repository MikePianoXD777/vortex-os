#!/usr/bin/env bash
# Arma el repo local [vortex] con los paquetes propios de Vortexine's OS (los que no están en los
# repos de Arch ni de CachyOS). Se corre como usuario normal (makepkg no corre como root).
#
#   iso/paquetes.sh [carpeta]     # por defecto /var/tmp/vortex-repo
#
# Los del AUR (Calamares tarda ~10 min en compilar) no se vuelven a compilar si el repo ya tiene
# esa misma versión: se compara la del PKGBUILD del AUR con lo que hay en la carpeta. Para
# compilarlos de todos modos: VORTEX_PAQUETES_FORZAR=1 iso/paquetes.sh
#
# Todo se compila para un x86-64 cualquiera, no para la CPU de quien construye (ver
# makepkg_generico): la ISO tiene que arrancar también en equipos sin AVX2.
#
# Después: sudo iso/construir.sh
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"

# Los PKGBUILD de este repo (siempre se vuelven a armar: su código puede cambiar sin cambiar la
# versión) y los paquetes del AUR. La luz nocturna la pide el shell. vortex-shop es la tienda de
# apps (Flathub): en la ISO solo viene ella, las apps se bajan después. Calamares (el instalador
# gráfico) se compila aquí porque los de CachyOS están enlazados a un boost viejo (no arrancan
# con el de Arch).
PROPIOS=(vortex-shell vortex-studio vortex-flatpaks vortex-claude vortex-shop vortex-miku)
AUR=(wl-gammarelay-rs calamares shim-signed)
AUR_URL="${VORTEX_AUR_URL:-https://aur.archlinux.org}"

# La ISO tiene que arrancar en cualquier x86-64 (construir.sh usa el [cachyos] genérico), pero el
# makepkg.conf de quien la construye no es genérico: el de CachyOS trae -march=native y su
# makepkg.conf.d/rust.conf, -C target-cpu=native. Un Calamares o un wl-gammarelay-rs compilados así
# mueren con «Illegal instruction» en una CPU sin AVX2 (Intel anteriores a Haswell, muchos
# Celeron/Pentium/Atom, AMD A8/A10 viejos): el instalador ni abre. Por eso todo se compila con una
# copia de esa config sin la CPU de quien construye (makepkg_generico).
# El .PKGINFO de lo compilado así lleva este «packager»: es la marca con la que ya_en_repo sabe que
# un paquete que ya está en el repo no salió con las banderas del constructor (el .BUILDINFO no
# guarda CFLAGS, así que no hay otra forma de saberlo).
PACKAGER_GENERICO="Vortexines OS (x86-64 generico)"

# Lo que se agrega al final de la config genérica (ver config_generica). makepkg hace «source» de
# esto y aborta si termina con error: de ahí los «if» en vez de «&&» y el «true» del final.
ajuste_generico() {
  cat <<'AJUSTE'

# --- Vortexine's OS: nada se compila para la CPU de quien construye la ISO (iso/paquetes.sh) ---
_vortex_generico() {
  sed -E \
    -e 's/(^|[[:space:]])-march=[^[:space:]]+/\1-march=x86-64/g' \
    -e 's/(^|[[:space:]])-mtune=[^[:space:]]+/\1-mtune=generic/g' \
    -e 's/(^|[[:space:]])(-C[[:space:]]*)target-cpu=[^[:space:]]+/\1\2target-cpu=x86-64/g' <<<"$1"
}
for _vortex_v in CFLAGS CXXFLAGS CPPFLAGS FFLAGS FCFLAGS RUSTFLAGS; do
  if [[ -v $_vortex_v ]]; then printf -v "$_vortex_v" '%s' "$(_vortex_generico "${!_vortex_v}")"; fi
done
if [[ "${RUSTFLAGS:-}" != *target-cpu=* ]]; then RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C target-cpu=x86-64"; fi
unset -f _vortex_generico
unset _vortex_v
true
AJUSTE
}

# Escribe en la carpeta $1 una config de makepkg igual a la del constructor (/etc/makepkg.conf y su
# makepkg.conf.d/, o VORTEX_MAKEPKG_CONF para probar) pero con la CPU genérica: -march=x86-64,
# -mtune=generic y RUSTFLAGS con -C target-cpu=x86-64 (el valor por defecto de rustc, explícito para
# que ni un ~/.cargo/config.toml lo cambie). Los ajustes se aplican al final de todo (en el propio
# makepkg.conf, que es lo único que leen los pacman viejos, y en un zz-*.conf de makepkg.conf.d, que
# pacman 7 lee después de rust.conf) sobre las variables ya evaluadas: así da igual cómo estén
# escritas las banderas (varias líneas, CXXFLAGS="$CFLAGS ...", -C target-cpu o -Ctarget-cpu). Un
# makepkg que usa --config no lee el ~/.makepkg.conf de quien construye: es a propósito.
config_generica() {
  local d="$1" base="${VORTEX_MAKEPKG_CONF:-/etc/makepkg.conf}" f
  [[ -r "$base" ]] || { echo "No puedo leer $base (la config de makepkg del constructor)." >&2; return 1; }
  mkdir -p "$d/makepkg.conf.d" || return 1
  cp "$base" "$d/makepkg.conf" || return 1
  for f in "$base.d"/*.conf; do
    [[ -f "$f" ]] && { cp "$f" "$d/makepkg.conf.d/" || return 1; }
  done
  ajuste_generico >>"$d/makepkg.conf"
  ajuste_generico >"$d/makepkg.conf.d/zz-vortex-generico.conf"
}

# makepkg con esa config genérica, y con el «packager» que marca lo compilado así (PACKAGER en el
# entorno le gana a lo que diga cualquier config, incluso si quien construye tiene el suyo).
# Los argumentos son los de makepkg.
makepkg_generico() {
  local d rc=0
  d="$(mktemp -d "${TMPDIR:-/tmp}/vortex-makepkg.XXXXXX")" || return 1
  config_generica "$d" || { rm -rf "$d"; return 1; }
  PACKAGER="$PACKAGER_GENERICO" makepkg --config "$d/makepkg.conf" "$@" || rc=$?
  rm -rf "$d"
  return "$rc"
}

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
#  - el paquete no se compiló con la config genérica (su «packager» del .PKGINFO no es
#    PACKAGER_GENERICO): un paquete de antes de este cambio, o hecho a mano, puede traer
#    -march=native del constructor y no arrancar en una CPU sin AVX2. Se recompila una vez.
ya_en_repo() {
  local dir="$1" repo="$2" lista=() f pkg deps=() info packager
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
    info="$(bsdtar -xOf "$f" .PKGINFO 2>/dev/null)" || return 1
    packager="$(sed -n 's/^packager = //p' <<<"$info")"
    [[ "$packager" == "$PACKAGER_GENERICO" ]] || return 1
    mapfile -t deps < <(sed -n 's/^depend = //p' <<<"$info")
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
    (cd "$aur/$p" && makepkg_generico -s --noconfirm --needed -f)
  done
}

principal() {
  local REPO d
  REPO="$(realpath -m "${1:-/var/tmp/vortex-repo}")"
  [[ $EUID -ne 0 ]] || { echo "Corre esto como usuario normal (makepkg no corre como root)." >&2; exit 1; }
  mkdir -p "$REPO"
  export PKGDEST="$REPO"
  echo "==> Se compila con banderas de un x86-64 genérico (no las de ${VORTEX_MAKEPKG_CONF:-/etc/makepkg.conf})"

  for d in "${PROPIOS[@]}"; do
    # vortex-miku (el asistente) puede faltar mientras su rama no entre: sin su PKGBUILD se avisa y
    # se sigue (con cualquier otro paquete propio, la falta sí es un error).
    if [[ "$d" == vortex-miku && ! -f "$RAIZ/packaging/$d/PKGBUILD" ]]; then
      echo "==> $d: no hay packaging/$d/PKGBUILD, se omite (¡la ISO lo pide en paquetes.txt!)" >&2
      continue
    fi
    echo "==> $d"
    (cd "$RAIZ/packaging/$d" && makepkg_generico -s --noconfirm --needed -f)
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
