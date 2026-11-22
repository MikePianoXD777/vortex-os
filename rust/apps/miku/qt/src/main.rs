//! `miku-rs`: Miku en Rust. El lanzador de siempre (`vortex-miku`) salta a este binario si existe
//! (`VORTEX_MIKU_PYTHON=1` fuerza la de Python). Una sola Miku por usuario: si otra ya tiene el candado, esta
//! sale sin hacer nada.
//!
//! Los QML van en `qml/` (se instalan en `/usr/share/vortex-miku-rs/qml`); la página del avatar y la persona
//! son las de siempre (`web/`, `persona/`).

mod arranque;
mod puente;

use std::path::PathBuf;
use vortex_miku::config::Config;
use vortex_miku::{interfaz, rutas::Rutas, sistema};

/// Donde están los QML: VORTEX_MIKU_QML, el repo (al compilar desde aquí) y donde los deja el paquete.
fn carpeta_qml() -> Option<PathBuf> {
    std::env::var_os("VORTEX_MIKU_QML")
        .filter(|d| !d.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([
            PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml")),
            PathBuf::from("/usr/share/vortex-miku-rs/qml"),
        ])
        .find(|q| q.join("Main.qml").is_file())
}

fn main() {
    cxx_qt::init_crate!(vortex_miku_qt);
    if std::env::args().any(|a| a == "--version") {
        println!("Miku {} (Rust)", interfaz::VERSION);
        return;
    }
    let prueba = std::env::var_os("VORTEX_MIKU_PRUEBA").is_some_and(|v| !v.is_empty());
    let rutas = Rutas::real();
    // Una sola Miku por usuario (las pruebas no la toman: no le quitan el lugar a la de la persona).
    let _candado = if prueba {
        None
    } else {
        sistema::candado(&vortex_miku::rutas::runtime().join("vortex-miku.vivo"))
    };
    if !prueba && _candado.is_none() {
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Miku no está instalada del todo: faltan sus QML. Reinstala el paquete vortex-miku.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("miku-rs");
    let cfg = Config::cargar(&rutas.config, &rutas);
    // El chat mini (doble clic en la compañera de escritorio) es siempre el panel chico, aunque esté el 3D.
    let chat = std::env::var("VORTEX_MIKU_CHAT").is_ok_and(|v| v == "1");
    let (avatar, nota) = interfaz::elegir_modo(cfg.avatar && !chat, &rutas.vendor);
    let _ = puente::MODO.set(puente::Modo { avatar, nota, cfg });
    let codigo = arranque::ffi::vortex_miku_correr(&qml.join("Main.qml").to_string_lossy(), avatar, prueba);
    // Si el comando dejó su PID, solo se quita cuando es el nuestro.
    let pid = vortex_miku::rutas::runtime().join("vortex-miku.pid");
    if std::fs::read_to_string(&pid).is_ok_and(|t| t.trim() == std::process::id().to_string()) {
        let _ = std::fs::remove_file(pid);
    }
    std::process::exit(codigo);
}
