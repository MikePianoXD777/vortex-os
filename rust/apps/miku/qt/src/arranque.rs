//! El arranque en C++ (`cpp/arranque.cpp`): la app con lo mismo que `app.py` (app_id «vortex-miku», estilo
//! Basic, SIGTERM que cierra bien), `mk` en el contexto raíz (cxx-qt-lib no trae `setContextProperty`),
//! QtWebEngine solo si hay avatar, y el modo de pruebas de `tests/interfaz.rs`.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida.
        fn vortex_miku_correr(qml: &str, avatar: bool, prueba: bool) -> i32;
        /// Un valor JSON (texto) como lo entiende QML.
        fn json_a_variante(texto: &str) -> QVariant;
    }
}
