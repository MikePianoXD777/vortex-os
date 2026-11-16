//! Rutas específicas de Vortex Player apoyadas en `vortex_core::rutas`.
//!
//! Mantiene compatibilidad exacta con `rutas.py` de la app oficial (`vortex-player/` en XDG).

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn subcarpeta_player(base_core: PathBuf) -> PathBuf {
    base_core
        .parent()
        .map(|p| p.join("vortex-player"))
        .unwrap_or_else(|| base_core.join("vortex-player"))
}

/// Carpeta de datos de Vortex Player (`biblioteca.db`).
pub fn datos() -> PathBuf {
    if let Some(val) = env::var_os("VORTEX_PLAYER_DATOS").filter(|v| !v.is_empty()) {
        return PathBuf::from(val);
    }
    subcarpeta_player(vortex_core::rutas::datos())
}

/// Carpeta de configuración de Vortex Player (`ajustes.json`).
pub fn config() -> PathBuf {
    if let Some(val) = env::var_os("VORTEX_PLAYER_CONFIG").filter(|v| !v.is_empty()) {
        return PathBuf::from(val);
    }
    subcarpeta_player(vortex_core::rutas::config())
}

/// Carpeta de caché de Vortex Player (portadas y letras).
pub fn cache() -> PathBuf {
    if let Some(val) = env::var_os("VORTEX_PLAYER_CACHE").filter(|v| !v.is_empty()) {
        return PathBuf::from(val);
    }
    subcarpeta_player(vortex_core::rutas::cache())
}

/// Carpeta Música del usuario (`VORTEX_PLAYER_MUSICA`, `xdg-user-dir MUSIC` o `$HOME/Música`).
pub fn musica() -> PathBuf {
    if let Some(val) = env::var_os("VORTEX_PLAYER_MUSICA").filter(|v| !v.is_empty()) {
        return PathBuf::from(val);
    }
    let home = vortex_core::rutas::home();
    if cfg!(windows) {
        return home.join("Music");
    }
    if let Ok(out) = Command::new("xdg-user-dir").arg("MUSIC").output()
        && out.status.success()
    {
        let ruta = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !ruta.is_empty() && ruta != home.to_string_lossy() {
            return PathBuf::from(ruta);
        }
    }
    home.join("Música")
}

/// Carpeta donde quedan las canciones descargadas: `Música/Vortex Player`.
pub fn descargas() -> PathBuf {
    musica().join("Vortex Player")
}

/// Salida de audio de `mpv` (`wasapi` en Windows; `pipewire,pulse,alsa` en Linux).
pub fn salida_audio() -> String {
    env::var("VORTEX_PLAYER_AO")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "wasapi".to_string()
            } else {
                "pipewire,pulse,alsa".to_string()
            }
        })
}

/// Devuelve `(ipc_server_para_mpv, ruta_o_nombre_cliente)` para conectar por IPC.
pub fn canal_mpv(nombre: &str, carpeta: &Path) -> (String, String) {
    if cfg!(windows) {
        (format!(r"\\.\pipe\{nombre}"), nombre.to_string())
    } else {
        let ruta = carpeta.join(format!("{nombre}.sock"));
        let s = ruta.to_string_lossy().to_string();
        (s.clone(), s)
    }
}
