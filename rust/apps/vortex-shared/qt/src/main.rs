//! `vortex-shared-rs`: Vortexine Shared en Rust. Se instala aparte de `vortex-shared` mientras se prueba.
//!
//!     vortex-shared-rs                     la ventana
//!     vortex-shared-rs ventana [ARCHIVOS]  la ventana, con esos archivos ya puestos (clic derecho de Thunar)
//!     vortex-shared-rs enviar | codigo | recibir | abrir-puerto | accion | escuchar | --version
//!     vortex-shared-rs --prueba-humo       carga la ventana y se cierra (para un paquete recién armado)
//!
//! Qt solo se carga para la ventana: las órdenes de arriba las atiende `vortex_shared::cli` antes (escuchar un
//! aviso no necesita 100 MB). Una sola ventana por persona: si ya hay una abierta (de Rust o de Python), esta le
//! pasa los archivos y sale.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_shared::{Config, cli, instancia};

/// Los QML: VORTEX_SHARED_QML, los que trae el repo (al compilar desde aquí) o /usr/share/vortex-shared-rs.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml"));
    std::env::var_os("VORTEX_SHARED_QML")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-shared-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

/// `Path(a).expanduser().resolve()`
fn absoluta(a: &str, home: &std::path::Path) -> String {
    let p = match a.strip_prefix("~/") {
        Some(r) => home.join(r),
        None if a == "~" => home.to_path_buf(),
        None => PathBuf::from(a),
    };
    std::fs::canonicalize(&p)
        .or_else(|_| std::path::absolute(&p))
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cfg = Config::del_entorno();
    if let Some(codigo) = cli::ejecutar(&args, &cfg) {
        std::process::exit(codigo);
    }
    cxx_qt::init_crate!(vortex_shared_qt);
    let humo = args.iter().any(|a| a == "--prueba-humo");
    let archivos: Vec<String> = args
        .iter()
        .skip(usize::from(args.first().is_some_and(|a| a == "ventana")))
        .filter(|a| !a.starts_with("--"))
        .map(|a| absoluta(a, &cfg.home))
        .collect();
    let ruta = instancia::ruta(&instancia::nombre());
    if !humo && instancia::avisar_a_la_abierta(&ruta, &archivos) {
        return; // ya hay una ventana: se le pasaron los archivos
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine Shared no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let _ = vortex_core::registro::iniciar("vortex-shared-rs");
    puente::poner_archivos_iniciales(archivos);
    let recorrer = std::env::var_os("VORTEX_SHARED_RECORRER").is_some();
    let codigo = arranque::ffi::vortex_shared_correr(&qml.join("Main.qml").to_string_lossy(), recorrer, humo);
    puente::cerrar();
    if !humo && !recorrer {
        let _ = std::fs::remove_file(&ruta);
    }
    std::process::exit(codigo);
}
