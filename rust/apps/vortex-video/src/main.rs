//! `vortex-video-motor`: el motor sin ventana, para revisarlo y medirlo. La app con ventana es
//! `vortex-video-rs` (carpeta `qt/`, con cxx-qt). Muestra qué carpetas, ajustes y última captura vería la app.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version") {
        println!("{} {} (Rust)", vortex_video::NOMBRE, vortex_video::VERSION);
        return;
    }
    let e = vortex_video::estado::Estado::new();
    println!("Fotos:  {}", e.carpetas.fotos.display());
    println!("Videos: {}", e.carpetas.videos.display());
    println!("Ajustes: {:?}", e.ajustes);
    println!(
        "Última: {}",
        e.ultima()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(ninguna)".into())
    );
    if args.iter().any(|a| a == "--medir") {
        // El pico de memoria del proceso (VmHWM), para comparar con la de Python.
        let estado = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
        if let Some(l) = estado.lines().find(|l| l.starts_with("VmHWM:")) {
            println!("{l}");
        }
    }
}
