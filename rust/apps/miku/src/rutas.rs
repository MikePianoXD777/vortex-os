//! Dónde vive cada cosa de Miku (la misma disposición que `rutas.py`): el código y la página web en
//! `/usr/share/vortex-miku`, los modelos y la memoria en `~/.local/share/vortex/miku`, la configuración en
//! `~/.config/vortex/miku.json`. Las variables `VORTEX_MIKU_*` mueven lo demás (pruebas).

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Rutas {
    /// La carpeta del código de Miku (con `web/`, `persona/`, `voces.json`, `miku/voz_worker.py`).
    pub raiz: PathBuf,
    pub web: PathBuf,
    pub vendor: PathBuf,
    pub personas: PathBuf,
    pub catalogo_voces: PathBuf,
    pub modelo_sistema: PathBuf,
    pub config_dir: PathBuf,
    pub config: PathBuf,
    pub datos: PathBuf,
    pub modelo_usuario: PathBuf,
    pub voz: PathBuf,
    pub oido: PathBuf,
    pub marca_internet: PathBuf,
    pub cerebro_cwd: PathBuf,
    pub libs_py: PathBuf,
    pub natural: PathBuf,
    pub runtime: PathBuf,
}

fn variable(nombre: &str) -> Option<PathBuf> {
    std::env::var_os(nombre)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

impl Rutas {
    /// Las de verdad, según el entorno (HOME, XDG y las `VORTEX_MIKU_*`).
    pub fn real() -> Self {
        let raiz = variable("VORTEX_MIKU_RAIZ").unwrap_or_else(buscar_raiz);
        let web = raiz.join("web");
        let datos = vortex_core::rutas::datos().join("miku");
        let config_dir = vortex_core::rutas::config();
        Self {
            vendor: variable("VORTEX_MIKU_VENDOR").unwrap_or_else(|| web.join("vendor")),
            personas: raiz.join("persona"),
            catalogo_voces: variable("VORTEX_MIKU_CATALOGO").unwrap_or_else(|| raiz.join("voces.json")),
            modelo_sistema: variable("VORTEX_MIKU_MODELO_SISTEMA")
                .unwrap_or_else(|| "/usr/share/vortex-miku/modelo/modelo.vrm".into()),
            config: config_dir.join("miku.json"),
            modelo_usuario: datos.join("modelo.vrm"),
            voz: datos.join("voz"),
            oido: datos.join("oido"),
            marca_internet: variable("VORTEX_MIKU_MARCA_INTERNET")
                .unwrap_or_else(|| "/etc/vortex/miku-internet".into()),
            cerebro_cwd: datos.join("cerebro"),
            libs_py: variable("VORTEX_MIKU_LIBS").unwrap_or_else(|| "/usr/lib/vortex-miku/python".into()),
            natural: variable("VORTEX_MIKU_NATURAL").unwrap_or_else(|| datos.join("natural")),
            runtime: runtime(),
            web,
            raiz,
            config_dir,
            datos,
        }
    }

    /// Todo colgando de una carpeta (para las pruebas: nada toca la de la persona).
    pub fn en(base: &Path) -> Self {
        let raiz = base.join("raiz");
        let datos = base.join("datos/miku");
        let config_dir = base.join("config");
        Self {
            web: raiz.join("web"),
            vendor: raiz.join("web/vendor"),
            personas: raiz.join("persona"),
            catalogo_voces: raiz.join("voces.json"),
            modelo_sistema: base.join("sistema/modelo.vrm"),
            config: config_dir.join("miku.json"),
            modelo_usuario: datos.join("modelo.vrm"),
            voz: datos.join("voz"),
            oido: datos.join("oido"),
            marca_internet: base.join("marca-internet"),
            cerebro_cwd: datos.join("cerebro"),
            libs_py: base.join("libs"),
            natural: datos.join("natural"),
            runtime: base.join("runtime"),
            raiz,
            config_dir,
            datos,
        }
    }

    pub fn natural_python(&self) -> PathBuf {
        self.natural.join("venv/bin/python")
    }
    pub fn natural_listo(&self) -> PathBuf {
        self.natural.join("listo")
    }
    pub fn natural_modo(&self) -> PathBuf {
        self.natural.join("modo")
    }

    /// El VRM que se usa: el de miku.json, el del usuario o el que dejó la ISO. `None` = no hay.
    pub fn ruta_modelo(&self, modelo_cfg: &str) -> Option<PathBuf> {
        let mut c = Vec::new();
        if !modelo_cfg.is_empty() {
            c.push(expandir_tilde(modelo_cfg));
        }
        c.push(self.modelo_usuario.clone());
        c.push(self.modelo_sistema.clone());
        c.into_iter().find(|p| p.is_file())
    }

    /// `CREDITO.txt` junto al modelo (la licencia del modelo pide crédito). Vacío si no hay.
    pub fn leer_credito(modelo: Option<&Path>) -> String {
        modelo
            .and_then(|m| std::fs::read_to_string(m.parent()?.join("CREDITO.txt")).ok())
            .map(|t| t.trim().to_string())
            .unwrap_or_default()
    }

    /// Audios de la voz mientras suenan (se borran en cuanto suenan).
    pub fn temporales(&self) -> PathBuf {
        let d = self.runtime.join("vortex-miku");
        let _ = std::fs::create_dir_all(&d);
        d
    }
}

/// `$XDG_RUNTIME_DIR` o, si no, `/tmp/vortex-miku-<uid>` (700), como `runtime()` de Python.
pub fn runtime() -> PathBuf {
    if let Some(b) = variable("XDG_RUNTIME_DIR").filter(|p| p.is_dir()) {
        return b;
    }
    let d = std::env::temp_dir().join(format!("vortex-miku-{}", crate::sistema::uid()));
    let _ = std::fs::create_dir_all(&d);
    let _ = crate::sistema::permisos(&d, 0o700);
    d
}

pub fn expandir_tilde(r: &str) -> PathBuf {
    if let Some(resto) = r.strip_prefix("~/") {
        return vortex_core::rutas::home().join(resto);
    }
    if r == "~" {
        return vortex_core::rutas::home();
    }
    PathBuf::from(r)
}

/// Donde está `web/`: la raíz de apps/miku (desde el repo), la del paquete, o junto al binario.
fn buscar_raiz() -> PathBuf {
    let repo = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../apps/miku"));
    [PathBuf::from("/usr/share/vortex-miku"), repo.clone()]
        .into_iter()
        .find(|c| c.join("web/index.html").is_file())
        .unwrap_or(repo)
}

/// Los núcleos físicos (máximo 8) para la voz y el oído; `VORTEX_MIKU_HILOS` manda.
pub fn hilos() -> usize {
    if let Some(n) = std::env::var("VORTEX_MIKU_HILOS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        return n.max(1);
    }
    let mut nucleos = std::collections::HashSet::new();
    if let Ok(d) = std::fs::read_dir("/sys/devices/system/cpu") {
        for e in d.flatten() {
            let t = e.path().join("topology");
            let leer = |n: &str| {
                std::fs::read_to_string(t.join(n))
                    .ok()
                    .map(|s| s.trim().to_string())
            };
            if let (Some(p), Some(c)) = (leer("physical_package_id"), leer("core_id")) {
                nucleos.insert((p, c));
            }
        }
    }
    let n = if nucleos.is_empty() {
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
    } else {
        nucleos.len()
    };
    n.clamp(1, 8)
}
