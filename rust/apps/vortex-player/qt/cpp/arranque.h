#pragma once
#include "cxx-qt-lib/qvariant.h"
#include "rust/cxx.h"

int vortex_player_correr(rust::Str qml, bool prueba);
QVariant json_a_variante(rust::Str texto);
rust::String variante_a_json(const QVariant &valor);
void copiar_texto(rust::Str texto);
rust::String color_de_imagen(rust::Str archivo);
bool datos_medidos();
void salir_de_la_app();
