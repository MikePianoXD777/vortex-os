//! `vortex-app-installer-motor ARCHIVO [--idioma es|en]`: lo que haría la ventana al abrir el archivo, como
//! JSON (el motor sin interfaz, para probarlo desde la terminal). `--instalados` lista lo ya instalado.

use vortex_app_installer::{Contexto, motor};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version") {
        println!(
            "{} {}",
            vortex_app_installer::NOMBRE,
            vortex_app_installer::VERSION
        );
        return;
    }
    let ctx = Contexto::del_entorno();
    let salida = if args.iter().any(|a| a == "--instalados") {
        serde_json::Value::Array(motor::instalados(&ctx))
    } else if let Some(ruta) = args.iter().find(|a| !a.starts_with('-')) {
        let info = motor::analizar(&ctx, std::path::Path::new(ruta), None);
        motor::para_mostrar(&ctx, &info)
    } else {
        eprintln!("uso: vortex-app-installer-motor ARCHIVO | --instalados | --version");
        std::process::exit(2);
    };
    println!("{}", serde_json::to_string_pretty(&salida).unwrap_or_default());
}
