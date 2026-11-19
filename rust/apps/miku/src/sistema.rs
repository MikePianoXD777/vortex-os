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
