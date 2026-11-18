//! `vortex-tareas-rs`: las tareas de la barra (mismas órdenes y salida que `bin/vortex-tareas`).
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    std::process::ExitCode::from(vortex_tareas::principal(&args))
}
