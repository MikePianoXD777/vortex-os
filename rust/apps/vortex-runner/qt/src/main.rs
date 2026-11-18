//! `vortex-runner-rs`: Vortexine's Runner en Rust. Se instala aparte de `vortex-runner` mientras se prueba.
//!
//!     vortex-runner-rs                 abre la biblioteca
//!     vortex-runner-rs ARCHIVO.exe     lo abre si ya está en tu biblioteca; si no, pregunta
//!     vortex-runner-rs --correr ID     abre ese programa sin ventana (los accesos del lanzador)
//!     vortex-runner-rs --version
//!
//! Una sola ventana por persona: si ya hay una abierta (de Rust o de Python), esta le pasa el archivo (o le
//! pide que se muestre) y sale.

mod arranque;
mod colores;
mod puente;

use std::path::{Path, PathBuf};
use vortex_runner::instancia::{self, Mensaje};
use vortex_runner::{Contexto, NOMBRE, VERSION, estado};

/// Los QML: VORTEX_RUNNER_QML, los que trae el repo (al compilar desde aquí) o /usr/share/vortex-runner-rs.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml"));
    std::env::var_os("VORTEX_RUNNER_QML")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-runner-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_runner_qt);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("{NOMBRE} {VERSION} (Rust)");
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--correr") {
        let codigo = args
            .get(i + 1)
            .map_or(2, |pid| estado::correr_programa(&Contexto::del_entorno(), pid));
        std::process::exit(codigo);
    }
    let archivo = args.iter().find(|a| !a.starts_with('-')).map(|a| {
        vortex_runner::rutas::resolver(Path::new(a))
            .to_string_lossy()
            .into_owned()
    });
    // ¿Ya hay una abierta? Se le pasa el archivo (o «mostrar») y listo.
    let mensaje = archivo.clone().map_or(Mensaje::Mostrar, Mensaje::Abrir);
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre()), &mensaje) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine's Runner no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-runner-rs");
    let prueba = std::env::var_os("VORTEX_RUNNER_PRUEBA").is_some();
    let codigo = arranque::ffi::vortex_runner_correr(
        &qml.join("Main.qml").to_string_lossy(),
        archivo.as_deref().unwrap_or(""),
        prueba,
    );
    std::process::exit(codigo);
}
