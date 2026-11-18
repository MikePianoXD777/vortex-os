//! Vortexine Shared sin Qt: pasa archivos a otra compu con Vortexine's OS, cerca o lejos.
//!
//! Se escribe la IP (o el nombre en Tailscale) y el usuario de la otra compu; a ella le sale un aviso para
//! aceptar. El archivo va cifrado de punta a punta con `sendme` (iroh). Sin Tailscale ni misma red, se crea
//! un código y se pega en la otra.
//!
//! Es lo mismo que el paquete `vortex_shared` de Python (`motor.py`, `servicio.py`, `thunar.py`), sin la
//! ventana: esa vive en el crate `qt/`. `servicio` escucha los avisos (lo arranca un socket de systemd, así
//! que en reposo no ocupa nada): es un binario chico, sin Qt ni intérprete.

pub mod cli;
pub mod cortar;
pub mod instancia;
pub mod motor;
pub mod servicio;
pub mod sistema;
pub mod thunar;

use sistema::Sistema;
use std::path::PathBuf;
use std::time::Duration;

pub const VERSION: &str = "1.0.0";
pub const NOMBRE: &str = "Vortexine Shared";
pub const APP_ID: &str = "vortex-shared";

/// De dónde sale todo lo que cambia entre una compu y una prueba (carpetas, puerto, esperas). El motor y el
/// servicio reciben esto en vez de leer el entorno por su cuenta: así las pruebas no tocan nada de nadie.
#[derive(Debug, Clone)]
pub struct Config {
    /// `VORTEX_SHARED_PUERTO` (1770)
    pub puerto: u16,
    /// `VORTEX_SHARED_SIN_AVANCE` (120 s): el otro se apagó o se cayó la red
    pub sin_avance: Duration,
    /// `VORTEX_SHARED_ESPERA` (120 s): cuánto espera el aviso a que alguien conteste
    pub espera_respuesta: Duration,
    /// `VORTEX_SHARED_INACTIVO` (45 s): sin nadie, el servicio se apaga
    pub inactivo: Duration,
    /// `VORTEX_SHARED_CARPETA`: dónde caen los recibidos (si no, Descargas/Vortexine Shared)
    pub carpeta_fija: Option<PathBuf>,
    /// `~/.cache/vortex-shared`
    pub cache: PathBuf,
    /// la marca de «vortex modo-juego»
    pub marca_juego: PathBuf,
    /// `$XDG_RUNTIME_DIR`
    pub run: PathBuf,
    /// `~/.config` (donde cuelgan `vortex/` y `Thunar/`)
    pub config: PathBuf,
    /// el usuario de esta sesión: solo a él se le puede mandar
    pub usuario: String,
    pub home: PathBuf,
    pub sistema: Sistema,
}

fn variable(nombre: &str) -> Option<String> {
    std::env::var(nombre).ok().filter(|v| !v.is_empty())
}

fn absoluta(nombre: &str) -> Option<PathBuf> {
    variable(nombre).map(PathBuf::from).filter(|p| p.is_absolute())
}

fn numero(nombre: &str, defecto: f64) -> Duration {
    Duration::from_secs_f64(
        variable(nombre)
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|n| *n >= 0.0 && n.is_finite())
            .unwrap_or(defecto),
    )
}

/// El uid de este proceso (el dueño de `/proc/self`).
pub fn uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").map_or(0, |m| m.uid())
}

impl Config {
    pub fn del_entorno() -> Self {
        let sistema = Sistema::real();
        let home = vortex_core::rutas::home();
        let usuario = ["LOGNAME", "USER", "LNAME", "USERNAME"]
            .iter()
            .find_map(|v| variable(v))
            .or_else(|| {
                let s = sistema.correr(&["id", "-un"], Duration::from_secs(5));
                s.bien().then(|| s.salida.trim().to_string())
            })
            .unwrap_or_else(|| uid().to_string());
        Config {
            puerto: variable("VORTEX_SHARED_PUERTO")
                .and_then(|p| p.parse().ok())
                .unwrap_or(1770),
            sin_avance: numero("VORTEX_SHARED_SIN_AVANCE", 120.0),
            espera_respuesta: numero("VORTEX_SHARED_ESPERA", 120.0),
            inactivo: numero("VORTEX_SHARED_INACTIVO", 45.0),
            carpeta_fija: variable("VORTEX_SHARED_CARPETA").map(PathBuf::from),
            cache: absoluta("XDG_CACHE_HOME")
                .unwrap_or_else(|| home.join(".cache"))
                .join("vortex-shared"),
            marca_juego: vortex_core::rutas::estado().join("modo-juego"),
            run: absoluta("XDG_RUNTIME_DIR").unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", uid()))),
            config: absoluta("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")),
            usuario,
            home,
            sistema,
        }
    }
}
