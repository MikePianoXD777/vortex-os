//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-video»), pone `vv` y `colores` en el contexto raíz, como hace la de Python
//! con `setContextProperty` (cxx-qt-lib no lo trae), y carga `Main.qml`.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `prueba` = modo de pruebas
        /// (imprime «DATOS{json}» a los 800 ms y sale, como `pruebas/test_interfaz.py`).
        fn vortex_video_correr(qml: &str, prueba: bool) -> i32;
    }
}
