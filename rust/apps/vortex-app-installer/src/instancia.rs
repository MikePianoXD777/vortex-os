//! Una sola ventana por persona: si ya hay una abierta, la nueva le pasa el archivo y sale.
//!
//! Es el mismo socket que usa la de Python con `QLocalServer`: `<carpeta temporal>/vortex-app-installer-<uid>`
//! y el mismo mensaje: una línea `abrir<TAB>ruta` (la ruta puede ir vacía: solo mostrar la ventana).

use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Lo que le piden a la ventana que ya está abierta: mostrarse y, si hay ruta, abrir ese archivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mensaje {
    pub archivo: String,
}

impl Mensaje {
    fn a_linea(&self) -> String {
        format!("abrir\t{}\n", self.archivo)
    }

    /// Una línea recibida (`abrir<TAB>ruta`); lo demás se ignora, como en Python.
    pub fn de_linea(l: &str) -> Option<Mensaje> {
        let (orden, _, archivo) = l.partition_once('\t');
        (orden == "abrir").then(|| Mensaje {
            archivo: archivo.to_string(),
        })
    }
}

trait Partir {
    fn partition_once(&self, c: char) -> (&str, bool, &str);
}

impl Partir for str {
    fn partition_once(&self, c: char) -> (&str, bool, &str) {
        match self.split_once(c) {
            Some((a, b)) => (a, true, b),
            None => (self, false, ""),
        }
    }
}

/// `vortex-app-installer-<uid>`, como `f"{APP_ID}-{os.getuid()}"`.
pub fn nombre() -> String {
    let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    format!("{}-{uid}", crate::APP_ID)
}

/// Donde QLocalServer pone el socket (`QDir::tempPath()`: `$TMPDIR` o `/tmp`).
pub fn ruta(nombre: &str) -> PathBuf {
    std::env::temp_dir().join(nombre)
}

/// Si hay una abierta, le manda el mensaje y devuelve `true`.
pub fn avisar_a_la_abierta(ruta: &Path, mensaje: &Mensaje) -> bool {
    let Ok(mut s) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    s.write_all(mensaje.a_linea().as_bytes()).is_ok()
}

/// El socket de la ventana abierta. `revisar()` da los mensajes que llegaron desde la última vez.
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
        std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o700))?;
        Ok(Escucha {
            ruta: ruta.to_path_buf(),
            socket,
        })
    }

    pub fn revisar(&self) -> Vec<Mensaje> {
        let mut mensajes = Vec::new();
        while let Ok((mut c, _)) = self.socket.accept() {
            let _ = c.set_nonblocking(false);
            let _ = c.set_read_timeout(Some(Duration::from_millis(200)));
            let mut texto = Vec::new();
            let _ = (&mut c).take(1 << 16).read_to_end(&mut texto);
            for l in String::from_utf8_lossy(&texto).lines() {
                mensajes.extend(Mensaje::de_linea(l));
            }
        }
        mensajes
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.ruta);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn pasa_el_archivo_a_la_abierta() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path().join("sock");
        assert!(!avisar_a_la_abierta(&r, &Mensaje { archivo: "x".into() }));
        let e = Escucha::new(&r).unwrap();
        assert!(avisar_a_la_abierta(&r, &Mensaje { archivo: "/a/b c.zip".into() }));
        assert!(avisar_a_la_abierta(&r, &Mensaje { archivo: String::new() }));
        std::thread::sleep(Duration::from_millis(50));
        let m = e.revisar();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].archivo, "/a/b c.zip");
        assert_eq!(m[1].archivo, "");
        assert_eq!(Mensaje::de_linea("otra"), None);
        drop(e);
        assert!(!r.exists());
    }
}
