//! Correr programas externos (flatpak, vortex, vortex-android, curl) con límite de tiempo, sin shell
//! y sin que se queden colgados.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Lo que dejó un programa que terminó.
#[derive(Debug, Clone)]
pub struct Salida {
    /// `None` si lo mató una señal.
    pub codigo: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Salida {
    pub fn bien(&self) -> bool {
        self.codigo == Some(0)
    }
}

#[derive(Debug)]
pub enum Fallo {
    /// No se pudo ni lanzar (no existe, sin permiso…).
    NoArranca(std::io::Error),
    /// Pasó el límite de tiempo (se mató).
    Tiempo,
}

/// El entorno con que se lanza un programa: se parte del de este proceso, con estos cambios.
/// Así las pruebas no tocan variables globales (que en Rust 2024 no se pueden tocar sin `unsafe`).
#[derive(Debug, Clone, Default)]
pub struct Entorno {
    pub cambios: Vec<(OsString, OsString)>,
    pub quitar: Vec<OsString>,
}

impl Entorno {
    pub fn con(mut self, k: impl Into<OsString>, v: impl Into<OsString>) -> Self {
        self.cambios.push((k.into(), v.into()));
        self
    }

    pub fn aplicar(&self, cmd: &mut Command) {
        for k in &self.quitar {
            cmd.env_remove(k);
        }
        for (k, v) in &self.cambios {
            cmd.env(k, v);
        }
    }

    /// El valor de una variable tal como la vería un programa hijo.
    pub fn valor(&self, clave: &str) -> Option<OsString> {
        if let Some((_, v)) = self.cambios.iter().rev().find(|(k, _)| k == clave) {
            return Some(v.clone());
        }
        if self.quitar.iter().any(|k| k == clave) {
            return None;
        }
        std::env::var_os(clave)
    }

    /// Dónde está un programa: una ruta tal cual si trae «/», o lo que encuentre en el PATH.
    pub fn buscar(&self, programa: &str) -> Option<PathBuf> {
        if programa.contains('/') {
            let p = PathBuf::from(programa);
            return p.is_file().then_some(p);
        }
        let path = self.valor("PATH")?;
        std::env::split_paths(&path)
            .map(|d| d.join(programa))
            .find(|p| p.is_file())
    }
}

/// Corre `cmd` y espera como mucho `limite`. `entrada` va a su stdin (si no, stdin vacío).
pub fn correr(mut cmd: Command, entrada: Option<&[u8]>, limite: Duration) -> Result<Salida, Fallo> {
    cmd.stdin(if entrada.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    let mut hijo = cmd.spawn().map_err(Fallo::NoArranca)?;
    if let (Some(datos), Some(mut stdin)) = (entrada, hijo.stdin.take()) {
        let datos = datos.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&datos);
        });
    }
    let leer = |tubo: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            if let Some(mut t) = tubo {
                let _ = t.read_to_end(&mut v);
            }
            v
        })
    };
    let h_out = leer(hijo.stdout.take().map(|t| Box::new(t) as _));
    let h_err = leer(hijo.stderr.take().map(|t| Box::new(t) as _));
    let fin = Instant::now() + limite;
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(e)) => break Some(e),
            Ok(None) if Instant::now() >= fin => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(e) => return Err(Fallo::NoArranca(e)),
        }
    };
    let Some(estado) = estado else {
        let _ = hijo.kill();
        let _ = hijo.wait();
        return Err(Fallo::Tiempo);
    };
    let texto = |h: std::thread::JoinHandle<Vec<u8>>| {
        String::from_utf8_lossy(&h.join().unwrap_or_default()).into_owned()
    };
    Ok(Salida {
        codigo: estado.code(),
        stdout: texto(h_out),
        stderr: texto(h_err),
    })
}
