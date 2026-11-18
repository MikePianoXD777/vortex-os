//! Lanzar programas (busctl, vortex, quickshell, nvidia-smi…): con plazo (lo que hacía `timeout N`),
//! sin esperar (lo que hacía `&`) y recogiendo a los hijos para que no queden zombis.

use crate::sis;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct Lanzador {
    /// HYPRLAND_INSTANCE_SIGNATURE de la instancia que se vigila: los programas lo heredan (el bash lo
    /// exportaba al buscarla).
    pub firma: String,
    hijos: Vec<Child>,
}

impl Lanzador {
    pub fn nuevo() -> Lanzador {
        Lanzador {
            firma: String::new(),
            hijos: Vec::new(),
        }
    }

    fn comando(&self, prog: &str, args: &[String]) -> Command {
        let mut c = Command::new(prog);
        c.args(args).stdin(Stdio::null()).stderr(Stdio::null());
        sis::hijo_sin_mascara(&mut c);
        if !self.firma.is_empty() {
            c.env("HYPRLAND_INSTANCE_SIGNATURE", &self.firma);
        }
        c
    }

    /// Espera al hijo (con plazo, o sin él). `true` = terminó con éxito; si se pasa del plazo, se mata.
    fn esperar_hijo(h: &mut Child, plazo: Option<Duration>) -> bool {
        let limite = plazo.map(|p| Instant::now() + p);
        let mut pausa = Duration::from_millis(1);
        loop {
            match h.try_wait() {
                Ok(Some(st)) => return st.success(),
                Ok(None) => {}
                Err(_) => return false,
            }
            if limite.is_some_and(|l| Instant::now() >= l) {
                let _ = h.kill();
                let _ = h.wait();
                return false;
            }
            std::thread::sleep(pausa);
            pausa = (pausa * 2).min(Duration::from_millis(20));
        }
    }

    /// Lanza, espera y dice si salió bien (`timeout N prog args >/dev/null 2>&1`).
    pub fn ejecutar(&mut self, prog: &str, args: &[String], plazo: Option<Duration>) -> bool {
        let Ok(mut h) = self.comando(prog, args).stdout(Stdio::null()).spawn() else {
            return false;
        };
        Self::esperar_hijo(&mut h, plazo)
    }

    /// Lanza, espera (con plazo) y devuelve lo que escribió en la salida (`None` si no pudo lanzarse).
    pub fn salida(&mut self, prog: &str, args: &[String], plazo: Duration) -> Option<Vec<u8>> {
        let mut h = self.comando(prog, args).stdout(Stdio::piped()).spawn().ok()?;
        let limite = Instant::now() + plazo;
        let mut sal = Vec::new();
        if let Some(mut o) = h.stdout.take() {
            let mut buf = [0u8; 8192];
            loop {
                let falta = limite.saturating_duration_since(Instant::now());
                if falta.is_zero() {
                    let _ = h.kill();
                    let _ = h.wait();
                    return Some(sal);
                }
                let mut fds = [sis::pollfd(o.as_raw_fd(), libc::POLLIN)];
                let ms = i32::try_from(falta.as_millis().max(1)).unwrap_or(i32::MAX);
                match sis::sondear(&mut fds, ms) {
                    Ok(0) | Err(_) => continue,
                    Ok(_) => {}
                }
                match o.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => sal.extend_from_slice(&buf[..n]),
                }
            }
        }
        Self::esperar_hijo(
            &mut h,
            Some(
                limite
                    .saturating_duration_since(Instant::now())
                    .max(Duration::from_millis(50)),
            ),
        );
        Some(sal)
    }

    /// Lanza sin esperar (`prog args &`). El hijo se recoge después con `recoger`.
    pub fn segundo_plano(&mut self, prog: &str, args: &[String]) {
        if let Ok(h) = self.comando(prog, args).stdout(Stdio::null()).spawn() {
            self.hijos.push(h);
        }
    }

    /// Como `segundo_plano`, pero con plazo (`timeout N prog args &`): un hilo lo espera y lo mata si se pasa.
    pub fn segundo_plano_con_plazo(&mut self, prog: &str, args: &[String], plazo: Duration) {
        let mut c = self.comando(prog, args);
        c.stdout(Stdio::null());
        std::thread::spawn(move || {
            if let Ok(mut h) = c.spawn() {
                Self::esperar_hijo(&mut h, Some(plazo));
            }
        });
    }

    /// Recoge a los hijos que ya terminaron.
    pub fn recoger(&mut self) {
        self.hijos.retain_mut(|h| matches!(h.try_wait(), Ok(None)));
    }

    pub fn pendientes(&self) -> usize {
        self.hijos.len()
    }
}

/// `command -v`: ¿existe y se puede ejecutar?
pub fn existe_comando(nombre: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let ok = |p: &std::path::Path| {
        std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
    };
    if nombre.contains('/') {
        return ok(std::path::Path::new(nombre));
    }
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| ok(&d.join(nombre))))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn exito_fallo_y_plazo() {
        let mut l = Lanzador::nuevo();
        assert!(l.ejecutar("true", &[], None));
        assert!(!l.ejecutar("false", &[], None));
        assert!(!l.ejecutar("/no/existe", &[], None));
        let t = Instant::now();
        assert!(!l.ejecutar("sleep", &s(&["5"]), Some(Duration::from_millis(200))));
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn salida_y_plazo() {
        let mut l = Lanzador::nuevo();
        assert_eq!(
            l.salida("echo", &s(&["hola"]), Duration::from_secs(2)).unwrap(),
            b"hola\n"
        );
        let t = Instant::now();
        let r = l
            .salida("sh", &s(&["-c", "echo a; sleep 5"]), Duration::from_millis(300))
            .unwrap();
        assert_eq!(r, b"a\n");
        assert!(t.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn segundo_plano_se_recoge() {
        let mut l = Lanzador::nuevo();
        l.segundo_plano("true", &[]);
        assert!(l.pendientes() <= 1);
        std::thread::sleep(Duration::from_millis(200));
        l.recoger();
        assert_eq!(l.pendientes(), 0);
    }

    #[test]
    fn los_hijos_no_heredan_las_senales_bloqueadas() {
        // El gestor bloquea TERM/INT/HUP/CHLD para leerlas por un descriptor; lo que lanza no debe heredarlo
        // (un `timeout` o un programa que no oye TERM se quedaría colgado).
        let _fd = sis::senales().unwrap();
        let mut l = Lanzador::nuevo();
        let sal = l
            .salida(
                "grep",
                &s(&["SigBlk", "/proc/self/status"]),
                Duration::from_secs(2),
            )
            .unwrap();
        assert!(
            String::from_utf8_lossy(&sal).contains("0000000000000000"),
            "{sal:?}"
        );
    }

    #[test]
    fn comando_existente() {
        assert!(existe_comando("sh"));
        assert!(!existe_comando("no-existe-este-programa"));
        assert!(existe_comando("/bin/sh"));
    }
}
