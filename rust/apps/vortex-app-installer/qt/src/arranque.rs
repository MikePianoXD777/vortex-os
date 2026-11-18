//! El arranque en C++ (`cpp/arranque.cpp`): crea la app con lo mismo que `__main__.py` (estilo Basic,
//! Adwaita Sans 14 px, id «vortex-app-installer», ícono), pone `instalador` y `colores` en el contexto raíz,
//! como hace la de Python con `setContextProperty` (cxx-qt-lib no lo trae), y carga `Main.qml`.
//! También trae tres ayudas que Qt tiene y cxx-qt-lib no: de JSON a `QVariant` y de vuelta, y el portapapeles.
#![allow(unsafe_code)] // el código que genera cxx

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    unsafe extern "C++" {
        include!("arranque.h");
        /// Corre la app hasta que se cierra; devuelve el código de salida.
        ///
        /// - `archivo`: lo que se pasó por la línea de comandos («» = ninguno).
        /// - `prueba`: recorre la ventana con los archivos de `VORTEX_APP_INSTALLER_PRUEBA_ARCHIVOS`, imprime
        ///   «DATOS{json}» y sale (como `pruebas/test_interfaz.py`).
        /// - `capturas`: carpeta donde guardar capturas (como `pruebas/capturas.py`); `plan` son los archivos
        ///   a abrir, uno por renglón, «ruta» o «ruta<TAB>acción».
        #[allow(clippy::too_many_arguments)] // el arranque en C++ recibe todo de una vez
        fn vortex_app_installer_correr(
            qml: &str,
            archivo: &str,
            prueba: bool,
            capturas: &str,
            plan: &str,
            ancho: i32,
            alto: i32,
            contrasena: &str,
        ) -> i32;
        /// Un JSON a lo que QML entiende como objeto, lista o valor (`QJsonDocument::toVariant`).
        fn json_a_variante(texto: &str) -> QVariant;
        /// Lo que manda QML (un objeto, una lista, un `QUrl`…) a JSON.
        fn variante_a_json(valor: &QVariant) -> String;
        /// Al portapapeles (`QGuiApplication::clipboard`).
        fn copiar_texto(texto: &str);
    }
}
