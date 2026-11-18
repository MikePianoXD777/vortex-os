#pragma once
#include "cxx-qt-lib/qvariant.h"
#include "rust/cxx.h"

int vortex_app_installer_correr(rust::Str qml, rust::Str archivo, bool prueba, rust::Str capturas, rust::Str plan,
                                int ancho, int alto, rust::Str contrasena);
QVariant json_a_variante(rust::Str texto);
rust::String variante_a_json(const QVariant &valor);
void copiar_texto(rust::Str texto);
