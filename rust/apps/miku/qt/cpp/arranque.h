#pragma once
#include "cxx-qt-lib/qvariant.h"
#include "rust/cxx.h"

int vortex_miku_correr(rust::Str qml, bool avatar, bool prueba);
QVariant json_a_variante(rust::Str texto);
