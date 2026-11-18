//! Correr un programa con límite de tiempo (`subprocess.run(..., timeout=)` de Python).

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Salida {
    pub codigo: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Corre `programa` y espera hasta `limite`. `None` si no existe, no arrancó o tardó demasiado (se mata).
pub fn con_limite(programa: &str, args: &[&str], limite: Duration) -> Option<Salida> {
    let mut hijo = Command::new(programa)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    // Se lee en hilos aparte: si no, una salida larga llena la tubería y el programa se queda esperando.
    let leer = |mut t: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            let mut b = Vec::new();
            let _ = t.read_to_end(&mut b);
            s.push_str(&String::from_utf8_lossy(&b));
            s
        })
    };
    let h_out = leer(Box::new(hijo.stdout.take()?));
    let h_err = leer(Box::new(hijo.stderr.take()?));
    let inicio = Instant::now();
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(e)) => break e,
            Ok(None) if inicio.elapsed() < limite => std::thread::sleep(Duration::from_millis(5)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                return None;
            }
        }
    };
    Some(Salida {
        codigo: estado.code().unwrap_or(-1),
        stdout: h_out.join().unwrap_or_default(),
        stderr: h_err.join().unwrap_or_default(),
    })
}

/// Si el programa existe en el PATH (`shutil.which`).
pub fn existe(programa: &str) -> bool {
    if programa.contains('/') {
        return std::path::Path::new(programa).is_file();
    }
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(programa).is_file()))
}
