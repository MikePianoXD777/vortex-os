//! Dónde guarda sus cosas Vortexine's Local AI (igual que `rutas.py`):
//!
//! - `datos`: los chats y la configuración (`~/.local/share/vortex-local-ai`).
//! - `cache`: el índice de la memoria del vault, que se puede rehacer (`~/.cache/vortex-local-ai`).
//! - `vault_por_defecto`: el vault que se crea si no abres otro (`~/Documentos/Vortexine Local AI`).
//!
//! Respetan `XDG_DATA_HOME` y `XDG_CACHE_HOME`. Con `VORTEX_LOCAL_AI_HOME` (para las pruebas) todo cuelga de
//! esa carpeta: `<home>/datos`, `<home>/cache` y `<home>/vault`, y no se toca nada de tu usuario.

use std::path::{Path, PathBuf};

use crate::APP_ID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rutas {
    pub datos: PathBuf,
    pub cache: PathBuf,
    pub vault_por_defecto: PathBuf,
}

fn absoluta(nombre: &str) -> Option<PathBuf> {
    std::env::var_os(nombre)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
}

/// La carpeta de documentos del usuario (`XDG_DOCUMENTS_DIR`; si no, `user-dirs.dirs`, `~/Documentos` o
/// `~/Documents`).
pub fn carpeta_documentos(casa: &Path) -> PathBuf {
    let casa_txt = casa.to_string_lossy().to_string();
    if let Some(xdg) = std::env::var_os("XDG_DOCUMENTS_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(xdg.to_string_lossy().replace("$HOME", &casa_txt));
    }
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| casa.join(".config"));
    if let Ok(texto) = std::fs::read_to_string(config.join("user-dirs.dirs")) {
        for linea in texto.lines() {
            if let Some(valor) = linea.strip_prefix("XDG_DOCUMENTS_DIR=") {
                let valor = valor.trim().trim_matches('"').replace("$HOME", &casa_txt);
                if !valor.is_empty() {
                    return PathBuf::from(valor);
                }
            }
        }
    }
    for nombre in ["Documentos", "Documents"] {
        if casa.join(nombre).is_dir() {
            return casa.join(nombre);
        }
    }
    casa.join("Documentos")
}

impl Rutas {
    /// Según las variables de entorno de ahora.
    pub fn actual() -> Rutas {
        if let Some(propia) = std::env::var_os("VORTEX_LOCAL_AI_HOME").filter(|v| !v.is_empty()) {
            let base = PathBuf::from(propia);
            return Rutas {
                datos: base.join("datos"),
                cache: base.join("cache"),
                vault_por_defecto: base.join("vault"),
            };
        }
        let casa = vortex_core::rutas::home();
        let datos = absoluta("XDG_DATA_HOME").unwrap_or_else(|| casa.join(".local").join("share"));
        let cache = absoluta("XDG_CACHE_HOME").unwrap_or_else(|| casa.join(".cache"));
        Rutas {
            datos: datos.join(APP_ID),
            cache: cache.join(APP_ID),
            vault_por_defecto: carpeta_documentos(&casa).join("Vortexine Local AI"),
        }
    }

    /// Carpeta de los chats guardados (`datos/chats`).
    pub fn chats(&self) -> PathBuf {
        self.datos.join("chats")
    }

    /// Archivo de configuración de la app (`datos/config.json`).
    pub fn config(&self) -> PathBuf {
        self.datos.join("config.json")
    }
}
