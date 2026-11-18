// Compila los QObject (Puente = «tienda», Colores = «colores») y el arranque en C++ (cpp/arranque.cpp).
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.Shop"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/puente.rs", "src/colores.rs", "src/arranque.rs"])
        .include_dir("cpp")
        .cpp_file("cpp/arranque.cpp")
        .build();
    println!("cargo:rerun-if-changed=cpp/arranque.h");
    println!("cargo:rerun-if-changed=cpp/json.h");
}
