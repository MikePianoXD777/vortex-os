//! `vortex-runner-motor`: el motor sin ventana. `--correr ID` abre un programa de la biblioteca (lo que
//! usan los accesos del lanzador) y `--estado` dice qué ve el motor (para revisarlo sin abrir la app).

use vortex_runner::{Contexto, NOMBRE, VERSION, estado};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ctx = Contexto::del_entorno();
    if args.iter().any(|a| a == "--version") {
        println!("{NOMBRE} {VERSION} (Rust)");
    } else if let Some(i) = args.iter().position(|a| a == "--correr") {
        std::process::exit(
            args.get(i + 1)
                .map_or(2, |pid| estado::correr_programa(&ctx, pid)),
        );
    } else {
        println!("{}", estado::resumen(&ctx));
    }
}
