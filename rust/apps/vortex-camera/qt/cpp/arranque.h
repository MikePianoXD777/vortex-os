#pragma once
#include "cxx-qt-lib/qvariant.h"
#include "rust/cxx.h"

int vortex_camera_correr(rust::Str qml, bool prueba);
QVariant json_a_variante(rust::Str texto);
rust::String variante_a_json(const QVariant &valor);
