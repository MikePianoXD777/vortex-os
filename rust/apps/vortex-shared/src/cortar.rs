//! Ctrl+C sin crates: una bandera que se prende con SIGINT/SIGTERM para que `enviar` y `codigo` limpien lo
//! que dejaron (la carpeta temporal y el sendme) en vez de morir a medias.

#![allow(unsafe_code)] // un manejador de señales: no se puede sin unsafe

use std::sync::atomic::{AtomicBool, Ordering};

static CORTADO: AtomicBool = AtomicBool::new(false);

extern "C" fn al_cortar(_: libc::c_int) {
    CORTADO.store(true, Ordering::SeqCst);
}

/// Desde aquí, Ctrl+C solo prende la bandera.
pub fn instalar() {
    // SAFETY: el manejador solo toca un AtomicBool, que es seguro dentro de una señal.
    unsafe {
        libc::signal(libc::SIGINT, al_cortar as *const () as libc::sighandler_t);
        libc::signal(libc::SIGTERM, al_cortar as *const () as libc::sighandler_t);
    }
}

pub fn cortado() -> bool {
    CORTADO.load(Ordering::SeqCst)
}
