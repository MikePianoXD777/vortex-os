// Compila los QObject (Puente = «vp», Rep = «rep», Colores = «colores») y el arranque en C++.
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.Player"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .qt_module("Network")
        .files(["src/puente.rs", "src/rep.rs", "src/colores.rs", "src/arranque.rs"])
        .include_dir("cpp")
        .cpp_file("cpp/arranque.cpp")
        .build();
    println!("cargo:rerun-if-changed=cpp/arranque.h");
}
