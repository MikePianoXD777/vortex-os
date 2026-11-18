//! `vortex-task-manager-rs`: Vortexine Task Manager en Rust. Se instala aparte de `vortex-task-manager`
//! mientras se prueba. Usa los mismos QML que la de Python (copiados en `../qml`).

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_task_manager::instancia;

/// Donde están los QML: VORTEX_TASK_MANAGER_DIR, la carpeta del repo (al compilar desde aquí) o
/// /usr/share/vortex-task-manager.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../qml"));
    std::env::var_os("VORTEX_TASK_MANAGER_DIR")
        .map(|d| PathBuf::from(d).join("qml"))
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-task-manager/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_task_manager_qt);
    if std::env::args().any(|a| a == "--version") {
        println!(
            "{} {} (Rust)",
            vortex_task_manager::NOMBRE,
            vortex_task_manager::VERSION
        );
        return;
    }
    // Una sola ventana por persona: si ya hay una abierta (de Rust o de Python), que se muestre esa.
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre())) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine Task Manager no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-task-manager-rs");
    let prueba = std::env::var_os("VORTEX_TASK_MANAGER_PRUEBA").is_some();
    let codigo = arranque::ffi::vortex_tm_correr(&qml.join("Main.qml").to_string_lossy(), prueba);
    std::process::exit(codigo);
}
