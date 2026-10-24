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
# Todo se compila para un x86-64 cualquiera, no para la CPU de quien construye: la ISO tiene que
# arrancar también en equipos sin AVX2. Lo que trae binarios (arch=x86_64: Calamares,
# wl-gammarelay-rs, Vortex Studio) se compila en un chroot limpio de Arch (devtools, ver
# compilar_en_chroot); lo que es «any», con makepkg_generico. Al final se revisa que ningún ELF del
# repo pida más que x86-64 básico (revisar_isa); si alguno lo pide, falla.
#
# Necesita devtools (sudo pacman -S devtools) y sudo: mkarchroot y makechrootpkg corren como root.
# El chroot vive en VORTEX_CHROOT (por defecto /var/tmp/vortex-chroot) y se reutiliza.
#
# Después: sudo iso/construir.sh
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RAIZ="$(dirname "$AQUI")"

# Los PKGBUILD de este repo (siempre se vuelven a armar: su código puede cambiar sin cambiar la
# versión) y los paquetes del AUR. La luz nocturna la pide el shell. vortex-shop es la tienda de
# apps (Flathub): en la ISO solo viene ella, las apps se bajan después. Calamares (el instalador
# gráfico) se compila aquí porque los de CachyOS están enlazados a un boost viejo (no arrancan
# con el de Arch).
PROPIOS=(vortex-shell vortex-studio vortex-flatpaks vortex-claude vortex-shop vortex-miku vortex-miku-internet)
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

# Con banderas genéricas no basta para lo que trae binarios: en un constructor con el glibc «v3» de
# CachyOS (cachyos-core-v3), su crt1.o lleva la nota «x86 ISA needed: x86-64-v3»
# (GNU_PROPERTY_X86_ISA_1_NEEDED) y el enlazador se la pasa a todo ejecutable. El glibc del live se
# niega a abrirlo en una CPU sin AVX2 («CPU ISA level is lower than required»): le pasó al Calamares
# de la ISO 1.3 (ver iso/arreglos/calamares-cpu-vieja.sh). Por eso lo que no es «any» se compila en
# un chroot de Arch con solo core/extra/multilib (nada de CachyOS: ni v3 ni genérico) y la
# makepkg.conf de x86_64 de devtools. Su «packager» es otra marca: lo compilado antes fuera del
# chroot (con la marca de arriba) no se reutiliza.
PACKAGER_CHROOT="Vortexines OS (x86-64 generico, chroot de Arch)"
CHROOT="${VORTEX_CHROOT:-/var/tmp/vortex-chroot}"
CHROOT_LISTO=0

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
# Con $2 se parte de otra config en vez de la del constructor (el chroot usa la de devtools).
config_generica() {
  local d="$1" base="${2:-${VORTEX_MAKEPKG_CONF:-/etc/makepkg.conf}}" f
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

# ¿Es un ELF? (\x7fELF, en hexadecimal: bash no traga los bytes nulos de otros archivos)
es_elf() { [[ "$(head -c4 "$1" 2>/dev/null | od -An -tx1 | tr -d ' \n')" == 7f454c46 ]]; }

# Escribe «archivo: nota» por cada ELF del paquete $1 cuya nota «x86 ISA needed» pide más que
# x86-64 básico (v2, v3 o v4): el glibc no lo abre en una CPU que no llega a ese nivel, aunque el
# código no use nada de eso. Sin nota no se escribe nada (el glibc no revisa). Si el paquete no se
# puede abrir, devuelve 1.
elf_no_genericos() {
  local f="$1" d e nota
  d="$(mktemp -d "${TMPDIR:-/tmp}/vortex-isa.XXXXXX")" || return 1
  bsdtar -xf "$f" -C "$d" 2>/dev/null || { rm -rf "$d"; return 1; }
  while IFS= read -r -d '' e; do
    es_elf "$e" || continue
    nota="$(LC_ALL=C readelf -n "$e" 2>/dev/null | sed -n 's/^[[:space:]]*x86 ISA needed:[[:space:]]*//p' | head -n1)"
    if [[ "$nota" =~ x86-64-v[2-9] ]]; then printf '%s: %s\n' "${e#"$d"/}" "$nota"; fi
  done < <(find "$d" -type f -print0)
  rm -rf "$d"
}

# Revisa todos los paquetes de la carpeta $1 (el repo [vortex]) con elf_no_genericos. Si alguno
# trae un ELF que pide más que x86-64 básico, lo dice y devuelve 1: esa ISO no abriría ese programa
# en una CPU sin AVX2. Lo usan principal (al final) y construir.sh (antes de armar la ISO).
revisar_isa() {
  local repo="$1" f malos rc=0
  command -v readelf >/dev/null || { echo "Falta readelf (binutils) para revisar los paquetes." >&2; return 1; }
  for f in "$repo"/*.pkg.tar*; do
    [[ -f "$f" && "$f" != *.sig ]] || continue
    if ! malos="$(elf_no_genericos "$f")"; then
      echo "No pude abrir $f para revisarlo." >&2
      rc=1
    elif [[ -n "$malos" ]]; then
      echo "${f##*/} trae programas que piden más que un x86-64 básico (no abrirían sin AVX2):" >&2
      sed 's/^/    /' <<<"$malos" >&2
      rc=1
    fi
  done
  return "$rc"
}

