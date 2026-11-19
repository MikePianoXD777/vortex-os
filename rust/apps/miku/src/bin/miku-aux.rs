//! `miku-aux`: lo que Miku lanza como procesos aparte.
//!
//!   miku-aux mcp         el servidor MCP de las acciones (Claude y las demás IA lo arrancan solo)
//!   miku-aux hook-web    el hook de Claude Code que pide permiso antes de cada búsqueda en internet
//!   miku-aux momentos V  arma los mejores momentos de una grabación como proyecto de Vortex Studio

use vortex_miku::{acciones, momentos, permiso_web};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("mcp") => acciones::servir(),
        Some("hook-web") => permiso_web::principal(),
        Some("momentos") => std::process::exit(momentos::principal(&args[1..])),
        _ => {
            eprintln!("uso: miku-aux mcp | hook-web | momentos VIDEO");
            std::process::exit(2);
        }
    }
}
