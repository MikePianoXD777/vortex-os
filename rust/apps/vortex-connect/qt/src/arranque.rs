//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `ventana.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-connect», ícono), pone `vc` y `colores` en el contexto raíz, como hace la
//! de Python con `setContextProperty` (cxx-qt-lib no lo trae), y carga `Main.qml`.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `recorrer` = modo de pruebas
        /// (recorre la ventana, imprime «DATOS{json}» y sale, como `pruebas/test_interfaz.py`); `humo` =
        /// `--prueba-humo` (carga la ventana y se cierra a los 3 s).
        fn vortex_connect_correr(qml: &str, recorrer: bool, humo: bool) -> i32;
    }
}
