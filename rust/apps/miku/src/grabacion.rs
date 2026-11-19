//! Grabar la pantalla (una partida) con gpu-screen-recorder, solo cuando la persona se lo pide a Miku.
//!
//! La grabación es un proceso aparte (sesión propia): sigue aunque cierres a Miku, y se para con «Miku, ya
//! para» o con `vortex-miku grabar parar`. Graba el monitor enfocado a 60 fps con el sonido del sistema (no
//! el micrófono) en `~/Vídeos/Miku`. gpu-screen-recorder guarda el archivo al recibir SIGINT.

use crate::sistema;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const PROGRAMA: &str = "gpu-screen-recorder";
pub const FPS: &str = "60";
pub const ESPERA_ARRANQUE: Duration = Duration::from_millis(1500);
pub const ESPERA_CIERRE: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorGrabacion(pub String);

impl std::fmt::Display for ErrorGrabacion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct EnCurso {
    pub pid: u32,
    pub archivo: PathBuf,
    pub inicio: f64,
}

fn runtime() -> PathBuf {
    let base = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/tmp/vortex-miku-{}", sistema::uid())));
    let d = base.join("vortex-miku");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// `~/Vídeos/Miku` (la carpeta de vídeos de tu idioma).
pub fn carpeta_videos() -> PathBuf {
    if let Some(v) = std::env::var_os("VORTEX_MIKU_VIDEOS").filter(|v| !v.is_empty()) {
        return v.into(); // pruebas
    }
    let home = vortex_core::rutas::home();
    let mut base = String::new();
    if sistema::cual("xdg-user-dir").is_some()
        && let Ok(o) = Command::new("xdg-user-dir")
            .arg("VIDEOS")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
    {
        base = String::from_utf8_lossy(&o.stdout).trim().to_string();
    }
    if base.is_empty() || Path::new(&base) == home {
        base = home.join("Vídeos").to_string_lossy().into_owned();
    }
    Path::new(&base).join("Miku")
}

fn es_grabadora(pid: u32) -> bool {
    let Ok(c) = std::fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    // El programa, o un script que lo envuelve (sh gpu-screen-recorder …, como en las pruebas).
    c.split(|b| *b == 0).take(2).any(|p| {
        Path::new(&*String::from_utf8_lossy(p))
            .file_name()
            .is_some_and(|n| n == PROGRAMA)
    })
}

/// La grabación en curso, o `None` (y borra un estado viejo).
pub fn grabando() -> Option<EnCurso> {
    let ruta = runtime().join("grabacion.json");
    let e: Value = serde_json::from_str(&std::fs::read_to_string(&ruta).ok()?).ok()?;
    let pid = e["pid"].as_u64()? as u32;
    if es_grabadora(pid) {
        return Some(EnCurso {
            pid,
            archivo: e["archivo"].as_str()?.into(),
            inicio: e["inicio"].as_f64().unwrap_or(0.0),
        });
    }
    let _ = std::fs::remove_file(ruta);
    None
}

/// El monitor enfocado de Hyprland (p. ej. DP-2), o «screen» (el primero que encuentre).
fn monitor() -> String {
    if sistema::cual("hyprctl").is_some()
        && let Ok(o) = Command::new("hyprctl")
            .args(["monitors", "-j"])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
        && let Ok(Value::Array(ms)) = serde_json::from_slice::<Value>(&o.stdout)
    {
        for m in ms {
            if m["focused"].as_bool() == Some(true)
                && let Some(n) = m["name"].as_str().filter(|n| !n.is_empty())
            {
                return n.into();
            }
        }
    }
    "screen".into()
}

pub fn ultima_grabacion() -> Option<PathBuf> {
    let mut v: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(carpeta_videos())
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "mp4") && e.path().is_file())
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    v.sort();
    v.pop().map(|x| x.1)
}

fn ahora_unix() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

