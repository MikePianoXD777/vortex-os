//! `vortex-camera-rs`: Vortexine's Camera en Rust. El lanzador de siempre (`vortex-camera`) salta a este
//! binario si existe (`VORTEX_CAMERA_PYTHON=1` fuerza la de Python).
//!
//! Usa los mismos QML que la de Python (no se copian al repo): los busca donde los deja ella.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_camera::instancia;

/// Donde están los QML: VORTEX_CAMERA_DIR (el lanzador la pone), el repo (al compilar desde aquí), donde
/// la deja «vortex camara instalar» y la copia del paquete vortex-rust.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../apps/vortex-camera"
    ));
    let instalada = vortex_core::rutas::datos()
        .parent()
        .map(|d| d.join("vortex-camera"))
        .unwrap_or_default();
    std::env::var_os("VORTEX_CAMERA_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, instalada])
        .map(|c| c.join("vortex_camera").join("qml"))
        .chain([PathBuf::from("/usr/share/vortex-camera-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_camera_qt);
    if std::env::args().any(|a| a == "--version") {
        println!("{} {} (Rust)", vortex_camera::NOMBRE, vortex_camera::VERSION);
        return;
    }
    // Si ya hay una abierta (de Rust o de Python), que se muestre esa y salir: dos ventanas abrirían dos
    // veces cada cámara.
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre())) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine's Camera no está instalada del todo: instálala con «vortex camara instalar».");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-camera-rs");
    let prueba = std::env::var_os("VORTEX_CAMERA_PRUEBA").is_some();
    let codigo = arranque::ffi::vortex_camera_correr(&qml.join("Main.qml").to_string_lossy(), prueba);
    std::process::exit(codigo);
}
