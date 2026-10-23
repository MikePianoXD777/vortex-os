#!/usr/bin/env bash
# Pruebas de Vortexine's Shop (apps/shop/ y packaging/vortex-shop), sin tocar el sistema:
#   1. sintaxis: py_compile del motor, bash -n del lanzador y de los scripts, el .desktop y el ícono;
#   2. el motor (apps/shop/pruebas/test_motor.py): el catálogo contra un Flathub FALSO en 127.0.0.1 con
#      respuestas reales guardadas (búsqueda, categorías, fichas, caché, sin red, timeouts, descripciones
#      a texto, ids raros) y flatpak.py contra un «flatpak» FALSO (instalar con progreso, quitar, listar,
#      actualizar, errores en español, apps del sistema) y contra una libflatpak falsa;
#   3. la interfaz (Qt Quick sin pantalla): cola, instancia única, colores, capturas y una prueba de
#      integración con el motor real contra el Flathub y el flatpak falsos (buscar, detalle, instalar,
#      abrir y quitar con clics de verdad). VORTEX_TIENDA_SIN_UI=1 la salta;
#   3b. el lanzador (apps/shop/vortex-shop): encuentra el código y pasa los argumentos;
#   4. el paquete: makepkg --verifysource, --nobuild y un paquete armado en una carpeta temporal (sin
#      instalarlo ni usar sudo), con lo que debe llevar;
#   5. una prueba REAL de solo lectura contra la API de Flathub (búsqueda «minecraft», la ficha de Prism
#      Launcher, categorías), si hay internet. VORTEX_TIENDA_SIN_RED=1 la salta.
# El flatpak de verdad nunca instala, quita ni agrega remotos aquí: todo lo que cambia algo va al falso.
#
#   pruebas/tienda.sh
#
# Lo que esto NO cubre (se prueba a mano en la VM, con Flathub de verdad): empaquetar con makepkg dentro de
# la VM e instalar (sudo pacman -U), abrir la tienda en la sesión gráfica (por SSH: XDG_RUNTIME_DIR,
# WAYLAND_DISPLAY y HYPRLAND_INSTANCE_SIGNATURE del usuario; «setsid vortex-shop &»), sacar capturas con
# «grim» dentro de la VM y manejarla con «vm/vm.sh clic X Y» y «vm/vm.sh escribir TEXTO» (coordenadas de
# 1920x1080, las mismas de la captura): buscar, entrar a una app, Instalar (progreso), Abrir, Quitar,
# Super+I y el lanzador (Super+Espacio). Ojo: si la sesión de la VM se bloquea por inactividad grim solo
# ve la pantalla de bloqueo; reiniciar la VM la desbloquea (el autologin vuelve a entrar).
set -uo pipefail
AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TIENDA="$AQUI/apps/shop"
T="$(mktemp -d "${TMPDIR:-/tmp}/vortex-tienda-prueba.XXXXXX")"
FALLOS=0
trap 'rm -rf "$T"; rm -rf "$AQUI/packaging/vortex-shop/"{pkg,src}' EXIT

bien() { printf '  ok  %s\n' "$*"; }
mal() { printf '  MAL %s\n' "$*"; FALLOS=$((FALLOS + 1)); }
probar() { if eval "$2"; then bien "$1"; else mal "$1"; fi; }

command -v python3 >/dev/null || { echo "Falta python3 para las pruebas"; exit 1; }

echo "== Sintaxis"
probar "py_compile: el motor y las pruebas" \
  "python3 -I -c 'import py_compile,sys; [py_compile.compile(f, cfile=\"$T/x.pyc\", doraise=True) for f in sys.argv[1:]]' \
     '$TIENDA'/vortex_shop/*.py '$TIENDA'/pruebas/*.py"
for f in apps/shop/vortex-shop apps/shop/pruebas/flatpak_falso pruebas/tienda.sh packaging/vortex-shop/PKGBUILD \
  install/30-escritorio.sh iso/paquetes.sh; do
  probar "bash -n $f" "bash -n '$AQUI/$f'"
