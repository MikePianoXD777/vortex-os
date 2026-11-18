// Compila los QObject (Puente = «tm», Colores = «colores») y el arranque en C++ (cpp/arranque.cpp,
// con el modelo de la lista y el proveedor de íconos).
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.TaskManager"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/puente.rs", "src/colores.rs", "src/arranque.rs"])
        .include_dir("cpp")
        .cpp_file("cpp/arranque.cpp")
        .build();
    println!("cargo:rerun-if-changed=cpp/arranque.h");
}
