//! `vortex-core`: lo que comparten todas las apps de Vortexine. No depende de Qt: el puente con QML
//! está en `vortex-core-qt`.
//!
//! - [`rutas`]: dónde va cada cosa (XDG en Linux, AppData en Windows).
//! - [`config`]: JSON con valores de fábrica que nunca truena y no pierde claves que no conoce.
//! - [`paleta`]: los colores del fondo de pantalla (matugen), los mismos que `colores.py`.
//! - [`registro`]: un archivo de registro por app, que se recorta solo.

pub mod config;
pub mod paleta;
pub mod registro;
pub mod rutas;

/// Una carpeta temporal nueva para las pruebas (nunca toca HOME ni las carpetas de la persona).
#[doc(hidden)]
pub fn carpeta_de_prueba(nombre: &str) -> std::path::PathBuf {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static N: AtomicUsize = AtomicUsize::new(0);
    let n = N.fetch_add(1, Ordering::SeqCst);
    let c = std::env::temp_dir().join(format!("vortex-core-pruebas-{}-{nombre}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&c);
    std::fs::create_dir_all(&c).expect("no pude crear la carpeta temporal de la prueba");
    c
}
