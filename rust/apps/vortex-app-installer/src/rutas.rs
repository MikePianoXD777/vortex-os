//! Dónde deja las cosas la app. Todo se calcula desde la casa y `XDG_DATA_HOME` / `XDG_CONFIG_HOME` de
//! la [`Rutas`] (las pruebas arman una con una casa temporal, así que nada se guarda en el equipo de verdad).

use crate::util::{abspath, normpath};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rutas {
    pub casa: PathBuf,
    /// `XDG_DATA_HOME` si es una ruta absoluta.
    pub xdg_datos: Option<PathBuf>,
    /// `XDG_CONFIG_HOME` si es una ruta absoluta.
    pub xdg_config: Option<PathBuf>,
}

impl Rutas {
    pub fn del_entorno() -> Self {
        let var = |n: &str| {
            std::env::var_os(n)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
        };
        Rutas {
            casa: std::env::var_os("HOME")
                .filter(|v| !v.is_empty())
                .map_or_else(vortex_core::rutas::home, PathBuf::from),
            xdg_datos: var("XDG_DATA_HOME"),
            xdg_config: var("XDG_CONFIG_HOME"),
        }
    }

    /// Una casa propia y sin variables XDG (pruebas).
    pub fn bajo(casa: &Path) -> Self {
        Rutas {
            casa: casa.to_path_buf(),
            xdg_datos: None,
            xdg_config: None,
        }
    }

    pub fn datos_xdg(&self) -> PathBuf {
        self.xdg_datos
            .clone()
            .unwrap_or_else(|| self.casa.join(".local/share"))
    }

    pub fn config_xdg(&self) -> PathBuf {
        self.xdg_config
            .clone()
            .unwrap_or_else(|| self.casa.join(".config"))
    }

    /// Los AppImages instalados: `~/Aplicaciones` si ya la usas (Vortexine's en español), si no
    /// `~/Applications`, la de siempre para AppImages (AppImageLauncher, Gear Lever).
    pub fn appimages(&self) -> PathBuf {
        let espanol = self.casa.join("Aplicaciones");
        if espanol.is_dir() {
            espanol
        } else {
            self.casa.join("Applications")
        }
    }

    /// Los programas que vienen en un .tar.gz, cada uno en su carpeta.
    pub fn opt(&self) -> PathBuf {
        self.casa.join(".local/opt")
    }

    pub fn lanzadores(&self) -> PathBuf {
        self.datos_xdg().join("applications")
    }

    pub fn iconos(&self) -> PathBuf {
        self.datos_xdg().join("icons/hicolor")
    }

    pub fn propios(&self) -> PathBuf {
        self.datos_xdg().join("vortex-app-installer")
    }

    pub fn registro(&self) -> PathBuf {
        self.propios().join("instalado.json")
    }

    pub fn desinstaladores(&self) -> PathBuf {
        self.propios().join("desinstaladores")
    }

    pub fn papelera(&self) -> PathBuf {
        self.datos_xdg().join("Trash")
    }

    /// Una carpeta de `user-dirs.dirs` (`XDG_DOWNLOAD_DIR`…), con `$HOME` ya reemplazado.
    fn carpeta_usuario(&self, clave: &str, respaldo: &str) -> PathBuf {
        let texto = std::fs::read_to_string(self.config_xdg().join("user-dirs.dirs")).unwrap_or_default();
        let patron = format!("{clave}=\"");
        for linea in texto.lines() {
            if let Some(resto) = linea.strip_prefix(&patron)
                && let Some(fin) = resto.find('"')
            {
                let valor = resto[..fin].replace("$HOME", &self.casa.to_string_lossy());
                if valor.starts_with('/') {
                    return PathBuf::from(valor);
                }
            }
        }
        self.casa.join(respaldo)
    }

    pub fn descargas(&self) -> PathBuf {
        self.carpeta_usuario("XDG_DOWNLOAD_DIR", "Descargas")
    }

    /// Carpetas que deshacer nunca manda a la papelera, aunque el registro las nombre.
    pub fn protegidas(&self) -> BTreeSet<PathBuf> {
        let c = &self.casa;
        let mut fijas: Vec<PathBuf> = vec![
            PathBuf::from("/"),
            c.clone(),
            c.join(".local"),
            c.join(".local/share"),
            c.join(".config"),
            self.datos_xdg(),
            self.config_xdg(),
            self.appimages(),
            self.opt(),
            self.lanzadores(),
            self.iconos(),
            self.propios(),
            self.desinstaladores(),
            self.papelera(),
        ];
        for n in [
            "Descargas", "Downloads", "Documentos", "Documents", "Escritorio", "Desktop", "Imágenes",
            "Pictures", "Música", "Music", "Vídeos", "Videos", ".minecraft",
        ] {
            fijas.push(c.join(n));
        }
        for (clave, respaldo) in [
            ("XDG_DOWNLOAD_DIR", "Descargas"),
            ("XDG_DOCUMENTS_DIR", "Documentos"),
            ("XDG_DESKTOP_DIR", "Escritorio"),
            ("XDG_PICTURES_DIR", "Imágenes"),
            ("XDG_MUSIC_DIR", "Música"),
            ("XDG_VIDEOS_DIR", "Vídeos"),
        ] {
            fijas.push(self.carpeta_usuario(clave, respaldo));
        }
        fijas
            .into_iter()
            .map(|p| PathBuf::from(normpath(&abspath(&p).to_string_lossy())))
            .collect()
    }
}
