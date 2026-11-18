//! El arranque en C++ (`cpp/arranque.cpp`): lo de `__main__.py` que cxx-qt-lib no trae (el estilo, la
//! fuente, el tema de íconos, `setContextProperty`, el proveedor `image://icono` y el modelo de la lista).
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;

        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `prueba` = modo de pruebas
        /// (recorre las páginas, imprime «DATOS{json}» y sale, como `pruebas/test_interfaz.py`).
        fn vortex_tm_correr(qml: &str, prueba: bool) -> i32;
        fn vortex_tm_json_a_variant(json: &str) -> QVariant;
        fn vortex_tm_icono_en_tema(nombre: &str) -> bool;
        fn vortex_tm_copiar(texto: &str);
        fn vortex_tm_filas_actualizar(ops: &str, filas: &str);
    }
}
