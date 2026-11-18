//! `vortex-tirones`: el detector de tirones de los juegos (ver lib.rs).
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    std::process::ExitCode::from(vortex_tirones::principal(&args))
}