done
if command -v shellcheck >/dev/null; then
  probar "shellcheck del lanzador y de las pruebas" \
    "shellcheck -x -S warning '$TIENDA/vortex-shop' '$TIENDA/pruebas/flatpak_falso' '$AQUI/pruebas/tienda.sh'"
else
  echo "  --  shellcheck no está instalado: se salta"
fi
if command -v desktop-file-validate >/dev/null; then
  probar "el .desktop es válido" "desktop-file-validate '$TIENDA/share/applications/vortexines-shop.desktop' >/dev/null"
fi
probar "el .desktop: nombre, categorías, clase de ventana y palabras clave en español" \
  "grep -qx \"Name=Vortexine's Shop\" '$TIENDA/share/applications/vortexines-shop.desktop' &&
   grep -qx 'Categories=System;PackageManager;' '$TIENDA/share/applications/vortexines-shop.desktop' &&
   grep -qx 'StartupWMClass=vortexines-shop' '$TIENDA/share/applications/vortexines-shop.desktop' &&
   grep -q '^Keywords=.*tienda.*juegos.*instalar.*flatpak' '$TIENDA/share/applications/vortexines-shop.desktop'"
probar "el ícono es un SVG válido con los colores de Vortexine's" \
  "python3 -I -c 'import sys, xml.dom.minidom as m; m.parse(sys.argv[1])' '$TIENDA/share/icons/hicolor/scalable/apps/vortexines-shop.svg' &&
   grep -qi '#9ec5ff' '$TIENDA/share/icons/hicolor/scalable/apps/vortexines-shop.svg' &&
   grep -qi '#c6a8ff' '$TIENDA/share/icons/hicolor/scalable/apps/vortexines-shop.svg' &&
   grep -qi '#0b0d1a' '$TIENDA/share/icons/hicolor/scalable/apps/vortexines-shop.svg'"

echo "== Motor (catálogo contra un Flathub falso, flatpak falso)"
# Las pruebas no deben ver tu flatpak ni tu caché: HOME y PATH propios los pone test_motor.py.
if python3 -I "$TIENDA/pruebas/test_motor.py" >"$T/motor.log" 2>&1; then
  bien "test_motor.py: $(grep -E '^Ran ' "$T/motor.log")"
else
  mal "test_motor.py falló:"
  grep -E '^(ERROR|FAIL):' "$T/motor.log" | sed 's/^/        /'
  tail -n 40 "$T/motor.log" | sed 's/^/        /'
fi

echo "== Interfaz (Qt Quick sin pantalla: QT_QPA_PLATFORM=offscreen)"
# Necesita PySide6. Si el sistema no lo trae, se usa (o se arma) un venv en ~/.cache/vortex-shop-pruebas.
# VORTEX_TIENDA_SIN_UI=1 la salta.
PY_UI=""
if [[ -n "${VORTEX_TIENDA_SIN_UI:-}" ]]; then
  echo "  --  VORTEX_TIENDA_SIN_UI: se salta"
elif python3 -c 'import PySide6.QtQuick' 2>/dev/null; then
  PY_UI=python3
else
  VENV_UI="${XDG_CACHE_HOME:-$HOME/.cache}/vortex-shop-pruebas"
  if [[ ! -x "$VENV_UI/bin/python" ]]; then
    python3 -m venv "$VENV_UI" >/dev/null 2>&1 && "$VENV_UI/bin/pip" install -q PySide6-Essentials >/dev/null 2>&1
  fi
  [[ -x "$VENV_UI/bin/python" ]] && "$VENV_UI/bin/python" -c 'import PySide6.QtQuick' 2>/dev/null && PY_UI="$VENV_UI/bin/python"
  [[ -n "$PY_UI" ]] || echo "  --  sin PySide6 (ni en el sistema ni en $VENV_UI): se salta la interfaz"
