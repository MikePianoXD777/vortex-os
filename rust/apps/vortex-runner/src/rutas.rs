//! Dónde guarda sus cosas Vortexine's Runner (igual que `rutas.py`).
//!
//! - DATOS (`~/.local/share/vortex-runner-datos`): la biblioteca (`biblioteca.json`), los íconos sacados de
//!   los .exe y los prefijos de Wine (uno por programa, en `prefijos/`).
//! - LANZADORES (`~/.local/share/applications`): los accesos de «Agregar al lanzador».
//!
//! Con `VORTEX_RUNNER_HOME` (pruebas) todo cuelga de esa carpeta.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rutas {
    pub datos: PathBuf,
    pub lanzadores: PathBuf,
}

impl Rutas {
    pub fn del_entorno() -> Self {
        if let Some(propia) = std::env::var_os("VORTEX_RUNNER_HOME").filter(|v| !v.is_empty()) {
            return Self::bajo(&expandir(Path::new(&propia)));
        }
        let share = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| vortex_core::rutas::home().join(".local").join("share"));
        Rutas {
            datos: share.join("vortex-runner-datos"),
            lanzadores: share.join("applications"),
        }
    }

    pub fn bajo(base: &Path) -> Self {
        Rutas {
            datos: base.join("datos"),
            lanzadores: base.join("applications"),
        }
    }

    pub fn biblioteca(&self) -> PathBuf {
        self.datos.join("biblioteca.json")
    }
    pub fn prefijos(&self) -> PathBuf {
        self.datos.join("prefijos")
    }
    pub fn iconos(&self) -> PathBuf {
        self.datos.join("iconos")
    }
    pub fn registros(&self) -> PathBuf {
        self.datos.join("registros")
    }
}

/// `~` al principio de una ruta (como `Path.expanduser`).
pub fn expandir(ruta: &Path) -> PathBuf {
    match ruta.strip_prefix("~") {
        Ok(resto) => vortex_core::rutas::home().join(resto),
        Err(_) => ruta.to_path_buf(),
    }
}

/// Como `Path.resolve()` de Python: absoluta, sin `.` ni `..` y con los enlaces resueltos; si no existe,
/// igual da una ruta absoluta limpia (no falla).
pub fn resolver(ruta: &Path) -> PathBuf {
    if let Ok(r) = ruta.canonicalize() {
        return r;
    }
    let absoluta = if ruta.is_absolute() {
        ruta.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(ruta)
    };
    // Resuelve lo que sí existe (el padre más cercano) y le pega el resto.
    let mut limpia = PathBuf::new();
    for c in absoluta.components() {
        match c {
            std::path::Component::ParentDir => {
                limpia.pop();
            }
            std::path::Component::CurDir => {}
            otro => limpia.push(otro.as_os_str()),
        }
    }
    let mut resto = Vec::new();
    let mut cabeza = limpia.clone();
    while !cabeza.exists() {
        match (cabeza.file_name().map(|n| n.to_owned()), cabeza.parent()) {
            (Some(n), Some(p)) => {
                resto.push(n);
                cabeza = p.to_path_buf();
            }
            _ => break,
        }
    }
    let mut base = cabeza.canonicalize().unwrap_or(cabeza);
    for n in resto.into_iter().rev() {
        base.push(n);
    }
    base
}

/// Normaliza una ruta sin tocar el disco (`os.path.normpath`).
pub fn normalizar(ruta: &str) -> String {
    let absoluta = ruta.starts_with('/');
    let mut partes: Vec<&str> = Vec::new();
    for p in ruta.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                if partes.last().is_some_and(|u| *u != "..") {
                    partes.pop();
                } else if !absoluta {
                    partes.push("..");
                }
            }
            otro => partes.push(otro),
        }
    }
    let unida = partes.join("/");
    match (absoluta, unida.is_empty()) {
        (true, _) => format!("/{unida}"),
        (false, true) => ".".into(),
        (false, false) => unida,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normaliza_como_python() {
        assert_eq!(normalizar("/a/b/../c//d/."), "/a/c/d");
        assert_eq!(normalizar("a/../.."), "..");
        assert_eq!(normalizar(""), ".");
        assert_eq!(normalizar("//x"), "/x");
    }

    #[test]
    fn rutas_bajo_una_casa() {
        let r = Rutas::bajo(Path::new("/tmp/casa"));
        assert_eq!(r.biblioteca(), PathBuf::from("/tmp/casa/datos/biblioteca.json"));
        assert_eq!(r.prefijos(), PathBuf::from("/tmp/casa/datos/prefijos"));
        assert_eq!(r.lanzadores, PathBuf::from("/tmp/casa/applications"));
    }

    #[test]
    fn resolver_lo_que_no_existe() {
        let c = vortex_core::carpeta_de_prueba("runner-resolver");
        let r = resolver(&c.join("a/../nada/x.exe"));
        assert_eq!(r, c.canonicalize().unwrap().join("nada/x.exe"));
    }
}
