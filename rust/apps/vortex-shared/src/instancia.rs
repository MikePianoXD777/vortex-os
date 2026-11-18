//! Una sola ventana por persona: si ya hay una abierta, la nueva le pasa los archivos y sale.
//!
//! Es el mismo socket que usa la de Python con `QLocalServer`: `<carpeta temporal>/vortex-shared-<uid>`, y el
//! mismo mensaje (una línea con una lista JSON de rutas, que puede ir vacía). Así la de Rust y la de Python
//! tampoco se abren dos veces.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// `vortex-shared-<uid>`, como `f"{APP_ID}-{os.getuid()}"`.
pub fn nombre() -> String {
    format!("{}-{}", crate::APP_ID, crate::uid())
}

/// Donde QLocalServer pone el socket (`QDir::tempPath()`: `$TMPDIR` o `/tmp`).
pub fn ruta(nombre: &str) -> PathBuf {
    std::env::temp_dir().join(nombre)
}

/// Si hay una abierta, le pasa los archivos y devuelve `true`.
pub fn avisar_a_la_abierta(ruta: &Path, archivos: &[String]) -> bool {
    let Ok(mut s) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    let mut linea = serde_json::json!(archivos).to_string();
    linea.push('\n');
    let _ = s.write_all(linea.as_bytes());
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

    /// Espera (sin gastar nada) a que otra copia mande archivos: las rutas que traía (puede ser ninguna).
    /// `None` si el socket se rompió.
    pub fn esperar(&self) -> Option<Vec<String>> {
        let (mut c, _) = self.socket.accept().ok()?;
        let _ = c.set_read_timeout(Some(Duration::from_millis(500)));
        let mut texto = Vec::new();
        let _ = (&mut c).take(1 << 20).read_to_end(&mut texto);
        let primera = String::from_utf8_lossy(&texto);
        let lista: Vec<serde_json::Value> =
            serde_json::from_str(primera.lines().next().unwrap_or("")).unwrap_or_default();
        Some(
            lista
                .into_iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        )
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.ruta);
    }
}
