//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `ventana.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-shared», ícono), pone `vs` y `colores` en el contexto raíz, como hace la
//! de Python con `setContextProperty` (cxx-qt-lib no lo trae), y carga `Main.qml`. También da el portapapeles.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `recorrer` = modo de pruebas
        /// (recorre la ventana, imprime «DATOS{json}» y sale, como `pruebas/test_interfaz.py`); `humo` =
        /// `--prueba-humo` (carga la ventana y se cierra a los 3 s).
        fn vortex_shared_correr(qml: &str, recorrer: bool, humo: bool) -> i32;
        /// Pone el texto en el portapapeles (`QGuiApplication::clipboard()`).
        fn vortex_shared_copiar(texto: &str);
    }
}
