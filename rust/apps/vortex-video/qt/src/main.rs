//! `vortex-video-rs`: Vortexine Video en Rust. Se instala aparte de `vortex-video` mientras se prueba.
//!
//! Usa los mismos QML que la de Python (no se copian): los busca como el lanzador `vortex-video`.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_video::instancia;

/// Donde están los QML: VORTEX_VIDEO_DIR, el repo (al compilar desde aquí) o /usr/share/vortex-video.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../apps/vortex-video"
    ));
    std::env::var_os("VORTEX_VIDEO_DIR")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-video")])
        .map(|c| c.join("vortex_video").join("qml"))
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_video_qt);
    if std::env::args().any(|a| a == "--version") {
        println!("{} {} (Rust)", vortex_video::NOMBRE, vortex_video::VERSION);
        return;
    }
    // Si ya hay una abierta (de Rust o de Python), que se muestre esa y salir.
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre())) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine Video no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-video-rs");
    let prueba = std::env::var_os("VORTEX_VIDEO_PRUEBA").is_some();
    let codigo = arranque::ffi::vortex_video_correr(&qml.join("Main.qml").to_string_lossy(), prueba);
    std::process::exit(codigo);
}