# ¿El PKGBUILD de la carpeta $1 solo arma paquetes «any» (sin binarios)? (0 = sí)
solo_any() {
  local archs
  archs="$(cd "$1" && makepkg --printsrcinfo 2>/dev/null | sed -n 's/^[[:space:]]*arch = //p' | sort -u)" || return 1
  [[ "$archs" == any ]]
}

# El pacman.conf del chroot: solo los repos de Arch. Architecture va fija (con «auto», el pacman de
# CachyOS de quien construye podría tomar la CPU por x86_64_v3) y el caché es del chroot: el de
# quien construye tiene paquetes de CachyOS con el mismo nombre de archivo que los de Arch.
pacman_conf_arch() {
  cat <<EOF
[options]
HoldPkg     = pacman glibc
Architecture = x86_64
CacheDir    = $1
NoProgressBar
ParallelDownloads = 5
DownloadUser = alpm
SigLevel    = Required DatabaseOptional
LocalFileSigLevel = Optional

[core]
Include = /etc/pacman.d/mirrorlist

[extra]
Include = /etc/pacman.d/mirrorlist

[multilib]
Include = /etc/pacman.d/mirrorlist
EOF
}

# Crea el chroot de Arch (mkarchroot, con base-devel) o lo pone al día. Si su pacman.conf no es el
# de pacman_conf_arch (un chroot de otra versión de este script, o con otros repos), se rehace.
# La config de makepkg es la de x86_64 de devtools pasada por config_generica, sin paquetes -debug.
preparar_chroot() {
  local conf="$CHROOT/conf" raiz="$CHROOT/root" base=/usr/share/devtools/makepkg.conf.d/x86_64.conf
  if ! command -v mkarchroot >/dev/null || ! command -v makechrootpkg >/dev/null || [[ ! -r "$base" ]]; then
    echo "Falta devtools (mkarchroot, makechrootpkg): sudo pacman -S devtools" >&2
    return 1
  fi
  echo "==> Chroot limpio de Arch para los paquetes con binarios: $CHROOT"
  mkdir -p "$CHROOT/cache" || return 1
  rm -rf "$conf"
  config_generica "$conf" "$base" || return 1
  echo 'OPTIONS+=(!debug)' >>"$conf/makepkg.conf.d/zz-vortex-generico.conf"
  pacman_conf_arch "$CHROOT/cache" >"$conf/pacman.conf"
  if [[ -d "$raiz" ]] && ! diff -q <(grep -v '^CacheDir' "$conf/pacman.conf") \
    <(grep -v '^CacheDir' "$raiz/etc/pacman.conf" 2>/dev/null) >/dev/null; then
    echo "    su pacman.conf no es el de ahora: se rehace"
    sudo rm -rf --one-file-system -- "$raiz" "$CHROOT/$USER" || return 1
  fi
  if [[ -d "$raiz" ]]; then
    arch-nspawn -C "$conf/pacman.conf" -M "$conf/makepkg.conf" "$raiz" pacman -Syuu --noconfirm
  else
    mkarchroot -C "$conf/pacman.conf" -M "$conf/makepkg.conf" -c "$CHROOT/cache" "$raiz" base-devel
  fi
}

# preparar_chroot una sola vez por corrida (va en el shell principal, no en el subshell que compila).
chroot_listo() {
  ((CHROOT_LISTO)) && return 0
  preparar_chroot || return 1
  CHROOT_LISTO=1
}

# Compila el PKGBUILD de la carpeta actual en una copia limpia del chroot (-c), con la marca
# PACKAGER_CHROOT. makechrootpkg instala las dependencias en la copia y deja los paquetes en PKGDEST.
compilar_en_chroot() {
  local logs rc=0
  logs="$(mktemp -d "${TMPDIR:-/tmp}/vortex-chroot-logs.XXXXXX")" || return 1
  PACKAGER="$PACKAGER_CHROOT" LOGDEST="$logs" makechrootpkg -c -r "$CHROOT" || rc=$?
  if ((rc)); then echo "    falló; los registros quedan en $logs" >&2; else rm -rf "$logs"; fi
  return "$rc"
}

