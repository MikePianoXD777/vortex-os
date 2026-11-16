//! El motor de Vortexine's Local AI en Rust, **sin Qt** (cliente de Ollama por HTTP, catálogo, vault de notas, memoria (búsqueda con SQLite) y chats).
//!
//! Aquí va todo lo que se puede probar con `cargo test` sin pantalla. La interfaz (cxx-qt + los QML de
//! la app de Python) se queda delgada y solo llama a este motor. Qué hay que igualar: ver README.md.

/// Nombre de la app, como en la de Python.
pub const NOMBRE: &str = "Vortexine's Local AI";
/// Versión del motor en Rust (no la de la app oficial de Python).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod pruebas {
    #[test]
    fn tiene_nombre() {
        assert!(!super::NOMBRE.is_empty());
    }
}
