//! `vortex-connect-motor`: Connect sin ventana (`estado`, `redes`, `vigilar`, `--version`). Es lo que
//! arranca el servicio del modo viaje: no carga Qt.

use vortex_connect::{cli, sistema::Sistema};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match cli::ejecutar(&args, &Sistema::real()) {
        Some(codigo) => std::process::exit(codigo),
        None => {
            eprintln!("Uso: vortex-connect-motor estado | redes | vigilar | --version");
            eprintln!("La ventana es vortex-connect-rs.");
            std::process::exit(2);
        }
    }
}
