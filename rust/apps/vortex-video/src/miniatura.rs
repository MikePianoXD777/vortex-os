//! El cuadrito del último video: un cuadro con ffmpeg en `~/.cache/vortex-video/miniatura.jpg` (la misma
//! ruta que la de Python). Sin ffmpeg, el cuadrito muestra solo el ícono de video.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const TIEMPO: Duration = Duration::from_secs(20);

/// Dónde queda la miniatura.
pub fn ruta_salida() -> PathBuf {
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| vortex_core::rutas::home().join(".cache"));
    base.join("vortex-video").join("miniatura.jpg")
}

/// El primer `ffmpeg` del PATH.
pub fn ffmpeg() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|d| d.join("ffmpeg"))
        .find(|p| p.is_file())
}

/// Saca un cuadro de `video` a `salida` (160 px de ancho). `true` si salió.
pub fn hacer_con(ffmpeg: &Path, video: &Path, salida: &Path) -> bool {
    if let Some(c) = salida.parent()
        && std::fs::create_dir_all(c).is_err()
    {
        return false;
    }
    let hijo = Command::new(ffmpeg)
        .args(["-y", "-v", "error", "-i"])
        .arg(video)
        .args(["-frames:v", "1", "-vf", "scale=160:-2"])
        .arg(salida)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut hijo) = hijo else { return false };
    let inicio = Instant::now();
    loop {
        match hijo.try_wait() {
            Ok(Some(estado)) => return estado.success() && salida.is_file(),
            Ok(None) if inicio.elapsed() < TIEMPO => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return false;
            }
        }
    }
}

/// La miniatura de `video` con el ffmpeg del sistema, en `ruta_salida()`.
pub fn hacer(video: &Path) -> Option<PathBuf> {
    let salida = ruta_salida();
    hacer_con(&ffmpeg()?, video, &salida).then_some(salida)
}