fi
if [[ -n "$PY_UI" ]]; then
  export QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software
  for prueba in ui_cola ui_instancia ui_tema ui_capturas ui_integracion; do
    # ui_tema y ui_capturas guardan capturas: a la carpeta temporal, no a tu ~/Vídeos.
    if "$PY_UI" "$TIENDA/pruebas/$prueba.py" "$T/capturas-$prueba" >"$T/$prueba.log" 2>&1; then
      bien "$prueba.py: $(grep -c '^ok' "$T/$prueba.log") comprobaciones"
    else
      mal "$prueba.py falló (mira $T/$prueba.log):"
      grep -E '^FALLA|Traceback|Error' "$T/$prueba.log" | head -n 8 | sed 's/^/        /'
    fi
  done
  unset QT_QPA_PLATFORM QT_QUICK_BACKEND
fi

echo "== Lanzador"
LAN="$T/lanzador"
mkdir -p "$LAN/copia" "$LAN/codigo/vortex_shop" "$LAN/vacio"
cp "$TIENDA/vortex-shop" "$LAN/copia/vortex-shop"
cat >"$LAN/codigo/vortex_shop/__init__.py" <<'PY'
PY
cat >"$LAN/codigo/vortex_shop/__main__.py" <<'PY'
import os, sys
print("ARGS=" + "|".join(sys.argv[1:]))
print("RAIZ=" + os.environ["PYTHONPATH"].split(os.pathsep)[0])
print("RESTO=" + os.environ.get("PYTHONPATH", "").partition(os.pathsep)[2])
PY
salida="$(VORTEX_SHOP_DIR="$LAN/codigo" PYTHONPATH=/otra/ruta "$LAN/copia/vortex-shop" --uno "dos tres" 2>&1)"
probar "VORTEX_SHOP_DIR: corre python -m vortex_shop con ese código" "grep -qx 'RAIZ=$LAN/codigo' <<<\"\$salida\""
probar "pasa los argumentos tal cual (con espacios)" "grep -qx 'ARGS=--uno|dos tres' <<<\"\$salida\""
probar "conserva el PYTHONPATH que ya había" "grep -qx 'RESTO=/otra/ruta' <<<\"\$salida\""
# Desde el repo (sin variable): usa la carpeta del propio script.
cp -r "$LAN/codigo/vortex_shop" "$LAN/copia/vortex_shop"
salida="$("$LAN/copia/vortex-shop" a 2>&1)"
probar "sin variable usa el código que está junto al lanzador (el repo)" "grep -qx 'RAIZ=$LAN/copia' <<<\"\$salida\""
# Por un enlace simbólico (como /usr/bin/vortex-shop → el repo).
ln -s "$LAN/copia/vortex-shop" "$LAN/enlace"
salida="$("$LAN/enlace" b 2>&1)"
probar "por un enlace simbólico también encuentra su carpeta" "grep -qx 'RAIZ=$LAN/copia' <<<\"\$salida\""
# Sin código por ningún lado (y sin el paquete instalado): avisa en español y sale con 1.
if [[ ! -f /usr/share/vortex-shop/vortex_shop/__init__.py ]]; then
  mkdir -p "$LAN/solo"
  cp "$TIENDA/vortex-shop" "$LAN/solo/vortex-shop"
  salida="$(VORTEX_SHOP_DIR="$LAN/vacio" "$LAN/solo/vortex-shop" 2>&1)"
  rc=$?
  probar "sin código: sale con 1" "[[ $rc == 1 ]]"
  probar "sin código: lo dice en español" "grep -q 'no encuentro su código' <<<\"\$salida\""
else
  echo "  --  vortex-shop está instalada en /usr/share: se salta la prueba de «sin código»"
fi

