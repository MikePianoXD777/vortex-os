//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-runner», ícono), pone `runner` y `colores` en el contexto raíz, como hace
//! la de Python con `setContextProperty` (cxx-qt-lib no lo trae), y carga `Main.qml`.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `archivo`: un .exe que se pasó
        /// por la línea de comandos («» = ninguno). `prueba` = modo de pruebas (recorre la ventana, imprime
        /// «DATOS{json}» y sale, como `pruebas/test_interfaz.py`).
        fn vortex_runner_correr(qml: &str, archivo: &str, prueba: bool) -> i32;
    }
}
