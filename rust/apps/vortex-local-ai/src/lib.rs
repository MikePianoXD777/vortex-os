//! El motor de Vortexine's Local AI en Rust, **sin Qt**: cliente de Ollama por HTTP (con streaming), el
//! catálogo de modelos, el vault de notas como Obsidian, la memoria («Preguntar a mi vault», SQLite) y los
//! chats guardados. Es lo mismo que el motor de la app de Python (`apps/vortex-local-ai/vortex_local_ai`) y lee
//! y escribe **los mismos archivos y formatos** (config, chats, índice de la memoria), así las dos
//! se pueden intercambiar.
//!
//! La interfaz (cxx-qt y los mismos QML) vive en `qt/` y solo llama a este motor.
//!
//! Todo lo que habla con Ollama o lee el vault entero bloquea: córrelo en un hilo aparte.

pub mod catalogo;
pub mod config;
pub mod conversaciones;
pub mod http;
pub mod instancia;
pub mod memoria;
pub mod ollama;
pub mod rutas;
pub mod sha1;
pub mod texto;
pub mod tiempo;
pub mod vault;

/// Nombre de la app, como en la de Python.
pub const NOMBRE: &str = "Vortexine's Local AI";
/// El id de la app (clase de ventana, nombre del lanzador y del socket de la ventana única).
pub const APP_ID: &str = "vortex-local-ai";
/// La versión es la de la app de Python (1.0.0): la de Rust la iguala.
pub const VERSION: &str = "1.0.0";