echo "== Paquete (packaging/vortex-shop)"
if command -v makepkg >/dev/null && [[ $EUID -ne 0 ]]; then
  PK="$AQUI/packaging/vortex-shop"
  export BUILDDIR="$T/build" PKGDEST="$T/paquetes" SRCDEST="$T/fuentes" SRCPKGDEST="$T/fuentes-paquete"
  mkdir -p "$BUILDDIR" "$PKGDEST" "$SRCDEST"
  probar "makepkg --verifysource" "(cd '$PK' && makepkg --verifysource >'$T/verify.log' 2>&1)"
  probar "makepkg --nobuild" "(cd '$PK' && makepkg --nobuild --force >'$T/nobuild.log' 2>&1)"
  if (cd "$PK" && makepkg --force --nodeps >"$T/paquete.log" 2>&1); then
    bien "makepkg arma el paquete (sin instalarlo)"
    pkg="$(compgen -G "$PKGDEST/vortex-shop-*.pkg.tar.*" | head -n1)"
    lista="$(bsdtar -tf "$pkg")"
    for archivo in usr/bin/vortex-shop usr/share/vortex-shop/vortex_shop/__init__.py \
      usr/share/vortex-shop/vortex_shop/catalogo.py usr/share/vortex-shop/vortex_shop/flatpak.py \
      usr/share/vortex-shop/vortex_shop/__main__.py usr/share/vortex-shop/vortex_shop/puente.py \
      usr/share/vortex-shop/vortex_shop/colores.py usr/share/vortex-shop/vortex_shop/qml/Main.qml \
      usr/share/vortex-shop/vortex_shop/qml/qmldir usr/share/vortex-shop/vortex_shop/qml/vortex-os.svg \
      usr/share/applications/vortexines-shop.desktop usr/share/icons/hicolor/scalable/apps/vortexines-shop.svg \
      usr/share/licenses/vortex-shop/LICENSE; do
      probar "el paquete lleva $archivo" "grep -qx '$archivo' <<<\"\$lista\""
    done
    probar "el paquete no lleva restos de pruebas ni de caché de otros" \
      "! grep -E 'pruebas/|\.tmp$|/pkg/|/src/' <<<\"\$lista\" >/dev/null"
    info="$(bsdtar -xOf "$pkg" .PKGINFO)"
    probar "arch=any" "grep -qx 'arch = any' <<<\"\$info\""
    for dependencia in python pyside6 qt6-declarative qt6-svg python-gobject flatpak; do
      probar "depende de $dependencia" "grep -qx 'depend = $dependencia' <<<\"\$info\""
    done
    probar "el lanzador instalado es el mismo del repo y ejecutable" \
      "[[ \"\$(bsdtar -xOf '$pkg' usr/bin/vortex-shop | sha256sum)\" == \"\$(sha256sum <'$TIENDA/vortex-shop')\" ]] && bsdtar -tvf '$pkg' usr/bin/vortex-shop | grep -q '^-rwxr-xr-x'"
  else
    mal "makepkg no armó el paquete (mira $T/paquete.log):"
    tail -n 15 "$T/paquete.log" | sed 's/^/        /'
  fi
  unset BUILDDIR PKGDEST SRCDEST SRCPKGDEST
else
  echo "  --  sin makepkg (o corriendo como root): se salta el paquete"
fi
probar "iso/paquetes.sh arma vortex-shop con los demás paquetes propios" \
  "grep -E '^PROPIOS=.*\\bvortex-shop\\b' '$AQUI/iso/paquetes.sh' >/dev/null"
probar "iso/paquetes.txt lleva vortex-shop (y pyside6 explícito)" \
  "grep -qx 'vortex-shop' '$AQUI/iso/paquetes.txt' && grep -qx 'pyside6' '$AQUI/iso/paquetes.txt'"
probar "install/30-escritorio.sh instala la tienda como el shell" \
  "grep -q 'packaging/vortex-shop' '$AQUI/install/30-escritorio.sh' && grep -q 'makepkg -sif --noconfirm' '$AQUI/install/30-escritorio.sh'"

echo "== Flathub de verdad (solo lectura)"
if [[ -n "${VORTEX_TIENDA_SIN_RED:-}" ]]; then
  echo "  --  VORTEX_TIENDA_SIN_RED: se salta"
else
  python3 -I "$TIENDA/pruebas/real_flathub.py"
  case $? in
    0) bien "la API real de Flathub responde como espera la tienda" ;;
    2) ;;
    *) mal "la prueba real contra Flathub falló" ;;
  esac
fi

echo
if ((FALLOS == 0)); then
  echo "Todas las pruebas de la tienda pasaron."
else
  echo "$FALLOS prueba(s) de la tienda fallaron."
  exit 1
fi
