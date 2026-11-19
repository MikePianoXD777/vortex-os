#pragma once
#include "cxx-qt-lib/qvariant.h"
#include "rust/cxx.h"
#include <cstddef>

int vortex_local_ai_correr(rust::Str qml, rust::Str guion);
QVariant json_a_variante(rust::Str texto);

// Las listas de los QML (cpp/modelo.h): el «asa» es el puntero del ModeloLista.
std::size_t modelo_nuevo(rust::Str roles, rust::Str defectos);
void modelo_poner(std::size_t m, rust::Str filas);
void modelo_agregar(std::size_t m, rust::Str fila);
void modelo_quitar_desde(std::size_t m, int i);
void modelo_cambiar(std::size_t m, int i, rust::Str campos);
rust::String modelo_fila(std::size_t m, int i);
int modelo_cuenta(std::size_t m);
QVariant modelo_a_variante(std::size_t m);

void copiar_al_portapapeles(rust::Str texto);
rust::String url_a_local(rust::Str url);
