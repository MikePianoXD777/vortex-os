//! Vortexine Connect sin Qt: se conecta a redes abiertas cuando viajas y cifra lo que mandas con Cloudflare
//! WARP. Es lo mismo que el paquete `vortex_connect` de Python (`motor.py`, `redes.py`, `proteccion.py`,
//! `viaje.py`), sin la ventana: esa vive en el crate `qt/`.
//!
//! Nada aquí toca la red, el firewall ni WARP por su cuenta en las pruebas: todo lo que habla con el sistema
//! pasa por [`sistema::Sistema`] (programas que se buscan en un `PATH` que se puede cambiar) y las pruebas
//! ponen programas falsos.

pub mod cli;
pub mod http;
pub mod instancia;
pub mod motor;
pub mod proteccion;
pub mod redes;
pub mod sistema;
pub mod viaje;

pub const APP_ID: &str = "vortex-connect";
pub const NOMBRE: &str = "Vortexine Connect";
pub const VERSION: &str = "1.0.0";
pub const ES_WINDOWS: bool = cfg!(windows);

/// Donde van los ajustes (`connect.json`) y el candado: la carpeta de configuración de Vortexine.
pub fn carpeta_config() -> std::path::PathBuf {
    vortex_core::rutas::config()
}
