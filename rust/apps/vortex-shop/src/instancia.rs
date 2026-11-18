//! Una sola ventana por persona: si ya hay una tienda abierta, la nueva le pide que se muestre y sale.
//!
//! Es el mismo socket que usa la de Python con `QLocalServer`: `<carpeta temporal>/vortexines-shop-<uid>` y
//! el mismo mensaje (`mostrar`, una línea). Así la de Rust y la de Python tampoco se abren dos veces.

use std::io::{Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

pub const APP_ID: &str = "vortexines-shop";

/// `vortexines-shop-<uid>`, como `f"{APP_ID}-{os.getuid()}"`.
pub fn nombre() -> String {
    let uid = std::fs::metadata("/proc/self").map(|m| m.uid()).unwrap_or(0);
    format!("{APP_ID}-{uid}")
}

/// Donde QLocalServer pone el socket (`QDir::tempPath()`: `$TMPDIR` o `/tmp`).
pub fn ruta(nombre: &str) -> PathBuf {
    std::env::temp_dir().join(nombre)
}

/// Si hay una abierta, le pide mostrarse y devuelve `true`.
pub fn avisar_a_la_abierta(ruta: &Path) -> bool {
    let Ok(mut s) = UnixStream::connect(ruta) else {
        return false;
    };
    let _ = s.set_write_timeout(Some(Duration::from_millis(400)));
    s.write_all(b"mostrar\n").is_ok()
}

/// El socket de la ventana abierta. `revisar()` dice cuántas veces pidieron mostrarse desde la última vez.
pub struct Escucha {
    ruta: PathBuf,
    socket: UnixListener,
}

impl Escucha {
    /// Quita un socket viejo (de una ventana que se cerró mal) y escucha. Solo tu usuario puede conectarse.
    pub fn nueva(ruta: &Path) -> std::io::Result<Self> {
        let _ = std::fs::remove_file(ruta);
        let socket = UnixListener::bind(ruta)?;
        socket.set_nonblocking(true)?;
        std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o700))?;
        Ok(Escucha {
            ruta: ruta.to_path_buf(),
            socket,
        })
    }

    pub fn revisar(&self) -> usize {
        let mut pedidos = 0;
        while let Ok((mut c, _)) = self.socket.accept() {
            let _ = c.set_nonblocking(false);
            let _ = c.set_read_timeout(Some(Duration::from_millis(200)));
            let mut texto = Vec::new();
            let _ = (&mut c).take(1 << 16).read_to_end(&mut texto);
            // Como la de Python: cualquier cosa que llegue es «mostrar».
            pedidos += 1;
        }
        pedidos
    }
}

impl Drop for Escucha {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.ruta);
    }
}

/// Le pide a Hyprland que enfoque esta ventana, aunque esté en otro espacio de trabajo.
///
/// `requestActivate()` no alcanza: Hyprland trae `misc:focus_on_activate` apagado y la ignora, así que con la
/// tienda abierta en otro espacio, abrirla otra vez no hacía nada visible. Primero la sintaxis de la config
/// en Lua (Hyprland 0.56+); si falla, la de antes. Sin Hyprland no hace nada.
pub fn enfocar_en_hyprland() {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let ventana = format!("pid:{}", std::process::id());
    for orden in [
        format!("hl.dsp.focus({{ window = \"{ventana}\" }})"),
        format!("focuswindow {ventana}"),
    ] {
        match Command::new("hyprctl").args(["dispatch", &orden]).output() {
            Ok(r) if r.status.success() => return,
            Ok(_) => {}
            Err(_) => return,
        }
    }
}

/// Abre una dirección web en el navegador (`xdg-open`), sin esperarlo.
pub fn abrir_url(url: &str) {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return;
    }
    if let Ok(mut h) = Command::new("xdg-open")
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        std::thread::spawn(move || {
            let _ = h.wait();
        });
    }
}
