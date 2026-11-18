//! Una sola ventana por persona: si ya hay una abierta, la nueva le pide que se muestre y sale.
//!
//! Es el mismo socket que usa la de Python con `QLocalServer`: `<carpeta temporal>/vortex-connect-<uid>`.
//! Cualquier conexión significa «muéstrate». Así la de Rust y la de Python tampoco se abren dos veces.

#[cfg(unix)]
pub use unix::*;

#[cfg(unix)]
mod unix {
    use std::io::{Read, Write};
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// `vortex-connect-<uid>`, como `f"{APP_ID}-{os.getuid()}"`.
    pub fn nombre() -> String {
        let uid = std::fs::metadata("/proc/self").map_or(0, |m| m.uid());
        format!("{}-{uid}", crate::APP_ID)
    }

    /// Donde QLocalServer pone el socket (`QDir::tempPath()`: `$TMPDIR` o `/tmp`).
    pub fn ruta(nombre: &str) -> PathBuf {
        std::env::temp_dir().join(nombre)
    }

    /// Si hay una abierta, le pide que se muestre y devuelve `true`.
    pub fn avisar_a_la_abierta(ruta: &Path) -> bool {
        let Ok(mut s) = UnixStream::connect(ruta) else {
            return false;
        };
        let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
        let _ = s.write_all(b"mostrar\n");
        true
    }

    /// El socket de la ventana abierta.
    pub struct Escucha {
        ruta: PathBuf,
        socket: UnixListener,
    }

    impl Escucha {
        /// Quita un socket viejo (de una ventana que se cerró mal) y escucha. Solo tu usuario puede conectarse.
        pub fn new(ruta: &Path) -> std::io::Result<Self> {
            let _ = std::fs::remove_file(ruta);
            let socket = UnixListener::bind(ruta)?;
            std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o700))?;
            Ok(Escucha {
                ruta: ruta.to_path_buf(),
                socket,
            })
        }

        /// Espera (sin gastar nada) a que otra copia pida mostrar la ventana. `false` si el socket se rompió.
        pub fn esperar(&self) -> bool {
            match self.socket.accept() {
                Ok((mut c, _)) => {
                    let _ = c.set_read_timeout(Some(Duration::from_millis(200)));
                    let mut basura = [0u8; 256];
                    let _ = c.read(&mut basura);
                    true
                }
                Err(_) => false,
            }
        }
    }

    impl Drop for Escucha {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.ruta);
        }
    }
}

#[cfg(not(unix))]
pub use otros::*;

#[cfg(not(unix))]
mod otros {
    use std::path::{Path, PathBuf};

    pub fn nombre() -> String {
        crate::APP_ID.to_string()
    }

    pub fn ruta(nombre: &str) -> PathBuf {
        std::env::temp_dir().join(nombre)
    }

    pub fn avisar_a_la_abierta(_ruta: &Path) -> bool {
        false
    }

    pub struct Escucha;

    impl Escucha {
        pub fn new(_ruta: &Path) -> std::io::Result<Self> {
            Ok(Escucha)
        }

        pub fn esperar(&self) -> bool {
            false
        }
    }
}
