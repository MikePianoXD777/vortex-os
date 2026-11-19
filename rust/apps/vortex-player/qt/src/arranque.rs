//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-player»), pone `vp`, `rep` y `colores` en el contexto raíz (cxx-qt-lib no
//! trae `setContextProperty`) y carga `Main.qml`. Y lo poco de Qt que el motor necesita: JSON ↔ QVariant,
//! el portapapeles, el color de una portada, si la conexión es de datos medidos y cerrar la app.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida. `prueba` = modo de pruebas
        /// (`tests/interfaz.rs`: hace lo de `pruebas/test_interfaz.py` e imprime «DATOS{json}»).
        fn vortex_player_correr(qml: &str, prueba: bool) -> i32;
        /// Un valor JSON (texto) como lo entiende QML: objeto, lista, número, texto o booleano.
        fn json_a_variante(texto: &str) -> QVariant;
        /// Lo que manda QML (un objeto de JavaScript, una lista de URL…) como texto JSON.
        fn variante_a_json(valor: &QVariant) -> String;
        /// Copia el texto al portapapeles.
        fn copiar_texto(texto: &str);
        /// El color de la portada (el degradado de la playlist y de la letra, como Spotify); «» si no se pudo.
        fn color_de_imagen(archivo: &str) -> String;
        /// ¿La conexión se cobra por lo que gastas (datos del cel)?
        fn datos_medidos() -> bool;
        /// Cierra la app (MPRIS «Quit»).
        fn salir_de_la_app();
    }
}
