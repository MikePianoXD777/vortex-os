// Compila el puente (src/colores.rs) como el módulo QML «Vortex.Colores».
// API de cxx-qt-build 0.8+; si 0.10 cambió algo, ajustarlo con su documentación (README.md).
use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("Vortex.Colores"))
        .qt_module("Quick")
        .files(["src/colores.rs"])
        .build();
}
