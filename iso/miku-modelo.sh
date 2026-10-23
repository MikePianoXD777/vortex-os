# shellcheck shell=bash
# El modelo 3D de Miku en la imagen de la ISO. Lo carga iso/construir.sh con source (y
# pruebas/miku.sh lo prueba sin root ni archiso).
#
# El modelo NO va en el repo (*.vrm está en .gitignore) ni en el paquete vortex-miku: es de su autor
# y su licencia pide crédito. Se pone al construir la ISO, desde una carpeta con modelo.vrm y
# CREDITO.txt:
#
#   sudo VORTEX_MIKU_MODELO=/ruta/a/la/carpeta iso/construir.sh
#
# Sin la variable se toma la carpeta de quien corre sudo, ~/.local/share/vortex/miku (donde la deja
# «vortex-miku modelo»); con la variable vacía (VORTEX_MIKU_MODELO=) la ISO sale sin modelo. Va a
# /usr/share/vortex-miku/modelo/ de la imagen (0644, root), el último sitio donde la app lo busca.
# Sin modelo no falla: avisa y sigue (el asistente funciona y la app muestra cómo poner uno).

# Carpeta de la que sale el modelo (vacía si no hay de dónde).
miku_carpeta_modelo() {
  local casa=""
  if [[ -n "${VORTEX_MIKU_MODELO+x}" ]]; then
    printf '%s\n' "$VORTEX_MIKU_MODELO"
    return 0
  fi
  if [[ -n "${SUDO_USER:-}" ]]; then casa="$(getent passwd "$SUDO_USER" | cut -d: -f6)" || casa=""; fi
  if [[ -n "$casa" ]]; then printf '%s\n' "$casa/.local/share/vortex/miku"; fi
  return 0
}

# miku_poner_modelo RAIZ_DEL_AIROOTFS: copia modelo.vrm (y CREDITO.txt) a RAIZ/usr/share/vortex-miku/modelo/.
# Deja en MIKU_ARCHIVOS las rutas de lo copiado dentro de la imagen (para los permisos de
# profiledef.sh). Los avisos van a stderr.
# shellcheck disable=SC2034  # la lee iso/construir.sh
MIKU_ARCHIVOS=()
miku_poner_modelo() {
  local raiz="$1" carpeta destino dueno=()
  MIKU_ARCHIVOS=()
  carpeta="$(miku_carpeta_modelo)"
  destino="$raiz/usr/share/vortex-miku/modelo"
  # Como root, que el dueño sea root aunque quien construye tenga otro.
  [[ $EUID -ne 0 ]] || dueno=(-o root -g root)

  if [[ -z "$carpeta" || ! -f "$carpeta/modelo.vrm" ]]; then
    echo "Aviso: la ISO sale sin el modelo 3D de Miku (no hay modelo.vrm en ${carpeta:-ninguna carpeta}; usa VORTEX_MIKU_MODELO=carpeta con modelo.vrm y CREDITO.txt)." >&2
    echo "       El asistente funciona igual y la app dice cómo poner un modelo (vortex-miku modelo RUTA.vrm)." >&2
    return 0
  fi
  # Un VRM es un glTF binario: empieza con «glTF» (67 6c 54 46, en hexadecimal: bash no traga bien
  # los bytes nulos de otros archivos).
  if [[ "$(head -c4 "$carpeta/modelo.vrm" 2>/dev/null | od -An -tx1 | tr -d ' \n')" != 676c5446 ]]; then
    echo "Aviso: $carpeta/modelo.vrm no es un modelo VRM (glTF binario): la ISO sale sin modelo de Miku." >&2
    return 0
  fi

  install -Dm644 "${dueno[@]}" "$carpeta/modelo.vrm" "$destino/modelo.vrm"
  MIKU_ARCHIVOS+=(/usr/share/vortex-miku/modelo/modelo.vrm)
  if [[ -f "$carpeta/CREDITO.txt" ]]; then
    install -Dm644 "${dueno[@]}" "$carpeta/CREDITO.txt" "$destino/CREDITO.txt"
    MIKU_ARCHIVOS+=(/usr/share/vortex-miku/modelo/CREDITO.txt)
  else
    echo "Aviso: $carpeta no tiene CREDITO.txt y la licencia del modelo pide dar crédito: ponlo antes de publicar la ISO." >&2
  fi
  echo "Miku: modelo 3D de $carpeta (${#MIKU_ARCHIVOS[@]} archivo(s)) en /usr/share/vortex-miku/modelo/"
}
