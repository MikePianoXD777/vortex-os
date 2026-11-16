// Compila los QObject (Puente = «vv», Colores = «colores») y el arranque en C++ (cpp/arranque.cpp).
// API de cxx-qt-build 0.8+; si la 0.10 cambió algo, ajustarlo con su documentación.
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.Video"))
        .qt_module("Quick")
        .qt_module("QuickControls2")
        .files(["src/puente.rs", "src/colores.rs", "src/arranque.rs"])
        .cc_builder(|cc| {
            cc.include("cpp");
            cc.file("cpp/arranque.cpp");
        })
        .build();
    println!("cargo:rerun-if-changed=cpp/arranque.cpp");
    println!("cargo:rerun-if-changed=cpp/arranque.h");
}
