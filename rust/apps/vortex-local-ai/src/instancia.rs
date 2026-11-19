//! Una sola ventana por persona: si ya hay una abierta, la nueva le pide que se muestre y sale.
//!
//! Es el mismo socket que usa la de Python con `QLocalServer`: `<carpeta temporal>/vortex-local-ai-<uid>`, y
//! el mismo mensaje («mostrar»). Así la de Rust y la de Python tampoco se abren dos veces.

use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `vortex-local-ai-<uid>`, como `f"{APP_ID}-{os.getuid()}"`.
pub fn nombre() -> String {
    format!("{}-{}", crate::APP_ID, uid())
}

fn uid() -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0)
}

/// Donde QLocalServer pone el socket (QDir::tempPath(): $TMPDIR o /tmp).
pub fn ruta(nombre: &str) -> PathBuf {
    std::env::temp_dir().join(nombre)
}

/// Si hay una abierta, le dice «mostrar» y devuelve `true`.
pub fn avisar_a_la_abierta(ruta: &Path) -> bool {
    let Ok(mut s) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    s.write_all(b"mostrar\n").is_ok()
}

/// El socket de la ventana abierta. `revisar()` dice cuántas veces pidieron «mostrar» desde la última vez.
pub struct Escucha {
    ruta: PathBuf,
    socket: UnixListener,
}

impl Escucha {
    /// Quita un socket viejo (de una ventana que se cerró mal) y escucha. Solo tu usuario puede conectarse.
    pub fn new(ruta: &Path) -> std::io::Result<Self> {
        let _ = std::fs::remove_file(ruta);
        let socket = UnixListener::bind(ruta)?;
        socket.set_nonblocking(true)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(Escucha {
            ruta: ruta.to_path_buf(),
            socket,
        })
    }

    pub fn revisar(&self) -> usize {
        let mut veces = 0;
        while let Ok((mut c, _)) = self.socket.accept() {
            let _ = c.set_nonblocking(false);
            let _ = c.set_read_timeout(Some(Duration::from_millis(200)));
            let mut buf = [0u8; 64];
            let _ = c.read(&mut buf);
            veces += 1;
        }
        veces
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.ruta);
    }
}
