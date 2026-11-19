//! Fotos y videos de las cámaras, con ffmpeg y sin recomprimir el video (`grabar.py`).
//!
//! La foto se saca del flujo principal (la resolución completa de la cámara, no la del mosaico) y va a
//! `~/Imágenes/Vortexine's Camera`; los videos, a `~/Vídeos/Vortexine's Camera`, en MP4 fragmentado: si la
//! PC se apaga a media grabación, lo grabado hasta ahí se puede ver. El audio de las Dahua suele venir en
//! G.711, que el MP4 no acepta: se pasa a AAC (el video se copia tal cual).

use crate::fecha::FechaHora;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub const CARPETA: &str = "Vortexine's Camera";

fn carpeta_xdg(tipo: &str, respaldo: &str) -> PathBuf {
    let casa = vortex_core::rutas::home();
    let ruta = Command::new("xdg-user-dir")
        .arg(tipo)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let base = if !ruta.is_empty() && Path::new(&ruta) != casa {
        PathBuf::from(ruta)
    } else {
        casa.join(respaldo)
    };
    base.join(CARPETA)
}

pub fn carpeta_fotos() -> PathBuf {
    carpeta_xdg("PICTURES", "Imágenes")
}

pub fn carpeta_videos() -> PathBuf {
    carpeta_xdg("VIDEOS", "Vídeos")
}

/// «Patio» → «Patio 2026-10-30 14.05.33.jpg» (sin / ni caracteres que estorben).
pub fn nombre_archivo(nombre: &str, extension: &str, cuando: FechaHora) -> String {
    let re = regex::Regex::new(r#"[/\\:*?"<>|\x00-\x1f]+"#).expect("regex fija");
    let limpio = re.replace_all(nombre, " ");
    let limpio = limpio.trim_matches(|c| c == ' ' || c == '.');
    let limpio: String = if limpio.is_empty() {
        "Cámara".into()
    } else {
        limpio.chars().take(60).collect()
    };
    format!("{limpio} {}.{extension}", cuando.sello())
}

/// Si ya existe, «Patio … (2).jpg», «(3)»…
fn libre(ruta: &Path) -> PathBuf {
    let tallo = ruta
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = ruta
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut n = 2;
    let mut nueva = ruta.to_path_buf();
    while nueva.exists() {
        nueva = ruta.with_file_name(format!("{tallo} ({n}){ext}"));
        n += 1;
    }
    nueva
}

fn ffmpeg() -> Result<PathBuf, String> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .map(|d| d.join("ffmpeg"))
        .find(|f| f.is_file())
        .ok_or_else(|| "Falta ffmpeg (sudo pacman -S ffmpeg).".to_string())
}

/// El error de ffmpeg → algo que se entienda.
pub fn explicar(error: &str) -> String {
    let e = error.to_lowercase();
    if e.contains("401") || e.contains("unauthorized") {
        "La cámara no aceptó el código de seguridad.".into()
    } else if ["timed out", "connection refused", "no route", "unreachable"]
        .iter()
        .any(|x| e.contains(x))
    {
        "No se pudo conectar con la cámara.".into()
    } else if e.contains("404") || e.contains("not found") {
        "La cámara no tiene ese video.".into()
    } else {
        "No se pudo leer el video de la cámara.".into()
    }
}

/// Un cuadro del video, en JPG. Si no se pudo, un texto para la persona.
pub fn foto(url: &str, carpeta: &Path, nombre: &str, espera: Duration) -> Result<PathBuf, String> {
    let ff = ffmpeg()?;
    std::fs::create_dir_all(carpeta).map_err(|_| "No se pudo crear la carpeta de fotos.".to_string())?;
    let destino = libre(&carpeta.join(nombre_archivo(nombre, "jpg", FechaHora::ahora())));
    // Los errores a un archivo: la URL lleva el código y no debe acabar en la terminal.
    let mut errores = tempfile::tempfile().map_err(|e| e.to_string())?;
    let mut hijo = Command::new(ff)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-rtsp_transport",
            "tcp",
            "-i",
        ])
        .arg(url)
        .args(["-frames:v", "1", "-q:v", "2", "-y"])
        .arg(&destino)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(errores.try_clone().map_err(|e| e.to_string())?)
        .spawn()
        .map_err(|e| e.to_string())?;
    let limite = Instant::now() + espera;
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if Instant::now() < limite => std::thread::sleep(Duration::from_millis(50)),
            _ => {
                let _ = hijo.kill();
                let _ = hijo.wait();
                break None;
            }
        }
    };
    let Some(estado) = estado else {
        let _ = std::fs::remove_file(&destino);
        return Err("La cámara no mandó imagen a tiempo.".into());
    };
    let tam = std::fs::metadata(&destino).map_or(0, |m| m.len());
    if !estado.success() || tam == 0 {
        let _ = std::fs::remove_file(&destino);
        let mut texto = String::new();
        let _ = errores.rewind();
        let _ = errores.read_to_string(&mut texto);
        return Err(explicar(&texto));
    }
    Ok(destino)
}

