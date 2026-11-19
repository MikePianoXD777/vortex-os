// Compila los QObject (General = «ia», Modelos, Chat, Boveda, Colores) y el arranque en C++ (cpp/): el
// arranque (cpp/arranque.cpp) y el modelo de listas que ven los QML (cpp/modelo.h, con moc).
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.LocalAI"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files([
            "src/general.rs",
            "src/modelos.rs",
            "src/chat.rs",
            "src/boveda.rs",
            "src/colores.rs",
            "src/arranque.rs",
        ])
        .include_dir("cpp")
        .cpp_file("cpp/modelo.h")
        .cpp_file("cpp/modelo.cpp")
        .cpp_file("cpp/prueba.h")
        .cpp_file("cpp/prueba.cpp")
        .cpp_file("cpp/arranque.cpp")
        .build();
    println!("cargo:rerun-if-changed=cpp/arranque.h");
}
