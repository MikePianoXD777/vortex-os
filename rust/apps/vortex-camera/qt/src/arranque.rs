//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-camera», ffmpeg callado), pone `cam` y `colores` en el contexto raíz
//! (cxx-qt-lib no trae `setContextProperty`) y carga `Main.qml`. Y los dos pasos JSON ↔ QVariant: QML ve
//! las listas y los objetos que arma el motor como arreglos y objetos de JavaScript.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `prueba` = modo de pruebas
        /// (`tests/interfaz.rs`: hace lo de `pruebas/test_interfaz.py` e imprime «DATOS{json}»).
        fn vortex_camera_correr(qml: &str, prueba: bool) -> i32;
        /// Un valor JSON (texto) como lo entiende QML: objeto, lista, número, texto o booleano.
        fn json_a_variante(texto: &str) -> QVariant;
        /// Lo que manda QML (un objeto de JavaScript) como texto JSON.
        fn variante_a_json(valor: &QVariant) -> String;
    }
}
