//! Hablar con el sistema: correr programas con tiempo límite y abrir páginas. Todo lo demás del crate pasa
//! por aquí, así las pruebas cambian el `PATH` y ningún programa real (nmcli, warp-cli, systemctl…) se toca.

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Lo que dejó un programa. `codigo`: 127 si no existe o no arranca, 124 si pasó el tiempo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Salida {
    pub codigo: i32,
    pub salida: String,
    pub error: String,
}

impl Salida {
    pub fn bien(&self) -> bool {
        self.codigo == 0
    }

    /// stdout y stderr juntos, sin espacios de sobra (como `(r.stdout + r.stderr).strip()` de Python).
    pub fn texto(&self) -> String {
        format!("{}{}", self.salida, self.error).trim().to_string()
    }
}

/// Cómo corre programas. `path`: el `PATH` donde se buscan (y que reciben los hijos); `None` = el real.
#[derive(Debug, Clone, Default)]
pub struct Sistema {
    pub path: Option<OsString>,
}

impl Sistema {
    pub fn real() -> Self {
        Self::default()
    }

    /// Con otro `PATH` (las pruebas: la carpeta de programas falsos primero).
    pub fn con_path(path: impl Into<OsString>) -> Self {
        Sistema {
            path: Some(path.into()),
        }
    }

    fn path_efectivo(&self) -> OsString {
        self.path
            .clone()
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default()
    }

    /// La ruta completa del programa, como `shutil.which`.
    pub fn buscar(&self, nombre: &str) -> Option<PathBuf> {
        if nombre.contains('/') {
            return es_ejecutable(Path::new(nombre)).then(|| PathBuf::from(nombre));
        }
        std::env::split_paths(&self.path_efectivo())
            .map(|d| d.join(nombre))
            .find(|r| es_ejecutable(r))
    }

    /// Corre `orden[0]` con el resto de argumentos. Nunca pasa por un shell (un nombre de red raro no se
    /// interpreta) y no lee del teclado.
    pub fn correr(&self, orden: &[&str], limite: Duration) -> Salida {
        let Some((programa, args)) = orden.split_first() else {
            return Salida {
                codigo: 127,
                ..Salida::default()
            };
        };
        let ruta = self.buscar(programa).unwrap_or_else(|| PathBuf::from(programa));
        let mut cmd = Command::new(ruta);
        cmd.args(args)
            .env("PATH", self.path_efectivo())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        sin_ventana(&mut cmd);
        let mut hijo = match cmd.spawn() {
            Ok(h) => h,
            Err(e) => {
                return Salida {
                    codigo: 127,
                    salida: String::new(),
                    error: e.to_string(),
                };
            }
        };
        let leer = |f: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut b = Vec::new();
                if let Some(mut f) = f {
                    let _ = f.read_to_end(&mut b);
                }
                String::from_utf8_lossy(&b).into_owned()
            })
        };
        let h_out = leer(hijo.stdout.take().map(|s| Box::new(s) as _));
        let h_err = leer(hijo.stderr.take().map(|s| Box::new(s) as _));
        let fin = Instant::now() + limite;
        let codigo = loop {
            match hijo.try_wait() {
                Ok(Some(e)) => break e.code().unwrap_or(1),
                Ok(None) if Instant::now() >= fin => {
                    let _ = hijo.kill();
                    let _ = hijo.wait();
                    break 124;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(5)),
                Err(_) => break 127,
            }
        };
        Salida {
            codigo,
            salida: h_out.join().unwrap_or_default(),
            error: h_err.join().unwrap_or_default(),
        }
    }

    /// Lo que cada comando pidió y su código, sin quedarse con la salida.
    pub fn codigo(&self, orden: &[&str], limite: Duration) -> i32 {
        self.correr(orden, limite).codigo
    }

    /// Abre una página con el navegador de la persona (`BROWSER`, `xdg-open`; en Windows, `start`).
    pub fn abrir_url(&self, url: &str) {
        if !(url.starts_with("http://") || url.starts_with("https://")) {
            return;
        }
        let navegador = std::env::var("BROWSER").unwrap_or_default();
        let navegador = navegador.split(':').find(|n| !n.trim().is_empty());
        let mut cmd = if let Some(n) = navegador {
            let mut partes = n.split_whitespace();
            let Some(p) = partes.next() else { return };
            let mut c = Command::new(self.buscar(p).unwrap_or_else(|| PathBuf::from(p)));
            c.args(partes).arg(url);
            c
        } else if crate::ES_WINDOWS {
            let mut c = Command::new("cmd");
            c.args(["/c", "start", "", url]);
            c
        } else if let Some(x) = self.buscar("xdg-open") {
            let mut c = Command::new(x);
            c.arg(url);
            c
        } else {
            return;
        };
        cmd.env("PATH", self.path_efectivo())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        sin_ventana(&mut cmd);
        if let Ok(mut h) = cmd.spawn() {
            // Que no quede un zombi esperando: se recoge en un hilo.
            std::thread::spawn(move || {
                let _ = h.wait();
            });
        }
    }
}

#[cfg(windows)]
fn sin_ventana(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
fn sin_ventana(_cmd: &mut Command) {}

#[cfg(unix)]
fn es_ejecutable(r: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    r.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn es_ejecutable(r: &Path) -> bool {
    r.is_file()
}
