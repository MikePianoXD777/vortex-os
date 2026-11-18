//! `vortex-shared-motor`: Shared sin ventana (`escuchar`, `enviar`, `codigo`, `recibir`, `abrir-puerto`,
//! `accion`, `--version`). Es lo que arranca el socket de systemd: no carga Qt.

use vortex_shared::{Config, cli};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::ejecutar(&args, &Config::del_entorno()) {
        Some(codigo) => std::process::exit(codigo),
        None => {
            eprintln!(
                "Uso: vortex-shared-motor escuchar | enviar | codigo | recibir | abrir-puerto | accion | --version"
            );
            eprintln!("La ventana es vortex-shared-rs.");
            std::process::exit(2);
        }
    }
}
