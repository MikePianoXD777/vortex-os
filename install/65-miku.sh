# shellcheck shell=bash
if ((!VORTEX_MIKU)) || ! preguntar "¿Instalar a Miku, el asistente de voz (Super+O; Claude es su cerebro; la voz y el oído se bajan la primera vez que la abres)?" s; then
  info "Miku: omitida"
  return 0
fi

paso "Miku, el asistente de voz"

# El paquete vortex-miku (packaging/vortex-miku) trae la app, la voz (Supertonic 3) y el oído
# (whisper.cpp) y pide sus dependencias a los repos oficiales (pyside6, qt6-webengine, whisper-cpp,
# python-onnxruntime-cpu...); makepkg las instala solo. Los modelos de voz y de oído no vienen en el
# paquete: la app los baja la primera vez que la abres. Como el shell, se arma desde este repo.
# Es opcional: si falla se avisa y la instalación sigue; si la rama del asistente todavía no trae su
# paquete, se avisa y se sigue.
if [[ ! -f "$VORTEX_DIR/packaging/vortex-miku/PKGBUILD" ]]; then
  aviso "Miku: este código no trae packaging/vortex-miku todavía; se omite"
  return 0
elif [[ "$VORTEX_DRY_RUN" == 1 ]]; then
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
info "Ajustes > General > Miku en 3D, o «vortex miku avatar off». Necesita Claude activado (Ajustes > General > Claude)."
