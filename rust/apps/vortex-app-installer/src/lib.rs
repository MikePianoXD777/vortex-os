//! El motor de Vortexine's App Installer en Rust, **sin Qt**: todo lo de la app de Python que no es la
//! ventana.
//!
//! - [`textos`]: los textos en español e inglés (`textos.py`).
//! - [`rutas`]: dónde deja las cosas (`rutas.py`).
//! - [`comprimidos`]: qué traen los comprimidos y descomprimirlos sin peligro (`comprimidos.py`).
//! - [`detectar`]: qué es un archivo, por su firma y su extensión (`detectar.py`).
//! - [`paquetes`]: el nombre de un .deb o un .rpm (`paquetes.py`).
//! - [`flathub`], [`flatpak`]: buscar y instalar en Flathub (`flathub.py`, `flatpak.py`).
//! - [`mods`]: mods de Minecraft (`mods.py`).
//! - [`registro`], [`papelera`]: lo instalado y cómo se deshace, siempre a la papelera.
//! - [`instalar`]: AppImages, programas .tar.gz y lo que corre código (`instalar.py`).
//! - [`motor`]: qué se le ofrece a la persona y hacerlo (`motor.py`).
//! - [`sesion`]: lo que hoy hace `puente.py` sin Qt; la ventana (cxx-qt) solo lo envuelve.
//! - [`instancia`]: una sola ventana por persona (el mismo socket que la de Python).
//!
//! Nada de esto instala por su cuenta algo de la distro: solo copia a la carpeta de la persona, llama a
//! `flatpak --user` y abre lo que la persona confirma. Las pruebas usan programas falsos y una casa temporal.

pub mod comprimidos;
pub mod contexto;
pub mod detectar;
pub mod flathub;
pub mod flatpak;
pub mod instalar;
pub mod instancia;
pub mod mods;
pub mod motor;
pub mod paquetes;
pub mod papelera;
pub mod proc;
pub mod registro;
pub mod rutas;
pub mod sesion;
pub mod textos;
mod textos_tabla;
pub mod util;

pub use contexto::Contexto;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const VERSION: &str = "1.0";
pub const NOMBRE: &str = "Vortex App Installer";
pub const APP_ID: &str = "vortex-app-installer";
/// Cabecera User-Agent de las búsquedas en Flathub.
pub const AGENTE: &str = "vortex-app-installer/1.0";

/// Todo lo que puede salir mal. Los mensajes ya van traducidos: se le muestran a la persona tal cual.
#[derive(Debug)]
pub enum Error {
    /// El destino ya existe. `sugerida` es un nombre libre al lado («Carpeta (2)»).
    Conflicto { ruta: PathBuf, sugerida: PathBuf },
    /// El comprimido pide contraseña (o la que se dio no es).
    Contrasena { incorrecta: bool, mensaje: String },
    /// Falta un programa (`7z`, `unrar`, `bsdtar`…); `comando` es lo que hay que correr para instalarlo.
    FaltaPrograma {
        programa: String,
        comando: String,
        mensaje: String,
    },
    Cancelado,
    /// Un error con su texto (`ErrorComprimido`, `ErrorInstalar`, `ErrorFlatpak`, `RuntimeError`).
    Mensaje(String),
    /// Lo que `motor::ejecutar` le muestra a la persona: texto y, si aplica, un comando para copiar.
    Problema { mensaje: String, comando: String },
    Io(std::io::Error),
}

pub type Resultado<T> = Result<T, Error>;

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Conflicto { ruta, .. } => write!(f, "{}", ruta.display()),
            Error::Contrasena { mensaje, .. }
            | Error::FaltaPrograma { mensaje, .. }
            | Error::Mensaje(mensaje)
            | Error::Problema { mensaje, .. } => write!(f, "{mensaje}"),
            Error::Cancelado => write!(f, "cancelado"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl Error {
    /// Como `strerror` de Python: «Permission denied», sin el «(os error 13)» de Rust.
    pub fn texto_io(e: &std::io::Error) -> String {
        let s = e.to_string();
        match s.rfind(" (os error") {
            Some(i) => s[..i].to_string(),
            None => s,
        }
    }
}

/// El avance y la cancelación de un trabajo largo. Se puede clonar y pasar a otro hilo.
#[derive(Clone)]
pub struct Control {
    progreso: Option<Arc<dyn Fn(f64) + Send + Sync>>,
    cancelar: Arc<AtomicBool>,
    factor: f64,
}

impl Control {
    /// Sin avance y sin cancelar.
    pub fn nulo() -> Self {
        Control {
            progreso: None,
            cancelar: Arc::default(),
            factor: 1.0,
        }
    }

    pub fn nuevo(progreso: impl Fn(f64) + Send + Sync + 'static, cancelar: Arc<AtomicBool>) -> Self {
        Control {
            progreso: Some(Arc::new(progreso)),
            cancelar,
            factor: 1.0,
        }
    }

    /// Solo cancelación (sin avance).
    pub fn con_cancelar(cancelar: Arc<AtomicBool>) -> Self {
        Control {
            progreso: None,
            cancelar,
            factor: 1.0,
        }
    }

    pub fn avanzar(&self, fraccion: f64) {
        if let Some(p) = &self.progreso {
            p(fraccion * self.factor);
        }
    }

    pub fn hay_progreso(&self) -> bool {
        self.progreso.is_some()
    }

    pub fn cancelado(&self) -> bool {
        self.cancelar.load(Ordering::SeqCst)
    }

    /// El mismo control, pero el avance se multiplica por `k` (para un tramo de un trabajo mayor).
    pub fn escalado(&self, k: f64) -> Control {
        Control {
            factor: self.factor * k,
            ..self.clone()
        }
    }
}
