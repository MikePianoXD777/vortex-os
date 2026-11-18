//! `vortex-app-installer-rs`: Vortexine's App Installer en Rust. Se instala aparte de `vortex-app-installer`
//! (el de Python) mientras se prueba.
//!
//!     vortex-app-installer-rs [ARCHIVO]     abre la ventana (con el archivo, si se da)
//!     vortex-app-installer-rs --version
//!     vortex-app-installer-rs --capturas CARPETA ARCHIVO[::ACCION]… [--accion ID] [--contrasena X]
//!                             [--ancho 760] [--alto 720]
//!         saca capturas de la ventana como `pruebas/capturas.py` (úsalo con QT_QPA_PLATFORM=offscreen)
//!
//! Una sola ventana por persona: si ya hay una abierta (de Rust o de Python), esta le pasa el archivo y sale.

mod arranque;
mod colores;
mod puente;

use std::path::PathBuf;
use vortex_app_installer::instancia::{self, Mensaje};
use vortex_app_installer::{Contexto, NOMBRE, VERSION, sesion};

/// Los QML: VORTEX_APP_INSTALLER_QML, los que trae el repo (al compilar desde aquí) o
/// /usr/share/vortex-app-installer-rs/qml.
fn carpeta_qml() -> Option<PathBuf> {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/qml"));
    std::env::var_os("VORTEX_APP_INSTALLER_QML")
        .map(PathBuf::from)
        .into_iter()
        .chain([repo, PathBuf::from("/usr/share/vortex-app-installer-rs/qml")])
        .find(|q| q.join("Main.qml").is_file())
}

/// El primer argumento que no es una opción (lo que pasa el explorador al dar doble clic: %f o %u).
fn archivo_de(args: &[String], ctx: &Contexto) -> String {
    args.iter()
        .find(|a| !a.starts_with('-') || a.starts_with("file:"))
        .map(|a| sesion::ruta_de(ctx, a))
        .unwrap_or_default()
}

fn valor_de<'a>(args: &'a [String], opcion: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == opcion)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn main() {
    cxx_qt::init_crate!(vortex_app_installer_qt);
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!("{NOMBRE} {VERSION} (Rust)");
        return;
    }
    let Some(qml) = carpeta_qml() else {
        eprintln!("Vortexine's App Installer no está instalado del todo: no encuentro su interfaz.");
        std::process::exit(1);
    };
    let ctx = Contexto::del_entorno();
    let qml = qml.join("Main.qml").to_string_lossy().into_owned();
    let prueba = std::env::var_os("VORTEX_APP_INSTALLER_PRUEBA").is_some();

    if let Some(carpeta) = valor_de(&args, "--capturas") {
        // «--capturas CARPETA ARCHIVO[::ACCION]…»: lo que viene después de la carpeta son los archivos.
        let accion = valor_de(&args, "--accion").unwrap_or("");
        let mut saltar = false;
        let mut plan = String::new();
        let mut tras_carpeta = false;
        for a in &args {
            if saltar {
                saltar = false;
                continue;
            }
            match a.as_str() {
                "--capturas" => {
                    tras_carpeta = true;
                    saltar = true;
                    continue;
                }
                "--accion" | "--contrasena" | "--ancho" | "--alto" => {
                    saltar = true;
                    continue;
                }
                o if o.starts_with("--") => continue,
                _ => {}
            }
            if !tras_carpeta {
                continue;
            }
            let (ruta, id) = a.split_once("::").unwrap_or((a, accion));
            plan.push_str(&format!("{}\t{id}\n", sesion::ruta_de(&ctx, ruta)));
        }
        let numero =
            |o: &str, defecto: i32| valor_de(&args, o).and_then(|v| v.parse().ok()).unwrap_or(defecto);
        let codigo = arranque::ffi::vortex_app_installer_correr(
            &qml,
            "",
            false,
            carpeta,
            &plan,
            numero("--ancho", 760),
            numero("--alto", 720),
            valor_de(&args, "--contrasena").unwrap_or(""),
        );
        std::process::exit(codigo);
    }

    let archivo = archivo_de(&args, &ctx);
    if !prueba {
        // ¿Ya hay una abierta? Se le pasa el archivo (o solo «mostrar») y listo.
        let mensaje = Mensaje {
            archivo: archivo.clone(),
        };
        if instancia::avisar_a_la_abierta(&instancia::ruta(&instancia::nombre()), &mensaje) {
            return;
        }
    }
    let _ = vortex_core::registro::iniciar("vortex-app-installer-rs");
    let codigo = arranque::ffi::vortex_app_installer_correr(&qml, &archivo, prueba, "", "", 0, 0, "");
    // Cerrada la ventana se sale ya, sin esperar a los hilos de fondo (una búsqueda en Flathub sin red).
    std::process::exit(codigo);
}
