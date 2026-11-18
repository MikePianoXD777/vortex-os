//! `vortex-local-ai-rs`: todavía no abre ventana. Cuando el motor esté listo, aquí se arranca la interfaz con
//! cxx-qt (ver crates/vortex-core-qt). Mientras, la app oficial es la de Python (`vortex-local-ai`).

use vortex_core::{paleta, registro};

fn main() {
    let _ = registro::iniciar("vortex-local-ai-rs");
    registro::info("arrancó (esqueleto)");
    let p = paleta::actual();
    println!(
        "{} (Rust) {}: en construcción. La app oficial sigue siendo la de Python.",
        vortex_local_ai::NOMBRE,
        vortex_local_ai::VERSION
    );
    println!(
        "Paleta: fondo {} · texto {} · azul {}",
        p["fondo"], p["texto"], p["azul"]
    );
}
