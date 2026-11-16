//! Dónde van las capturas, cómo se llaman y cuál es la última (lo de `motor.py` y `puente.py`).
//!
//! Las fotos van a Imágenes/Cámara y los videos a Vídeos/Cámara, con las carpetas de `user-dirs.dirs`
//! (las mismas que da `QStandardPaths` en la de Python). Para pruebas, `VORTEX_VIDEO_CARPETA` las cambia
//! por `<carpeta>/PicturesLocation/Cámara` y `<carpeta>/MoviesLocation/Cámara`, como en la de Python.

use crate::hora::FechaHora;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const EXT_FOTO: [&str; 3] = ["jpg", "jpeg", "png"];
pub const EXT_VIDEO: [&str; 3] = ["mp4", "mkv", "webm"];

fn extension(ruta: &Path) -> String {
    ruta.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

pub fn es_video(ruta: &Path) -> bool {
    EXT_VIDEO.contains(&extension(ruta).as_str())
}

pub fn es_captura(ruta: &Path) -> bool {
    let e = extension(ruta);
    EXT_FOTO.contains(&e.as_str()) || EXT_VIDEO.contains(&e.as_str())
}

/// «Foto 2026-11-05 14.03.21.jpg»; si ya existe uno de ese segundo, «… (2).jpg», «… (3).jpg»…
pub fn nombre_nuevo(carpeta: &Path, prefijo: &str, extension: &str, ahora: FechaHora) -> PathBuf {
    let sello = ahora.sello();
    let mut ruta = carpeta.join(format!("{prefijo} {sello}{extension}"));
    let mut n = 2;
    while ruta.exists() {
        ruta = carpeta.join(format!("{prefijo} {sello} ({n}){extension}"));
        n += 1;
    }
    ruta
}

/// La captura más nueva de esas carpetas (la del cuadrito de abajo al abrir la app).
pub fn ultima(carpetas: &[&Path]) -> Option<PathBuf> {
    let mut mejor: Option<(PathBuf, SystemTime)> = None;
    for carpeta in carpetas {
        let Ok(entradas) = fs::read_dir(carpeta) else {
            continue;
        };
        for e in entradas.flatten() {
            let ruta = e.path();
            if !es_captura(&ruta) {
                continue;
            }
            let Ok(t) = e.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if mejor.as_ref().is_none_or(|(_, m)| t > *m) {
                mejor = Some((ruta, t));
            }
        }
    }
    mejor.map(|(r, _)| r)
}

/// Las carpetas donde se guardan las fotos y los videos.
#[derive(Debug, Clone, PartialEq)]
pub struct Carpetas {
    pub fotos: PathBuf,
    pub videos: PathBuf,
}

impl Carpetas {
    /// Las de la persona (o las de `VORTEX_VIDEO_CARPETA`, en pruebas).
    pub fn de_la_persona() -> Self {
        if let Some(p) = std::env::var_os("VORTEX_VIDEO_CARPETA").filter(|p| !p.is_empty()) {
            return Self::de_prueba(Path::new(&p));
        }
        let home = vortex_core::rutas::home();
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".config"));
        let dirs = fs::read_to_string(config.join("user-dirs.dirs")).unwrap_or_default();
        Carpetas {
            fotos: carpeta_de_usuario(&dirs, "XDG_PICTURES_DIR", &home, "Pictures").join("Cámara"),
            videos: carpeta_de_usuario(&dirs, "XDG_VIDEOS_DIR", &home, "Videos").join("Cámara"),
        }
    }

    pub fn de_prueba(base: &Path) -> Self {
        Carpetas {
            fotos: base.join("PicturesLocation").join("Cámara"),
            videos: base.join("MoviesLocation").join("Cámara"),
        }
    }

    pub fn ultima(&self) -> Option<PathBuf> {
        ultima(&[&self.fotos, &self.videos])
    }
}

/// Una carpeta de `user-dirs.dirs` (`XDG_PICTURES_DIR="$HOME/Imágenes"`), como la lee QStandardPaths.
pub fn carpeta_de_usuario(contenido: &str, clave: &str, home: &Path, por_defecto: &str) -> PathBuf {
    for linea in contenido.lines() {
        let linea = linea.trim();
        let Some(resto) = linea
            .strip_prefix(clave)
            .and_then(|r| r.trim_start().strip_prefix('='))
        else {
            continue;
        };
        let valor = resto.trim().trim_matches('"');
        let ruta = if let Some(r) = valor.strip_prefix("$HOME") {
            home.join(r.trim_start_matches('/'))
        } else {
            PathBuf::from(valor)
        };
        if ruta.is_absolute() && ruta != home {
            return ruta;
        }
    }
    home.join(por_defecto)
}
