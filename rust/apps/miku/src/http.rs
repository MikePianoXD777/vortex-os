//! HTTP con `curl` (siempre está en el sistema, y habla HTTPS sin traer una crate de TLS). Las peticiones
//! se pueden leer en flujo (la respuesta de la IA llega en trozos) y se pueden cortar desde otro hilo.
//! El cuerpo va por la entrada estándar de curl: nunca aparece en la lista de procesos.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Respuesta {
    pub estado: u16,
    pub cabeceras: Vec<(String, String)>,
    lector: BufReader<ChildStdout>,
    hijo: Child,
    vivo: Arc<AtomicBool>,
}

/// Corta una respuesta desde otro hilo (cancelar): matar a curl despierta al que lee.
#[derive(Clone)]
pub struct Cortador {
    pid: u32,
    vivo: Arc<AtomicBool>,
}

impl Cortador {
    pub fn cortar(&self) {
        if self.vivo.load(Ordering::SeqCst) {
            crate::sistema::matar(self.pid, libc::SIGTERM);
        }
    }
}

impl Respuesta {
    pub fn cortador(&self) -> Cortador {
        Cortador { pid: self.hijo.id(), vivo: self.vivo.clone() }
    }

    /// Una línea (sin el salto), `None` al terminar la respuesta.
    pub fn linea(&mut self) -> Option<Vec<u8>> {
        let mut v = Vec::new();
        match self.lector.read_until(b'\n', &mut v) {
            Ok(0) | Err(_) => None,
            Ok(_) => {
                while matches!(v.last(), Some(b'\n' | b'\r')) {
                    v.pop();
                }
                Some(v)
            }
        }
    }

    pub fn leer_todo(&mut self) -> Vec<u8> {
        let mut v = Vec::new();
        let _ = self.lector.read_to_end(&mut v);
        v
    }

    /// ¿Se cortó antes de terminar (curl salió con error, por ejemplo por pasar el tiempo sin datos)?
    pub fn fallo(&mut self) -> Option<i32> {
        match self.hijo.try_wait() {
            Ok(Some(e)) if !e.success() => e.code().or(Some(-1)),
            _ => None,
        }
    }
}

impl Drop for Respuesta {
    fn drop(&mut self) {
        self.vivo.store(false, Ordering::SeqCst);
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

/// Hace la petición y espera las cabeceras. `inactivo`: segundos sin recibir nada antes de darla por caída.
/// `Err` = no se pudo conectar (el texto de curl).
pub fn pedir(
    metodo: &str,
    url: &str,
    cabeceras: &[(&str, String)],
    cuerpo: Option<Vec<u8>>,
    inactivo: u64,
) -> Result<Respuesta, String> {
    let mut c = Command::new("curl");
    c.args(["-sS", "-N", "-D", "-", "--connect-timeout", "15", "--speed-limit", "1", "--speed-time"])
        .arg(inactivo.max(1).to_string())
        .args(["-X", metodo, "-H", "Expect:", "-H", "User-Agent: vortex-miku"]);
    for (k, v) in cabeceras {
        c.arg("-H").arg(format!("{k}: {v}"));
    }
    if cuerpo.is_some() {
        c.args(["--data-binary", "@-"]);
    }
    c.arg(url).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut hijo = c.spawn().map_err(|e| format!("no pude correr curl: {e}"))?;
    if let Some(mut e) = hijo.stdin.take() {
        std::thread::spawn(move || {
            if let Some(cuerpo) = cuerpo {
                let _ = e.write_all(&cuerpo);
            }
        });
    }
    let mut lector = BufReader::new(hijo.stdout.take().expect("stdout de curl"));
    let (mut estado, mut cab) = (0u16, Vec::new());
    loop {
        let mut l = String::new();
        let n = lector.read_line(&mut l).unwrap_or(0);
        let l = l.trim_end().to_string();
        if n == 0 {
            let mut err = String::new();
            if let Some(mut e) = hijo.stderr.take() {
                let _ = e.read_to_string(&mut err);
            }
            let _ = hijo.wait();
            return Err(err.trim().to_string());
        }
        if l.is_empty() {
            // Fin de un bloque de cabeceras: si era un 1xx o el «Connection established», viene otro.
            if estado >= 200 {
                break;
            }
            continue;
        }
        if l.starts_with("HTTP/") {
            estado = l.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0);
            cab.clear();
        } else if let Some((k, v)) = l.split_once(':') {
            cab.push((k.trim().to_lowercase(), v.trim().to_string()));
        }
    }
    Ok(Respuesta { estado, cabeceras: cab, lector, hijo, vivo: Arc::new(AtomicBool::new(true)) })
}

/// POST de JSON y el cuerpo entero de la respuesta.
pub fn json_a_json(url: &str, cabeceras: &[(&str, String)], datos: &serde_json::Value, inactivo: u64) -> Result<(u16, String), String> {
    let mut h: Vec<(&str, String)> = vec![("Content-Type", "application/json".into())];
    h.extend(cabeceras.iter().cloned());
    let mut r = pedir("POST", url, &h, Some(datos.to_string().into_bytes()), inactivo)?;
    let cuerpo = String::from_utf8_lossy(&r.leer_todo()).into_owned();
    Ok((r.estado, cuerpo))
}

pub fn obtener(url: &str, inactivo: u64) -> Result<(u16, String), String> {
    let mut r = pedir("GET", url, &[], None, inactivo)?;
    let cuerpo = String::from_utf8_lossy(&r.leer_todo()).into_owned();
    Ok((r.estado, cuerpo))
}
