//! 4.5: quién está usando la cámara, para el aviso de la barra.
//!
//! Discord, Zoom, el navegador y OBS abren `/dev/videoN` directo (sin PipeWire), así que se vigila con
//! inotify: un hilo aparte duerme en `read()` hasta que alguien abre o cierra una cámara (o se conecta una
//! nueva), espera un momento —el navegador abre las cámaras un instante solo para listarlas— y busca en
//! `/proc/*/fd` qué programas la tienen abierta. El resultado va a `$XDG_RUNTIME_DIR/vortex/camara` (un
//! programa por renglón; vacío = nadie) y el shell lo lee con un FileView que vigila el archivo.

use crate::sis;
use std::collections::BTreeSet;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Cuánto se espera después de abrir o cerrar antes de mirar quién la tiene.
const ESPERA: Duration = Duration::from_millis(1500);

/// Los que la abren por otros (PipeWire la presta a las apps; esas se ven en el shell por PipeWire).
const SALTAR: &[&str] = &["pipewire", "wireplumber"];

fn es_camara(nombre: &str) -> bool {
    nombre
        .strip_prefix("video")
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Los programas (su `comm`) que tienen abierta alguna cámara de `dev`.
pub fn quien_la_usa(proc: &Path, dev: &Path) -> Vec<String> {
    let mut apps = BTreeSet::new();
    let Ok(rd) = std::fs::read_dir(proc) else {
        return Vec::new();
    };
    for e in rd.flatten() {
        let nombre = e.file_name();
        let pid = nombre.to_string_lossy();
        if pid.is_empty() || !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(e.path().join("fd")) else {
            continue; // de otro usuario, o ya terminó
        };
        let usa = fds.flatten().any(|f| {
            std::fs::read_link(f.path()).is_ok_and(|l| {
                l.parent() == Some(dev) && l.file_name().is_some_and(|n| es_camara(&n.to_string_lossy()))
            })
        });
        if !usa {
            continue;
        }
        let comm = std::fs::read_to_string(e.path().join("comm")).unwrap_or_default();
        let comm = comm.trim();
        if !comm.is_empty() && !SALTAR.contains(&comm) {
            apps.insert(comm.to_string());
        }
    }
    apps.into_iter().collect()
}

fn escribir(arch: &Path, apps: &[String]) {
    if let Some(d) = arch.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let mut t = apps.join("\n");
    if !t.is_empty() {
        t.push('\n');
    }
    let tmp = arch.with_extension("tmp");
    if std::fs::write(&tmp, t).is_ok() {
        let _ = std::fs::rename(&tmp, arch);
    }
}

fn vigilar_camaras(fd: &impl AsRawFd, dev: &Path) {
    let Ok(rd) = std::fs::read_dir(dev) else {
        return;
    };
    for e in rd.flatten() {
        if es_camara(&e.file_name().to_string_lossy()) {
            // Repetir el mismo archivo no hace nada (inotify devuelve el mismo wd).
            let _ = sis::inotify_vigilar(
                fd,
                &e.path(),
                libc::IN_OPEN | libc::IN_CLOSE_WRITE | libc::IN_CLOSE_NOWRITE,
            );
        }
    }
}

/// Arranca el hilo. Sin inotify, o si `dev` no existe, no hace nada (y no hay aviso de cámara).
pub fn arrancar(dev: PathBuf, proc: PathBuf, estado: PathBuf) {
    let Ok(fd) = sis::inotify_nuevo() else {
        return;
    };
    // Una cámara nueva (USB) aparece como un archivo nuevo en /dev.
    if sis::inotify_vigilar(&fd, &dev, libc::IN_CREATE).is_err() {
        return;
    }
    vigilar_camaras(&fd, &dev);
    let arch = estado.join("camara");
    let mut ultimo = quien_la_usa(&proc, &dev);
    escribir(&arch, &ultimo);
    let _ = std::thread::Builder::new().name("camara".into()).spawn(move || {
        let mut f = std::fs::File::from(fd);
        let mut buf = [0u8; 4096];
        loop {
            // Duerme aquí hasta que pase algo.
            match f.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            // Lo que llegue mientras se espera despierta una vuelta más, que no cambia nada.
            std::thread::sleep(ESPERA);
            vigilar_camaras(&f, &dev);
            let ahora = quien_la_usa(&proc, &dev);
            if ahora != ultimo {
                escribir(&arch, &ahora);
                ultimo = ahora;
            }
        }
    });
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_las_camaras() {
        assert!(es_camara("video0"));
        assert!(es_camara("video12"));
        assert!(!es_camara("video"));
        assert!(!es_camara("videox"));
        assert!(!es_camara("dri"));
    }

    #[test]
    fn encuentra_quien_la_tiene() {
        let d = tempfile::tempdir().unwrap();
        let dev = d.path().join("dev");
        let proc = d.path().join("proc");
        std::fs::create_dir_all(&dev).unwrap();
        std::fs::write(dev.join("video0"), "").unwrap();
        std::fs::write(dev.join("null"), "").unwrap();
        let pid = |n: &str, comm: &str, a: &str| {
            let p = proc.join(n);
            std::fs::create_dir_all(p.join("fd")).unwrap();
            std::fs::write(p.join("comm"), format!("{comm}\n")).unwrap();
            std::os::unix::fs::symlink(dev.join(a), p.join("fd/3")).unwrap();
        };
        pid("100", "Discord", "video0");
        pid("101", "kitty", "null");
        pid("102", "pipewire", "video0");
        pid("103", "Discord", "video0");
        std::fs::create_dir_all(proc.join("self")).unwrap();
        assert_eq!(quien_la_usa(&proc, &dev), vec!["Discord"]);
    }
}
