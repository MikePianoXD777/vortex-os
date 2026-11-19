//! `vortex-local-ai-rs`: Vortexine's Local AI en Rust. El lanzador de siempre (`vortex-local-ai`) salta a
//! este binario si existe (`VORTEX_LOCAL_AI_PYTHON=1` fuerza la de Python).
//!
//! Usa los mismos QML que la de Python (no se copian al repo): los busca donde los deja ella.

mod arranque;
mod boveda;
mod chat;
mod colores;
mod comun;
mod general;
mod lista;
mod modelos;

use std::path::PathBuf;
use vortex_local_ai::instancia;

/// Donde están los QML: VORTEX_LOCAL_AI_DIR (el lanzador la pone), el repo (al compilar desde aquí), donde
/// la deja «vortex actualizar» y la copia del paquete vortex-rust.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../apps/vortex-local-ai"
    ));
    let instalada = vortex_core::rutas::datos()
        .parent()
        .map(|d| d.join("vortex-local-ai"))
        .unwrap_or_default();
    std::env::var_os("VORTEX_LOCAL_AI_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, instalada])
        .map(|c| c.join("vortex_local_ai").join("qml"))
        .chain([PathBuf::from("/usr/share/vortex-local-ai-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_local_ai_qt);
    if std::env::args().any(|a| a == "--version") {
        println!("{} {} (Rust)", vortex_local_ai::NOMBRE, vortex_local_ai::VERSION);
        return;
    }
    // Si ya hay una abierta (de Rust o de Python), que se muestre esa y salir: una sola ventana por persona.
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre())) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine's Local AI no está instalada del todo: instálala con «vortex actualizar».");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-local-ai-rs");
    // Solo para `tests/interfaz.rs`: un guion de JavaScript que maneja la ventana (cpp/prueba.h).
    let guion = std::env::var("VORTEX_LOCAL_AI_PRUEBA").unwrap_or_default();
    let codigo = arranque::ffi::vortex_local_ai_correr(&qml.join("Main.qml").to_string_lossy(), &guion);
    std::process::exit(codigo);
}
