//! Correr programas de afuera (`bsdtar`, `flatpak`, `curl`…) sin que se cuelguen: sin terminal, con tiempo
//! límite y con la salida recogida en hilos aparte (así una salida larga nunca bloquea al programa).

use crate::util::{matar_grupo, nueva_sesion};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub struct Salida {
    /// `None` si lo mató una señal.
    pub codigo: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// Se pasó del tiempo y se detuvo.
    pub agotado: bool,
}

impl Salida {
    pub fn ok(&self) -> bool {
        !self.agotado && self.codigo == Some(0)
    }
    pub fn texto_out(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn texto_err(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

/// Corre `cmd` en su propia sesión, con `entrada` por stdin (o nada), y recoge todo. Si pasa `limite`,
/// detiene todo su grupo de procesos.
pub fn ejecutar(
    mut cmd: Command,
    entrada: Option<&[u8]>,
    limite: Option<Duration>,
) -> std::io::Result<Salida> {
    cmd.stdin(if entrada.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    nueva_sesion(&mut cmd);
    // Un programa recién escrito puede dar «Text file busy» si otro hilo hizo fork justo en ese momento:
    // se reintenta unos instantes.
    let mut intentos = 0;
    let mut hijo = loop {
        match cmd.spawn() {
            Ok(h) => break h,
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && intentos < 50 => {
                intentos += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(e),
        }
    };
    let pid = hijo.id();
    let mut out = hijo.stdout.take();
    let mut err = hijo.stderr.take();
    let mut sin = hijo.stdin.take();
    let entrada = entrada.map(<[u8]>::to_vec);
    let inicio = Instant::now();
    let mut agotado = false;
    let (so, se) = std::thread::scope(|s| {
        let escritor = s.spawn(move || {
            if let (Some(mut i), Some(datos)) = (sin.take(), entrada) {
                let _ = i.write_all(&datos); // si el programa ya terminó, se pierde y no importa
            }
        });
        let lector_out = s.spawn(move || {
            let mut v = Vec::new();
            if let Some(o) = out.as_mut() {
                let _ = o.read_to_end(&mut v);
            }
            v
        });
        let lector_err = s.spawn(move || {
            let mut v = Vec::new();
            if let Some(e) = err.as_mut() {
                let _ = e.read_to_end(&mut v);
            }
            v
        });
        loop {
            match hijo.try_wait() {
                Ok(Some(_)) | Err(_) => break,
                Ok(None) => {}
            }
            if limite.is_some_and(|l| inicio.elapsed() > l) {
                agotado = true;
                matar_grupo(pid, libc::SIGKILL);
                let _ = hijo.kill();
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = hijo.wait();
        // Si quedó algún nieto con la tubería abierta, los lectores esperan hasta que se cierre: se
        // le manda la señal al grupo para que no se queden colgados.
        if agotado {
            matar_grupo(pid, libc::SIGKILL);
        }
        let _ = escritor.join();
        (
            lector_out.join().unwrap_or_default(),
            lector_err.join().unwrap_or_default(),
        )
    });
    let codigo = hijo.wait().ok().and_then(|e| e.code());
    Ok(Salida {
        codigo,
        stdout: so,
        stderr: se,
        agotado,
    })
}
