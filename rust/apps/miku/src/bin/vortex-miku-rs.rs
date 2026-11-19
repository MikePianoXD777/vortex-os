//! `vortex-miku-rs`: el comando `vortex-miku` en Rust (avatar, estado, modelo, voz, ia, cámara, compañera,
//! habla-sola, grabar, sentir…). El lanzador `/usr/bin/vortex-miku` salta a este binario si existe; con
//! `VORTEX_MIKU_PYTHON=1` se fuerza el de Python. Solo `natural` y `elevenlabs` se le pasan a Python.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(vortex_miku::cli::principal(&args));
}