/// Un ffmpeg grabando una cámara. `detener` le manda «q» para que cierre bien el archivo.
pub struct Grabacion {
    pub destino: PathBuf,
    proceso: Child,
    errores: Option<std::fs::File>,
}

impl Grabacion {
    pub fn new(url: &str, carpeta: &Path, nombre: &str) -> Result<Grabacion, String> {
        let ff = ffmpeg()?;
        std::fs::create_dir_all(carpeta).map_err(|_| "No se pudo crear la carpeta de videos.".to_string())?;
        let destino = libre(&carpeta.join(nombre_archivo(nombre, "mp4", FechaHora::ahora())));
        // Los errores van a un archivo y no a una tubería: en horas de grabación una tubería llena dejaría a
        // ffmpeg esperando.
        let errores = tempfile::tempfile().map_err(|e| e.to_string())?;
        let proceso = Command::new(ff)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-rtsp_transport",
                "tcp",
                "-i",
            ])
            .arg(url)
            .args([
                "-map",
                "0:v:0",
                "-map",
                "0:a:0?",
                "-c:v",
                "copy",
                "-c:a",
                "aac",
                "-b:a",
                "96k",
                "-movflags",
                "+frag_keyframe+empty_moov+default_base_moof",
                "-f",
                "mp4",
            ])
            .arg(&destino)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(errores.try_clone().map_err(|e| e.to_string())?)
            .spawn()
            .map_err(|_| "No se pudo empezar a grabar.".to_string())?;
        Ok(Grabacion {
            destino,
            proceso,
            errores: Some(errores),
        })
    }

    pub fn activa(&mut self) -> bool {
        matches!(self.proceso.try_wait(), Ok(None))
    }

    /// Si ffmpeg ya terminó solo (la cámara se cayó, código malo…), por qué.
    pub fn error(&mut self) -> String {
        if self.activa() {
            return String::new();
        }
        let Some(f) = self.errores.as_mut() else {
            return String::new();
        };
        let mut texto = String::new();
        let _ = f.rewind();
        let _ = f.read_to_string(&mut texto);
        explicar(&texto)
    }

    fn esperar(&mut self, espera: Duration) -> bool {
        let limite = Instant::now() + espera;
        while Instant::now() < limite {
            if !matches!(self.proceso.try_wait(), Ok(None)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }

    /// Cierra el archivo. Devuelve la ruta si quedó algo grabado.
    pub fn detener(mut self, espera: Duration) -> Option<PathBuf> {
        if self.activa() {
            if let Some(entrada) = self.proceso.stdin.as_mut() {
                let _ = entrada.write_all(b"q");
                let _ = entrada.flush();
            }
            if !self.esperar(espera) {
                // SIGTERM como `terminate()`; si tampoco, SIGKILL.
                let _ = Command::new("kill")
                    .args(["-TERM", &self.proceso.id().to_string()])
                    .status();
                if !self.esperar(Duration::from_secs(3)) {
                    let _ = self.proceso.kill();
                }
            }
        }
        let _ = self.proceso.wait();
        drop(self.proceso.stdin.take());
        self.errores = None;
        if std::fs::metadata(&self.destino).is_ok_and(|m| m.len() > 0) {
            return Some(self.destino.clone());
        }
        let _ = std::fs::remove_file(&self.destino);
        None
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nombres() {
        let t = FechaHora::new(2026, 10, 30, 14, 5, 33);
        assert_eq!(
            nombre_archivo("Patio/Cochera", "jpg", t),
            "Patio Cochera 2026-10-30 14.05.33.jpg"
        );
        assert_eq!(
            nombre_archivo(" ..:: ", "mp4", t),
            "Cámara 2026-10-30 14.05.33.mp4"
        );
        assert!(nombre_archivo(&"x".repeat(90), "jpg", t).starts_with(&"x".repeat(60)));
    }

    #[test]
    fn explica_los_errores() {
        assert!(explicar("method DESCRIBE failed: 401 Unauthorized").contains("código"));
        assert!(explicar("Connection refused").contains("conectar"));
        assert!(explicar("404 Not Found").contains("ese video"));
        assert!(explicar("algo raro").contains("leer"));
    }

    #[test]
    fn libre_no_pisa() {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("a b.jpg");
        assert_eq!(libre(&r), r);
        std::fs::write(&r, "x").unwrap();
        assert_eq!(libre(&r), d.path().join("a b (2).jpg"));
    }
}
