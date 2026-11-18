//! Vortexine's Shop: la tienda de aplicaciones de Vortexine's OS, el motor en Rust.
//!
//! Las apps salen de Flathub (Flatpak) y se instalan para tu usuario, sin contraseña y sin terminal. Este
//! crate es el motor, igual que el paquete `vortex_shop` de Python (sin Qt, con pruebas):
//!
//! - [`catalogo`]: qué hay en Flathub (su API pública, vía `curl`, con caché en disco);
//! - [`flatpak`]: instalar, quitar y actualizar con la CLI de `flatpak` (con progreso real);
//! - [`propias`]: las apps propias de Vortexine's (Camera, Runner), que instala el comando `vortex`;
//! - [`android`]: los juegos de Android (Waydroid), que maneja `vortex-android`;
//! - [`tienda`]: el estado que ve la interfaz (lo que en Python es `puente.py`), con su cola y sus hilos.
//!
//! La ventana (cxx-qt, con los mismos QML de Python) está en el crate `vortex-shop-qt`.

pub mod android;
pub mod catalogo;
pub mod errores;
pub mod flatpak;
pub mod ids;
pub mod motor;
pub mod proceso;
pub mod propias;
pub mod texto;
pub mod tienda;

pub const VERSION: &str = "1.1.1";
pub const NOMBRE: &str = "Vortexine's Shop";
/// Lo que la tienda dice ser en la red (cabecera User-Agent de las peticiones a Flathub).
pub const AGENTE: &str = "vortexines-shop/1.1.1";

/// Un URL `file://` de una ruta (con los caracteres raros en %XX), como `Path.as_uri()` de Python.
pub fn url_de_archivo(ruta: &std::path::Path) -> String {
    let mut s = String::from("file://");
    for b in ruta.to_string_lossy().bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~".contains(&b) {
            s.push(b as char);
        } else {
            s.push_str(&format!("%{b:02X}"));
        }
    }
    s
}
