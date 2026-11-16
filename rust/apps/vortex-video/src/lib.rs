//! El motor de Vortexine Video en Rust, **sin Qt**: todo lo de la app de Python que no es la ventana.
//!
//! - [`ajustes`]: `~/.config/vortex/video.json` (el mismo archivo que la de Python).
//! - [`formatos`]: elegir la resolución y los cuadros por segundo de la cámara.
//! - [`capturas`]: dónde van las fotos y los videos, sus nombres y la última captura.
//! - [`miniatura`]: el cuadrito del último video (ffmpeg).
//! - [`instancia`]: una sola ventana por persona (el mismo socket que la de Python).
//! - [`estado`]: lo que hoy hace `puente.py` sin Qt; la interfaz (cxx-qt) solo lo envuelve.
//!
//! La cámara en sí (verla, tomar la foto, grabar) es de Qt Multimedia en el QML, igual que hoy: este motor
//! nunca abre la cámara ni el micrófono.

pub mod ajustes;
pub mod capturas;
pub mod estado;
pub mod formatos;
pub mod hora;
pub mod instancia;
pub mod miniatura;

pub const NOMBRE: &str = "Vortexine Video";
pub const APP_ID: &str = "vortex-video";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
