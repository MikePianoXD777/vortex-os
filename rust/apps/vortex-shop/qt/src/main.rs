//! `vortex-shop-rs`: Vortexine's Shop en Rust. Se instala aparte de `vortex-shop` mientras se prueba.
//!
//!     vortex-shop-rs            abre la tienda
//!     vortex-shop-rs --version
//!
//! Una sola ventana por persona: si ya hay una abierta (de Rust o de Python), esta le pide que se muestre
//! y sale.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_shop::instancia;
use vortex_shop::{NOMBRE, VERSION};

/// Los QML: VORTEX_SHOP_QML, los que trae el repo (al compilar desde aquí) o /usr/share/vortex-shop-rs/qml.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml"));
    std::env::var_os("VORTEX_SHOP_QML")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-shop-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_shop_qt);
    if std::env::args().any(|a| a == "--version") {
        println!("{NOMBRE} {VERSION} (Rust)");
        return;
    }
    // ¿Ya hay una abierta? Se le pide mostrarse y listo.
    if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre())) {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine's Shop no está instalada del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = puente::CARPETA_QML.set(qml.clone());
    let _ = vortex_core::registro::iniciar("vortex-shop-rs");
    let prueba = std::env::var_os("VORTEX_SHOP_PRUEBA").is_some();
    let codigo = arranque::ffi::vortex_shop_correr(&qml.join("Main.qml").to_string_lossy(), prueba);
    std::process::exit(codigo);
}
