# shellcheck shell=bash
if ((!VORTEX_MIKU)) || ! preguntar "¿Instalar a Miku, el asistente de voz (Super+O; Claude es su cerebro; el paquete baja unos 450 MB de voz y oído)?" s; then
  info "Miku: omitida"
  return 0
fi

paso "Miku, el asistente de voz"

# El paquete vortex-miku (packaging/vortex-miku) trae la app, la voz y el oído con sus modelos
# (funcionan sin internet) y pide sus dependencias a los repos oficiales (pyside6, qt6-webengine,
# whisper-cpp, espeak-ng...); makepkg las instala solo. Como el shell, se arma desde este repo.
# Es opcional: si falla se avisa y la instalación sigue.
if [[ "$VORTEX_DRY_RUN" == 1 ]]; then
  run makepkg -sif --noconfirm "(en $VORTEX_DIR/packaging/vortex-miku)"
elif (cd "$VORTEX_DIR/packaging/vortex-miku" && con_reintentos makepkg -sif --noconfirm); then
  rm -rf "$VORTEX_DIR/packaging/vortex-miku/"{pkg,src} "$VORTEX_DIR/packaging/vortex-miku/"*.pkg.tar.*
  bien "Miku instalada"
else
  aviso "No se pudo instalar a Miku. Cuando tengas internet, corre: cd $VORTEX_DIR/packaging/vortex-miku && makepkg -sif"
  return 0
fi

# El modelo 3D no viene con el instalador (es de su autor y su licencia pide crédito): sin él, Miku
# funciona igual como asistente de voz, y su ventana dice cómo poner uno (vortex-miku modelo RUTA.vrm).
info "Super+O abre y cierra a Miku. Le hablas con el micrófono (solo escucha con su ventana abierta) y"
info "ve tu pantalla solo cuando se lo pides, avisándote con una notificación. Para dejarla solo como voz:"
info "Ajustes > General > Miku en 3D, o «vortex miku avatar off». Necesita Claude (paso anterior)."
