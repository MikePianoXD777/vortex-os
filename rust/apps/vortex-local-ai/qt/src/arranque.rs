//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-local-ai»), pone `ia`, `chat`, `modelos`, `boveda` y `colores` en el
//! contexto raíz (cxx-qt-lib no trae `setContextProperty`) y carga `Main.qml`. También: el paso JSON ↔
//! QVariant (QML ve las listas y los objetos que arma el motor como arreglos y objetos de JavaScript) y las
//! listas de los QML (`cpp/modelo.h`).
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `guion` = el guion de pruebas
        /// (`tests/interfaz.rs`), o vacío para la app de siempre.
        fn vortex_local_ai_correr(qml: &str, guion: &str) -> i32;
        /// Un valor JSON (texto) como lo entiende QML: objeto, lista, número, texto o booleano.
        fn json_a_variante(texto: &str) -> QVariant;

        fn modelo_nuevo(roles: &str, defectos: &str) -> usize;
        fn modelo_poner(m: usize, filas: &str);
        fn modelo_agregar(m: usize, fila: &str);
        fn modelo_quitar_desde(m: usize, i: i32);
        fn modelo_cambiar(m: usize, i: i32, campos: &str);
        fn modelo_fila(m: usize, i: i32) -> String;
        fn modelo_cuenta(m: usize) -> i32;
        fn modelo_a_variante(m: usize) -> QVariant;

        fn copiar_al_portapapeles(texto: &str);
        fn url_a_local(url: &str) -> String;
    }
}