# La marca («packager») que debe llevar en el repo lo que arma el PKGBUILD de la carpeta $1.
marca_de() { if solo_any "$1"; then echo "$PACKAGER_GENERICO"; else echo "$PACKAGER_CHROOT"; fi; }

# Compila el PKGBUILD de la carpeta $1: lo que es «any» con makepkg_generico (no hay binarios, y
# sus dependencias pueden ser de CachyOS), lo demás en el chroot de Arch.
compilar() {
  if solo_any "$1"; then
    (cd "$1" && makepkg_generico -s --noconfirm --needed -f)
  else
    chroot_listo || return 1
    (cd "$1" && compilar_en_chroot)
  fi
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
    es_elf "$e" || continue
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
#  - el paquete no se compiló como toca (su «packager» del .PKGINFO no es $3, por defecto
#    PACKAGER_GENERICO; para lo que va en el chroot, PACKAGER_CHROOT): un paquete de antes de este
#    cambio, o hecho a mano, puede traer -march=native del constructor o la marca v3 de su glibc y
#    no arrancar en una CPU sin AVX2. Se recompila una vez;
#  - algún ELF del paquete pide más que x86-64 básico (elf_no_genericos), venga de donde venga.
ya_en_repo() {
  local dir="$1" repo="$2" marca="${3:-$PACKAGER_GENERICO}" lista=() f pkg deps=() info packager malos
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
    [[ "$packager" == "$marca" ]] || return 1
    mapfile -t deps < <(sed -n 's/^depend = //p' <<<"$info")
    ((${#deps[@]} == 0)) || pacman -T "${deps[@]}" >/dev/null 2>&1 || return 1
    enlaza_bien "$f" || return 1
    malos="$(elf_no_genericos "$f")" && [[ -z "$malos" ]] || return 1
  done
  return 0
}

# Compila cada paquete del AUR que haga falta (ver ya_en_repo), con compilar: los «any» en este
# equipo, los demás en el chroot de Arch.
construir_aur() {
  local repo="$1" aur p
  aur="$(mktemp -d)"
  # shellcheck disable=SC2064  # se expande ahora a propósito: $aur es local de esta función
  trap "rm -rf '$aur'" EXIT
  for p in "${AUR[@]}"; do
    echo "==> $p (AUR)"
    git clone -q --depth 1 "$AUR_URL/$p.git" "$aur/$p"
    if ya_en_repo "$aur/$p" "$repo" "$(marca_de "$aur/$p")"; then
      echo "    ya está en $repo con la misma versión del AUR: no se vuelve a compilar"
      continue
    fi
    compilar "$aur/$p"
  done
}

principal() {
  local REPO d
  REPO="$(realpath -m "${1:-/var/tmp/vortex-repo}")"
  [[ $EUID -ne 0 ]] || { echo "Corre esto como usuario normal (makepkg no corre como root)." >&2; exit 1; }
  mkdir -p "$REPO"
  export PKGDEST="$REPO"
  echo "==> Se compila con banderas de un x86-64 genérico (no las de ${VORTEX_MAKEPKG_CONF:-/etc/makepkg.conf}); lo que trae binarios, en un chroot de Arch"

  for d in "${PROPIOS[@]}"; do
    # vortex-miku (el asistente) puede faltar mientras su rama no entre: sin su PKGBUILD se avisa y
    # se sigue (con cualquier otro paquete propio, la falta sí es un error).
    if [[ "$d" == vortex-miku && ! -f "$RAIZ/packaging/$d/PKGBUILD" ]]; then
      echo "==> $d: no hay packaging/$d/PKGBUILD, se omite (¡la ISO lo pide en paquetes.txt!)" >&2
      continue
    fi
    echo "==> $d"
    compilar "$RAIZ/packaging/$d"
    rm -rf "$RAIZ/packaging/$d/"{pkg,src}
  done

  construir_aur "$REPO"

  # Solo la última versión de cada paquete, y la base de datos del repo.
  cd "$REPO"
  rm -f vortex.db* vortex.files*
  repo-add -q -R vortex.db.tar.gz ./*.pkg.tar.zst
  ls -la "$REPO"

  # Ningún programa del repo puede pedir más que x86-64 básico (si no, la ISO no lo abre sin AVX2).
  # Va después de repo-add: con -R ya se borraron las versiones viejas de cada paquete.
  echo "==> Revisando que ningún ELF del repo pida más que x86-64 básico (readelf -n)"
  revisar_isa "$REPO" || { echo "El repo $REPO no sirve para la ISO: hay programas que no abrirían en una CPU sin AVX2." >&2; exit 1; }
  echo "    todo en x86-64 básico"
}

# Se puede cargar con source (pruebas/iso.sh prueba ya_en_repo) sin armar nada.
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  principal "$@"
fi
