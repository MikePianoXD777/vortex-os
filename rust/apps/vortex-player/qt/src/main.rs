//! `vortex-player-rs`: Vortex Player en Rust. El lanzador de siempre (`vortex-player`) salta a este binario
//! si existe (`VORTEX_PLAYER_PYTHON=1` fuerza la de Python).
//!
//! Usa los mismos QML que la de Python (no se copian al repo): los busca donde los deja ella.

mod arranque;
mod colores;
mod instancia;
mod motor;
mod puente;
mod rep;
mod vp_cuenta;
mod vp_red;
mod vp_sonido;

use std::path::PathBuf;

const NOMBRE: &str = "Vortex Player";
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Donde están los QML: VORTEX_PLAYER_DIR (el lanzador la pone), el repo (al compilar desde aquí) y la copia
/// del paquete vortex-rust.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../../apps/vortex-player"));
    std::env::var_os("VORTEX_PLAYER_DIR")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-player")])
        .map(|c| c.join("vortex_player").join("qml"))
        .chain([PathBuf::from("/usr/share/vortex-player-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_player_qt);
    let todos: Vec<String> = std::env::args().skip(1).collect();
    if todos.iter().any(|a| a == "--version") {
        println!("{NOMBRE} {VERSION} (Rust)");
        return;
    }
    let args: Vec<String> = todos.into_iter().filter(|a| !a.starts_with('-') || a == "--seguir").collect();
    let prueba = std::env::var_os("VORTEX_PLAYER_PRUEBA").is_some_and(|v| !v.is_empty());
    // Una sola ventana: si ya hay una abierta (de Rust o de Python), le pasa lo que recibió y sale.
    if !prueba && instancia::avisar_a_la_abierta(&instancia::ruta(), &args) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortex Player no está instalado del todo: faltan sus pantallas (QML).");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-player-rs");
    let _ = puente::ARGS.set(args);
    let codigo = arranque::ffi::vortex_player_correr(&qml.join("Main.qml").to_string_lossy(), prueba);
    // Como `os._exit` de la de Python: los hilos que esperan red no detienen la salida.
    std::process::exit(codigo);
}
