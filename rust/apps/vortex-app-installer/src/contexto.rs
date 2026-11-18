//! Todo lo que cambia entre la PC real y las pruebas: la casa, las variables XDG, dónde se buscan los
//! programas (`PATH`), el idioma, la dirección de Flathub y si de verdad se abren programas.
//!
//! En la app sale del entorno ([`Contexto::del_entorno`]); las pruebas lo arman a mano
//! ([`Contexto::de_prueba`]) y así no tocan variables globales (corren en paralelo) ni nada real.

use crate::rutas::Rutas;
use crate::textos::{Idioma, tr};
use crate::util::nueva_sesion;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

pub const API_FLATHUB: &str = "https://flathub.org/api/v2";

/// Los programas que «se habrían abierto» (en las pruebas no se abre ninguno de verdad).
pub type Lanzados = Arc<Mutex<Vec<Vec<String>>>>;

#[derive(Clone, Debug)]
pub struct Contexto {
    pub rutas: Rutas,
    pub idioma: Idioma,
    /// El `PATH` donde se buscan `bsdtar`, `flatpak`, `kitty`, `wine`…
    pub path: OsString,
    /// `VORTEX_APP_INSTALLER_FLATHUB` o la API de siempre.
    pub flathub: String,
    /// `Some` = pruebas: [`Contexto::lanzar`] anota el comando en vez de abrirlo.
    pub lanzados: Option<Lanzados>,
    /// Pruebas: si hay FUSE 2 (si no, se mira el sistema).
    pub fuse2: Option<bool>,
}

impl Contexto {
    pub fn del_entorno() -> Self {
        Contexto {
            rutas: Rutas::del_entorno(),
            idioma: Idioma::del_entorno(),
            path: std::env::var_os("PATH").unwrap_or_default(),
            flathub: std::env::var("VORTEX_APP_INSTALLER_FLATHUB")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| API_FLATHUB.into()),
            lanzados: None,
            fuse2: None,
        }
    }

    /// Una casa temporal en `base/casa`, un `PATH` con `base/bin` primero (para los programas falsos),
    /// español, y sin abrir programas de verdad.
    pub fn de_prueba(base: &Path) -> Self {
        let casa = base.join("casa");
        let bins = base.join("bin");
        let _ = std::fs::create_dir_all(&casa);
        let _ = std::fs::create_dir_all(&bins);
        let real = std::env::var_os("PATH").unwrap_or_default();
        let mut carpetas = vec![bins];
        carpetas.extend(std::env::split_paths(&real));
        Contexto {
            rutas: Rutas::bajo(&casa),
            idioma: Idioma::Es,
            path: std::env::join_paths(carpetas).unwrap_or(real),
            flathub: API_FLATHUB.into(),
            lanzados: Some(Arc::default()),
            fuse2: Some(true),
        }
    }

    pub fn tr(&self, clave: &str, valores: &[(&str, &str)]) -> String {
        tr(self.idioma, clave, valores)
    }

    /// `shutil.which`: el primer ejecutable con ese nombre en `PATH` (o la ruta misma si trae «/»).
    pub fn which(&self, nombre: &str) -> Option<PathBuf> {
        if nombre.contains('/') {
            let p = PathBuf::from(nombre);
            return crate::util::es_ejecutable(&p).then_some(p);
        }
        std::env::split_paths(&self.path)
            .map(|c| c.join(nombre))
            .find(|p| p.is_file() && crate::util::es_ejecutable(p))
    }

    /// Un comando ya con la casa, `PATH` y XDG de este contexto (lo demás del entorno se hereda).
    /// Los programas se buscan en `self.path`, no en el del proceso.
    pub fn comando(&self, programa: &str) -> Command {
        let exe = self.which(programa).unwrap_or_else(|| PathBuf::from(programa));
        let mut c = Command::new(exe);
        self.preparar(&mut c);
        c
    }

    /// Lo mismo para un comando que ya existe.
    pub fn preparar(&self, c: &mut Command) {
        c.env("HOME", &self.rutas.casa).env("PATH", &self.path);
        match &self.rutas.xdg_datos {
            Some(v) => c.env("XDG_DATA_HOME", v),
            None => c.env_remove("XDG_DATA_HOME"),
        };
        match &self.rutas.xdg_config {
            Some(v) => c.env("XDG_CONFIG_HOME", v),
            None => c.env_remove("XDG_CONFIG_HOME"),
        };
    }

    pub fn hay_fuse2(&self) -> bool {
        self.fuse2.unwrap_or_else(|| {
            [
                "/usr/lib/libfuse.so.2",
                "/usr/lib64/libfuse.so.2",
                "/lib/x86_64-linux-gnu/libfuse.so.2",
                "/usr/lib/x86_64-linux-gnu/libfuse.so.2",
            ]
            .iter()
            .any(|p| Path::new(p).exists())
        })
    }

    /// Variables extra para abrir un AppImage: sin FUSE 2 no monta, así que se descomprime y corre igual.
    pub fn entorno_appimage(&self) -> Vec<(String, String)> {
        if self.hay_fuse2() {
            vec![]
        } else {
            vec![("APPIMAGE_EXTRACT_AND_RUN".into(), "1".into())]
        }
    }

    /// Abre un programa aparte (sigue vivo aunque se cierre esta ventana). En las pruebas solo lo anota.
    pub fn lanzar(
        &self,
        args: &[String],
        entorno: &[(String, String)],
        carpeta: Option<&Path>,
    ) -> Result<(), crate::Error> {
        if let Some(l) = &self.lanzados {
            if let Ok(mut l) = l.lock() {
                l.push(args.to_vec());
            }
            return Ok(());
        }
        let Some((exe, resto)) = args.split_first() else {
            return Err(crate::Error::Mensaje(self.tr("no_se_pudo_abrir", &[])));
        };
        let mut c = self.comando(exe);
        c.args(resto)
            .envs(entorno.iter().map(|(k, v)| (k, v)))
            .current_dir(carpeta.unwrap_or(&self.rutas.casa))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        nueva_sesion(&mut c);
        match c.spawn() {
            Ok(mut hijo) => {
                // Se recoge solo, sin esperar (que no quede zombi si termina antes que esta ventana).
                std::thread::spawn(move || {
                    let _ = hijo.wait();
                });
                Ok(())
            }
            Err(_) => Err(crate::Error::Mensaje(self.tr("no_se_pudo_abrir", &[]))),
        }
    }

    /// Lo que se «lanzó» en una prueba.
    pub fn lanzados_ahora(&self) -> Vec<Vec<String>> {
        self.lanzados
            .as_ref()
            .and_then(|l| l.lock().ok().map(|l| l.clone()))
            .unwrap_or_default()
    }
}
