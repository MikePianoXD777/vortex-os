//! Lo poco que pide `libc` o el sistema: el uid, permisos de archivos, el nombre de la persona y
//! comprobar que un programa existe. Aquí vive el único `unsafe` (libc) del motor.
#![allow(unsafe_code)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn uid() -> u32 {
    // SAFETY: getuid no falla ni toca memoria.
    unsafe { libc::getuid() }
}

pub fn permisos(ruta: &Path, modo: u32) -> std::io::Result<()> {
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(modo))
}

/// Que el proceso hijo muera con nosotros (PR_SET_PDEATHSIG): para el `pre_exec` de los procesos aparte.
pub fn morir_con_el_padre() {
    // SAFETY: prctl con PR_SET_PDEATHSIG solo cambia una bandera del proceso.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
    }
}

pub fn matar(pid: u32, senal: i32) -> bool {
    // SAFETY: kill solo manda una señal.
    unsafe { libc::kill(pid as i32, senal) == 0 }
}

/// El primer nombre de la persona (el GECOS de /etc/passwd, sin llamar a nada más). Vacío si no hay.
pub fn nombre_persona() -> String {
    let uid = uid().to_string();
    let Ok(t) = std::fs::read_to_string("/etc/passwd") else {
        return String::new();
    };
    for l in t.lines() {
        let c: Vec<&str> = l.split(':').collect();
        if c.len() >= 5 && c[2] == uid {
            let gecos = c[4].split(',').next().unwrap_or("").trim();
            return gecos.split_whitespace().next().unwrap_or("").to_string();
        }
    }
    String::new()
}

/// Dónde está un programa en el PATH (como `shutil.which`).
pub fn cual(programa: &str) -> Option<PathBuf> {
    if programa.contains('/') {
        let p = PathBuf::from(programa);
        return es_ejecutable(&p).then_some(p);
    }
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|d| d.join(programa))
            .find(|p| es_ejecutable(p))
    })
}

pub fn es_ejecutable(p: &Path) -> bool {
    p.is_file()
        && std::fs::metadata(p)
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
}

/// Que el proceso hijo viva en su propia sesión (no se ata a Miku ni a su terminal): para lo que sigue
/// corriendo después (apps, instalaciones, la grabación).
pub fn nueva_sesion(cmd: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    // SAFETY: `setsid` es async-signal-safe y solo se llama entre el fork y el exec.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
}

/// Lanza algo aparte (sesión propia, sin entrada ni salida).
pub fn lanzar_aparte(programa: &str, args: &[String]) -> std::io::Result<()> {
    use std::process::{Command, Stdio};
    let mut c = Command::new(programa);
    c.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    nueva_sesion(&mut c);
    c.spawn().map(|_| ())
}

/// ¿Está vivo el proceso con ese pid?
pub fn existe(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Una sola Miku por usuario: toma el candado exclusivo del archivo (se suelta al cerrar el proceso).
/// `None` si otra ya lo tiene.
pub fn candado(ruta: &Path) -> Option<std::fs::File> {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(ruta)
        .ok()?;
    // SAFETY: flock sobre un descriptor que acabamos de abrir.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return None;
    }
    let _ = f.set_len(0);
    let _ = write!(f, "{}", std::process::id());
    Some(f)
}

#[cfg(test)]
mod pruebas_candado {
    #[test]
    fn solo_uno_toma_el_candado() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("vivo");
        let a = super::candado(&r).expect("el primero lo toma");
        assert!(super::candado(&r).is_none(), "el segundo no");
        assert_eq!(
            std::fs::read_to_string(&r).unwrap(),
            std::process::id().to_string()
        );
        drop(a);
        assert!(
            super::candado(&r).is_some(),
            "al soltarlo se puede tomar otra vez"
        );
    }
}
