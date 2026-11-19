//! Miku en Rust: el motor de `apps/miku` (Python) sin Qt. Misma persona, mismos archivos de configuración y
//! de memoria (`~/.config/vortex/miku.json`, `Documentos/Miku`), mismas acciones con permiso. La ventana
//! (cxx-qt, QML y el avatar web) vive en `qt/`.

pub mod config;
pub mod corazon;
pub mod fecha;
pub mod rutas;
pub mod sistema;

pub const VERSION: &str = "4.0.0";
pub const NOMBRE: &str = "Miku";