pub fn empezar() -> Result<PathBuf, ErrorGrabacion> {
    if grabando().is_some() {
        return Err(ErrorGrabacion(
            "Ya estoy grabando. Para pararla: «Miku, ya para» o vortex-miku grabar parar.".into(),
        ));
    }
    if sistema::cual(PROGRAMA).is_none() {
        return Err(ErrorGrabacion(
            "Falta gpu-screen-recorder (sudo pacman -S gpu-screen-recorder).".into(),
        ));
    }
    let carpeta = carpeta_videos();
    std::fs::create_dir_all(&carpeta)
        .map_err(|e| ErrorGrabacion(format!("No pude crear {}: {e}", carpeta.display())))?;
    let t = crate::fecha::ahora();
    let archivo = carpeta.join(format!(
        "Partida {} {:02}.{:02}.{:02}.mp4",
        t.fecha_iso(),
        t.hora,
        t.minuto,
        t.segundo
    ));
    let monitor = monitor();
    let registro = runtime().join("grabacion.log");
    let log = std::fs::File::create(&registro).map_err(|e| ErrorGrabacion(e.to_string()))?;
    let mut c = Command::new(PROGRAMA);
    c.args([
        "-w",
        &monitor,
        "-f",
        FPS,
        "-a",
        "default_output",
        "-c",
        "mp4",
        "-o",
    ])
    .arg(&archivo)
    .stdin(Stdio::null())
    .stdout(log.try_clone().map_err(|e| ErrorGrabacion(e.to_string()))?)
    .stderr(log);
    sistema::nueva_sesion(&mut c);
    let mut hijo = c
        .spawn()
        .map_err(|e| ErrorGrabacion(format!("No pude empezar a grabar ({monitor}): {e}")))?;
    std::thread::sleep(ESPERA_ARRANQUE);
    if let Ok(Some(_)) = hijo.try_wait() {
        let ultimo = std::fs::read_to_string(&registro)
            .ok()
            .and_then(|t| t.trim().lines().last().map(String::from))
            .unwrap_or_default();
        return Err(ErrorGrabacion(
            format!("No pude empezar a grabar ({monitor})")
                + &if ultimo.is_empty() {
                    ".".to_string()
                } else {
                    format!(": {ultimo}")
                },
        ));
    }
    let estado = json!({"pid": hijo.id(), "archivo": archivo, "inicio": ahora_unix()});
    let _ = std::fs::write(runtime().join("grabacion.json"), estado.to_string());
    std::thread::spawn(move || {
        let _ = hijo.wait(); // que no quede zombi si seguimos vivos
    });
    Ok(archivo)
}

/// (archivo, segundos grabados).
pub fn parar() -> Result<(PathBuf, f64), ErrorGrabacion> {
    let e = grabando().ok_or_else(|| ErrorGrabacion("No estoy grabando nada.".into()))?;
    sistema::matar(e.pid, libc::SIGINT);
    let limite = Instant::now() + ESPERA_CIERRE;
    while es_grabadora(e.pid) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = std::fs::remove_file(runtime().join("grabacion.json"));
    let dur = (ahora_unix() - if e.inicio > 0.0 { e.inicio } else { ahora_unix() }).max(0.0);
    if !e.archivo.is_file() || e.archivo.metadata().map(|m| m.len() == 0).unwrap_or(true) {
        return Err(ErrorGrabacion(
            "Paré la grabación, pero no quedó ningún video.".into(),
        ));
    }
    Ok((e.archivo, dur))
}

pub fn duracion_texto(segundos: f64) -> String {
    let t = segundos.round() as i64;
    let (m, s) = (t / 60, t % 60);
    let (h, m) = (m / 60, m % 60);
    if h > 0 {
        format!("{h} h {m} min")
    } else if m > 0 {
        format!("{m} min {s} s")
    } else {
        format!("{s} s")
    }
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn duraciones() {
        assert_eq!(super::duracion_texto(5.4), "5 s");
        assert_eq!(super::duracion_texto(125.0), "2 min 5 s");
        assert_eq!(super::duracion_texto(3725.0), "1 h 2 min");
    }
}
