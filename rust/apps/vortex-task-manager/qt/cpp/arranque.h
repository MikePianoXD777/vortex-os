#pragma once
#include <QtCore/QVariant>
#include "rust/cxx.h"

// Arranca la app (QGuiApplication, estilo, fuente, íconos, modelo de la lista y los dos QObject de Rust).
int vortex_tm_correr(rust::Str qml, bool prueba);
// JSON (texto) → QVariant (mapa, lista, número…), para pasarle a QML lo que calcula el motor.
QVariant vortex_tm_json_a_variant(rust::Str json);
// Si el tema de íconos tiene ese ícono.
bool vortex_tm_icono_en_tema(rust::Str nombre);
void vortex_tm_copiar(rust::Str texto);
// Aplica al modelo de la lista las operaciones [[i1,i2,j1,j2],…] y deja las filas nuevas.
void vortex_tm_filas_actualizar(rust::Str ops, rust::Str filas);
