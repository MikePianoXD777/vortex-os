//! Las apps propias de Vortexine's que no vienen de fábrica (Vortexine's Camera y Vortexine's Runner).
//!
//! No son de Flathub: las instala, quita y abre el comando `vortex` («vortex camara instalar»…), para tu
//! usuario y sin contraseña. La tienda solo pregunta si están (su carpeta en `~/.local/share`) y llama al
//! comando en un hilo.
//!
//! Para pruebas: `VORTEX_SHOP_VORTEX=/ruta/a/un/vortex` (falso) cambia el comando que se llama.

use crate::errores::{Error, Resultado};
use crate::proceso::{self, Entorno, Fallo};
use crate::url_de_archivo;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

pub struct AppPropia {
    pub id: &'static str,
    pub nombre: &'static str,
    pub resumen: &'static str,
    pub subcomando: &'static str,
    pub carpeta: &'static str,
    pub paquete: &'static str,
    /// Dentro de la carpeta `qml/`.
    pub icono: &'static str,
}

pub const APPS: [AppPropia; 2] = [
    AppPropia {
        id: "vortex-camera",
        nombre: "Vortexine's Camera",
        resumen: "Tus cámaras Imou en mosaico, como Imou Life en la PC: en vivo, grabaciones de la microSD, fotos, video y las que giran. Sin nube y sin límites.",
        subcomando: "camara",
        carpeta: "vortex-camera",
        paquete: "vortex_camera",
        icono: "propias/vortex-camera.svg",
    },
    AppPropia {
        id: "vortex-runner",
        nombre: "Vortexine's Runner",
        resumen: "Juega y usa programas de Windows (.exe), como Heroic: cada uno en su propio espacio, con el Proton de CachyOS o GE-Proton. Arrastra un .exe y listo.",
        subcomando: "runner",
        carpeta: "vortex-runner",
        paquete: "vortex_runner",
        icono: "propias/vortex-runner.svg",
    },
];

pub struct Propias {
    entorno: Entorno,
    /// La carpeta `qml/` (de ahí salen los íconos).
    qml: PathBuf,
}

impl Propias {
    pub fn nuevo(qml: PathBuf, entorno: Entorno) -> Self {
        Propias { entorno, qml }
    }

    fn datos(&self) -> PathBuf {
        match self.entorno.valor("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            Some(d) => PathBuf::from(d),
            None => PathBuf::from(self.entorno.valor("HOME").unwrap_or_default()).join(".local/share"),
        }
    }

    fn vortex(&self) -> String {
        self.entorno
            .valor("VORTEX_SHOP_VORTEX")
            .filter(|v| !v.is_empty())
            .map_or_else(|| "vortex".into(), |v| v.to_string_lossy().into_owned())
    }

    pub fn app(&self, app_id: &str) -> Resultado<&'static AppPropia> {
        APPS.iter()
            .find(|a| a.id == app_id)
            .ok_or_else(|| Error::Propia("Esa app no es de Vortexine's.".into()))
    }

    pub fn instalada(&self, app_id: &str) -> Resultado<bool> {
        let a = self.app(app_id)?;
        Ok(self
            .datos()
            .join(a.carpeta)
            .join(a.paquete)
            .join("__init__.py")
            .is_file())
    }

    /// Para QML: cada app con su ícono (URL de archivo) y si ya está.
    pub fn lista(&self) -> Vec<Value> {
        APPS.iter()
            .map(|a| {
                json!({"id": a.id, "nombre": a.nombre, "resumen": a.resumen, "desarrollador": "Vortexine's",
                       "icono": url_de_archivo(&self.qml.join(a.icono)),
                       "instalada": self.instalada(a.id).unwrap_or(false)})
            })
            .collect()
    }

    fn correr(&self, app_id: &str, accion: &str) -> Resultado<()> {
        let a = self.app(app_id)?;
        let mut cmd = Command::new(self.vortex());
        cmd.args([a.subcomando, accion]);
        self.entorno.aplicar(&mut cmd);
        let r = match proceso::correr(cmd, None, Duration::from_secs(300)) {
            Err(Fallo::NoArranca(e)) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::Propia("No encuentro el comando vortex.".into()));
            }
            Err(Fallo::NoArranca(e)) => {
                return Err(Error::Propia(format!("No se pudo llamar a vortex: {e}")));
            }
            Err(Fallo::Tiempo) => return Err(Error::Propia(format!("{} tardó demasiado.", a.nombre))),
            Ok(r) => r,
        };
        if !r.bien() {
            let ultima = r.stderr.trim().lines().next_back().unwrap_or("");
            let detalle = ultima
                .strip_prefix(&format!("vortex {}: ", a.subcomando))
                .unwrap_or(ultima);
            let verbo = if accion == "instalar" {
                "instalar"
            } else {
                "quitar"
            };
            let sufijo = if detalle.is_empty() {
                ".".to_string()
            } else {
                format!(": {detalle}")
            };
            return Err(Error::Propia(format!("No se pudo {verbo} {}{sufijo}", a.nombre)));
        }
        Ok(())
    }

    pub fn instalar(&self, app_id: &str) -> Resultado<()> {
        self.correr(app_id, "instalar")
    }

    pub fn quitar(&self, app_id: &str) -> Resultado<()> {
        self.correr(app_id, "quitar")
    }

    pub fn abrir(&self, app_id: &str) -> Resultado<()> {
        let a = self.app(app_id)?;
        let mut cmd = Command::new(self.vortex());
        cmd.args([a.subcomando, "abrir"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        self.entorno.aplicar(&mut cmd);
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut hijo = cmd
            .spawn()
            .map_err(|_| Error::Propia(format!("No se pudo abrir {}.", a.nombre)))?;
        std::thread::spawn(move || {
            let _ = hijo.wait();
        });
        Ok(())
    }
}
