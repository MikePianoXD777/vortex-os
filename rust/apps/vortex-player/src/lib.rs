//! El motor de Vortex Player en Rust, **sin Qt** (biblioteca, cola, IPC de mpv, calidad, descargas, letras, nube).
//!
//! Aquí va todo lo que se puede probar con `cargo test` sin pantalla. La interfaz (cxx-qt + los QML de
//! la app de Python) se queda delgada y solo llama a este motor. Qué hay que igualar: ver README.md.

pub mod audio;
pub mod biblioteca;
pub mod calidad;
pub mod cola;
pub mod descargas;
pub mod discord;
pub mod ecualizador;
pub mod letras;
pub mod mpv;
pub mod nube;
pub mod rutas;

/// Nombre de la app, como en la de Python.
pub const NOMBRE: &str = "Vortex Player";
/// Versión del motor en Rust (no la de la app oficial de Python).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod pruebas {
    #[test]
    fn tiene_nombre() {
        assert!(!super::NOMBRE.is_empty());
    }
}
