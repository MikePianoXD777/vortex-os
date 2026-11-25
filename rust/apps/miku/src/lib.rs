//! Miku en Rust: el motor de `apps/miku` (Python) sin Qt. Misma persona, mismos archivos de configuración y
//! de memoria (`~/.config/vortex/miku.json`, `Documentos/Miku`), mismas acciones con permiso. La ventana
//! (cxx-qt, QML y el avatar web) vive en `qt/`.

pub mod acciones;
pub mod asistente;
pub mod base64;
pub mod camara;
pub mod cerebro;
pub mod cerebro_abierto;
pub mod cli;
pub mod companera;
pub mod config;
pub mod corazon;
pub mod descargas;
pub mod eventos;
pub mod fecha;
pub mod grabacion;
pub mod http;
pub mod ia;
pub mod interfaz;
pub mod mcp;
pub mod momentos;
pub mod oido;
pub mod permiso_web;
pub mod rutas;
pub mod sha256;
pub mod sistema;
pub mod voz;

pub const VERSION: &str = "4.0.0";
pub const NOMBRE: &str = "Miku";
