//! `vortex-connect-rs`: Vortexine Connect en Rust. Se instala aparte de `vortex-connect` mientras se prueba.
//!
//!     vortex-connect-rs                 la ventana
//!     vortex-connect-rs estado          dice en qué red estás y si va cifrado
//!     vortex-connect-rs redes           las redes abiertas cerca
//!     vortex-connect-rs vigilar         el modo viaje (lo arranca su servicio)
//!     vortex-connect-rs --prueba-humo   carga la ventana y se cierra (para un paquete recién armado)
//!     vortex-connect-rs --version
//!
//! Qt solo se carga para la ventana. Una sola ventana por persona: si ya hay una abierta (de Rust o de
//! Python), esta le pide que se muestre y sale.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_connect::sistema::Sistema;
use vortex_connect::{cli, instancia};

/// Los QML: VORTEX_CONNECT_QML, los que trae el repo (al compilar desde aquí) o /usr/share/vortex-connect-rs.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml"));
    std::env::var_os("VORTEX_CONNECT_QML")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-connect-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(codigo) = cli::ejecutar(&args, &Sistema::real()) {
        std::process::exit(codigo);
    }
    cxx_qt::init_crate!(vortex_connect_qt);
    let humo = args.iter().any(|a| a == "--prueba-humo");
    let ruta = instancia::ruta(&instancia::nombre());
    if !humo && instancia::avisar_a_la_abierta(&ruta) {
        return; // ya hay una ventana: se le pidió que se muestre
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine Connect no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-connect-rs");
    let recorrer = std::env::var_os("VORTEX_CONNECT_RECORRER").is_some();
    let codigo =
        arranque::ffi::vortex_connect_correr(&qml.join("Main.qml").to_string_lossy(), recorrer, humo);
    if !humo && !recorrer {
        let _ = std::fs::remove_file(&ruta);
    }
    std::process::exit(codigo);
}
