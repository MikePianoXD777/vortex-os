//! `vortex-usb-rs`: el montador de memorias USB (mismo comportamiento que `bin/vortex-usb`).
fn main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    std::process::ExitCode::from(vortex_usb::principal(&args))
}
