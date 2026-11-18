//! Lanzadores (`.desktop`): de ahí salen el nombre y el ícono de cada app, buscados por la clase de su
//! ventana o por su ejecutable.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lanzador {
    pub id: String,
    pub nombre: String,
    pub icono: String,
    pub exe: String,
    pub clase: String,
    pub oculto: bool,
}

/// Las carpetas `applications` de XDG y de Flatpak (la tuya primero).
pub fn carpetas_aplicaciones() -> Vec<PathBuf> {
    let home = vortex_core::rutas::home();
    let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let mut datos = vec![
        var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share").to_string_lossy().into_owned()),
    ];
    datos.extend(
        var("XDG_DATA_DIRS")
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into())
            .split(':')
            .map(String::from),
    );
    datos.push(
        home.join(".local/share/flatpak/exports/share")
            .to_string_lossy()
            .into_owned(),
    );
    datos.push("/var/lib/flatpak/exports/share".into());
    let mut carpetas: Vec<PathBuf> = Vec::new();
    for d in datos {
        let p = Path::new(&d).join("applications");
        if !d.is_empty() && p.is_dir() && !carpetas.contains(&p) {
            carpetas.push(p);
        }
    }
    carpetas
}

fn idiomas() -> Vec<String> {
    let lang = std::env::var("LC_MESSAGES")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LANG").ok().filter(|v| !v.is_empty()))
        .unwrap_or_else(|| "es_MX".into());
    let lang = lang.split('.').next().unwrap_or("es_MX").to_string();
    let corto = lang.split('_').next().unwrap_or("").to_string();
    vec![lang, corto]
}

/// Las claves de `[Desktop Entry]` que se usan aquí (Name en tu idioma, Icon, Exec, StartupWMClass).
pub fn leer_desktop(ruta: &Path) -> Option<Lanzador> {
    let texto = String::from_utf8_lossy(&std::fs::read(ruta).ok()?).into_owned();
    let mut claves: HashMap<String, String> = HashMap::new();
    let mut dentro = false;
    for linea in texto.lines() {
        let linea = linea.trim();
        if linea.starts_with('[') {
            dentro = linea == "[Desktop Entry]";
            continue;
        }
        if dentro
            && !linea.starts_with('#')
            && let Some((k, v)) = linea.split_once('=')
        {
            claves
                .entry(k.trim().to_string())
                .or_insert_with(|| v.trim().to_string());
        }
    }
    if claves.get("Type").map_or("Application", String::as_str) != "Application" {
        return None;
    }
    let nombre = idiomas()
        .iter()
        .find_map(|l| claves.get(&format!("Name[{l}]")))
        .or_else(|| claves.get("Name"))
        .cloned()
        .unwrap_or_default();
    let exe = claves
        .get("Exec")
        .map(String::as_str)
        .unwrap_or("")
        .split_whitespace()
        .find(|p| *p != "env" && !p.contains('='))
        .map(|p| {
            let p = p.trim_matches('"');
            Path::new(p)
                .file_name()
                .map_or(String::new(), |f| f.to_string_lossy().into_owned())
        })
        .unwrap_or_default();
    Some(Lanzador {
        id: ruta
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
        nombre,
        icono: claves.get("Icon").cloned().unwrap_or_default(),
        exe,
        clase: claves.get("StartupWMClass").cloned().unwrap_or_default(),
        oculto: claves
            .get("NoDisplay")
            .is_some_and(|v| v.eq_ignore_ascii_case("true")),
    })
}

/// Índice de lanzadores: por id (archivo), por StartupWMClass y por ejecutable, sin mayúsculas.
#[derive(Default)]
pub struct Aplicaciones {
    por_id: Vec<(String, Lanzador)>,
    por_clase: HashMap<String, Lanzador>,
    por_exe: HashMap<String, Lanzador>,
}

impl Aplicaciones {
    pub fn new() -> Self {
        Self::de_carpetas(&carpetas_aplicaciones())
    }

    pub fn de_carpetas(carpetas: &[PathBuf]) -> Self {
        let mut a = Aplicaciones::default();
        for carpeta in carpetas {
            let mut rutas: Vec<PathBuf> = std::fs::read_dir(carpeta)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "desktop"))
                .collect();
            rutas.sort();
            for ruta in rutas {
                let Some(d) = leer_desktop(&ruta) else { continue };
                if d.nombre.is_empty() {
                    continue;
                }
                // La primera carpeta (la tuya) manda.
                let id = d.id.to_lowercase();
                if !a.por_id.iter().any(|(i, _)| *i == id) {
                    a.por_id.push((id, d.clone()));
                }
                if !d.clase.is_empty() {
                    a.por_clase
                        .entry(d.clase.to_lowercase())
                        .or_insert_with(|| d.clone());
                }
                let exe = d.exe.to_lowercase();
                if !exe.is_empty() && !["flatpak", "sh", "bash", "env"].contains(&exe.as_str()) {
                    // El lanzador visible gana a los ocultos (kitty.desktop y no «kitty URL Launcher»).
                    let reemplaza = a.por_exe.get(&exe).is_none_or(|v| v.oculto && !d.oculto);
                    if reemplaza {
                        a.por_exe.insert(exe, d);
                    }
                }
            }
        }
        a
    }

    fn por_id(&self, id: &str) -> Option<&Lanzador> {
        self.por_id.iter().find(|(i, _)| i == id).map(|(_, d)| d)
    }

    pub fn de_clase(&self, clase: &str) -> Option<&Lanzador> {
        let c = clase.to_lowercase();
        if c.is_empty() {
            return None;
        }
        if let Some(d) = self.por_clase.get(&c) {
            return Some(d);
        }
        if let Some(d) = self.por_id(&c) {
            return Some(d);
        }
        // org.gnome.Nautilus ← «nautilus»; com.spotify.Client…
        for (ident, d) in &self.por_id {
            if ident.rsplit('.').next() == Some(c.as_str()) || ident.ends_with(&format!(".{c}")) {
                return Some(d);
            }
        }
        self.por_exe.get(&c)
    }

    pub fn de_ejecutable(&self, nombre: &str) -> Option<&Lanzador> {
        let n = nombre.to_lowercase();
        self.por_id(&n).or_else(|| self.por_exe.get(&n))
    }
}
