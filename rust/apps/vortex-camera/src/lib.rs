//! Vortexine's Camera en Rust: tus cámaras de seguridad (Imou/Dahua) en mosaico, directo por RTSP, sin la
//! nube de Imou. Es el motor de `apps/vortex-camera` (Python) portado tal cual: mismo archivo de
//! configuración (`~/.config/vortex-camera/camaras.json`), mismas URL, misma búsqueda en la red y mismas
//! respuestas para la interfaz. La ventana (cxx-qt, los mismos QML) vive en `qt/`.

pub mod camaras;
pub mod cripto;
pub mod descubrir;
pub mod fecha;
pub mod grabar;
pub mod http;
pub mod instancia;
pub mod ptz;

pub const VERSION: &str = "1.0.0";
pub const NOMBRE: &str = "Vortexine's Camera";
pub const APP_ID: &str = "vortex-camera";
