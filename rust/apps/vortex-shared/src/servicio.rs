//! Escucha los avisos de otras compus. Lo arranca el socket de systemd (`vortex-shared.socket`) cuando alguien
//! se conecta y se apaga solo al terminar: en reposo no hay ningún proceso.
//!
//! Sale un aviso para aceptar o rechazar. Nada se baja sin que aceptes, y nunca sale con un juego abierto.

#![allow(unsafe_code)] // adoptar el descriptor que abrió systemd (LISTEN_FDS)

use crate::Config;
use crate::motor::{self, ErrorShared, MAX_LINEA};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const APP: &str = "Vortexine Shared";

/// El Hyprland vivo es el del socket más nuevo (el servicio no hereda la sesión).
fn socket_hyprland(cfg: &Config) -> Option<PathBuf> {
    std::fs::read_dir(cfg.run.join("hypr"))
        .ok()?
        .flatten()
        .map(|d| d.path().join(".socket2.sock"))
        .filter_map(|p| Some((std::fs::metadata(&p).ok()?.modified().ok()?, p)))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// La marca de «vortex modo-juego», si es de esta sesión de Hyprland.
pub fn jugando(cfg: &Config) -> bool {
    let Ok(marca) = std::fs::metadata(&cfg.marca_juego).and_then(|m| m.modified()) else {
        return false;
    };
    match socket_hyprland(cfg).and_then(|p| std::fs::metadata(p).ok()?.modified().ok()) {
        None => true,
        Some(hypr) => marca > hypr,
    }
}

fn leer_todo(mut r: impl Read) -> String {
    let mut s = String::new();
    let _ = r.read_to_string(&mut s);
    s
}

/// Cierra un aviso por su número.
fn cerrar_aviso(cfg: &Config, id: &str) {
    cfg.sistema.correr(
        &[
            "gdbus",
            "call",
            "--session",
            "--dest",
            "org.freedesktop.Notifications",
            "--object-path",
            "/org/freedesktop/Notifications",
            "--method",
            "org.freedesktop.Notifications.CloseNotification",
            id,
        ],
        Duration::from_secs(5),
    );
}

/// El aviso con sus botones. Si nadie contesta a tiempo, se cierra y cuenta como no. Picarle a la tarjeta
/// («default») no acepta nada: el aviso vuelve a salir.
pub fn preguntar(cfg: &Config, titulo: &str, cuerpo: &str) -> bool {
    let Some(notify) = cfg.sistema.buscar("notify-send") else {
        return false;
    };
    let fin = Instant::now() + cfg.espera_respuesta;
    loop {
        let hijo = std::process::Command::new(&notify)
            .args([
                "-a",
                APP,
                "-i",
                "folder-download",
                "-u",
                "critical",
                "-p",
                "-A",
                "default=Detalles",
                "-A",
                "aceptar=Aceptar",
                "-A",
                "rechazar=Rechazar",
                titulo,
                cuerpo,
            ])
            .env(
                "PATH",
                cfg.sistema
                    .path
                    .clone()
                    .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let Ok(mut hijo) = hijo else { return false };
        let lector = std::thread::spawn({
            let o = hijo.stdout.take().unwrap();
            move || leer_todo(o)
        });
        let limite = fin.max(Instant::now() + Duration::from_millis(100));
        let a_tiempo = loop {
            match hijo.try_wait() {
                Ok(Some(_)) => break true,
                Ok(None) if Instant::now() >= limite => break false,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(_) => break true,
            }
        };
        if !a_tiempo {
            let _ = hijo.kill();
            let _ = hijo.wait();
            let salida = lector.join().unwrap_or_default();
            if let Some(id) = salida
                .split_whitespace()
                .next()
                .filter(|p| p.bytes().all(|b| b.is_ascii_digit()))
            {
                cerrar_aviso(cfg, id);
            }
            return false;
        }
        let _ = hijo.wait();
        let salida = lector.join().unwrap_or_default();
        let palabras: Vec<&str> = salida.split_whitespace().collect();
        if palabras.contains(&"default") && Instant::now() < fin {
            continue;
        }
        return palabras.contains(&"aceptar");
    }
}

/// «Ya llegó», con un botón para abrir la carpeta.
pub fn avisar_listo(cfg: &Config, carpeta: &Path, texto: &str) {
    let Some(notify) = cfg.sistema.buscar("notify-send") else {
        return;
    };
    let notify = notify.to_string_lossy().into_owned();
    let r = cfg.sistema.correr(
        &[
            &notify,
            "-a",
            APP,
            "-i",
            "folder-download",
            "-A",
            "abrir=Abrir carpeta",
            "Ya llegó",
            texto,
        ],
        Duration::from_secs(300),
    );
    if r.salida.trim() != "abrir" {
        return;
    }
    let firma =
        socket_hyprland(cfg).and_then(|p| p.parent()?.file_name().map(|n| n.to_string_lossy().into_owned()));
    let ruta = carpeta.to_string_lossy().replace('"', "");
    for orden in [
        format!("hl.dsp.exec_cmd(\"xdg-open '{ruta}'\")"),
        format!("exec xdg-open '{ruta}'"),
    ] {
        let mut args = vec!["hyprctl", "dispatch", orden.as_str()];
        let r = match &firma {
            Some(f) => {
                let con = format!("HYPRLAND_INSTANCE_SIGNATURE={f}");
                let mut a = vec!["env", con.as_str()];
                a.append(&mut args);
                cfg.sistema.correr(&a, Duration::from_secs(3))
            }
            None => cfg.sistema.correr(&args, Duration::from_secs(3)),
        };
        if r.bien() || r.codigo == 124 || r.codigo == 127 {
            return;
        }
    }
}

fn responder(c: &mut TcpStream, datos: Value) -> bool {
    let mut l = datos.to_string();
    l.push('\n');
    c.write_all(l.as_bytes()).is_ok()
}

fn leer_linea(c: &mut TcpStream, espera: Duration) -> Vec<u8> {
    let _ = c.set_read_timeout(Some(Duration::from_secs(1).min(espera)));
    let fin = Instant::now() + espera;
    let mut datos: Vec<u8> = Vec::new();
    while !datos.contains(&b'\n') && datos.len() <= MAX_LINEA && Instant::now() < fin {
        let mut t = [0u8; 4096];
        match c.read(&mut t) {
            Ok(0) => break,
            Ok(n) => datos.extend_from_slice(&t[..n]),
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => break,
        }
    }
    match datos.iter().position(|b| *b == b'\n') {
        Some(i) => datos[..i].to_vec(),
        None => Vec::new(),
    }
}

pub struct Servicio {
    pub oyente: TcpListener,
    pub cfg: Config,
    /// uno a la vez: quien lo tiene está recibiendo
    pub ocupado: Arc<Mutex<()>>,
    ultimo: Arc<Mutex<Instant>>,
}

impl Servicio {
    pub fn new(oyente: TcpListener, cfg: Config) -> Self {
        Servicio {
            oyente,
            cfg,
            ocupado: Arc::default(),
            ultimo: Arc::new(Mutex::new(Instant::now())),
        }
    }

    /// Hace como si hubiera pasado mucho sin nada (para que el servicio se apague).
    pub fn dar_por_inactivo(&self) {
        *self.ultimo.lock().unwrap() = Instant::now()
            .checked_sub(Duration::from_secs(86400))
            .unwrap_or_else(Instant::now);
    }

    /// Atiende una conexión ya aceptada (`ip` es la de quien llama).
    pub fn atender(&self, mut conexion: TcpStream, ip: &str) {
        let _ = conexion.set_nodelay(true);
        self.atender_dentro(&mut conexion, ip);
        *self.ultimo.lock().unwrap() = Instant::now();
    }

    fn atender_dentro(&self, c: &mut TcpStream, ip: &str) {
        if !motor::ip_permitida(ip) {
            return;
        }
        let linea = leer_linea(c, Duration::from_secs(10));
        let Ok(valor) = serde_json::from_slice::<Value>(&linea) else {
            responder(c, json!({"r": "error"}));
            return;
        };
        let Ok(peticion) = motor::validar_peticion(&valor) else {
            responder(c, json!({"r": "error"}));
            return;
        };
        if peticion.usuario != self.cfg.usuario {
            responder(c, json!({"r": "usuario"}));
            return;
        }
        let ocupado = Arc::clone(&self.ocupado);
        let guardia = ocupado.try_lock();
        if jugando(&self.cfg) || guardia.is_err() {
            responder(c, json!({"r": "ocupado"}));
            return;
        }
        self.recibir(c, &peticion, ip);
    }

    fn recibir(&self, c: &mut TcpStream, peticion: &motor::Peticion, ip: &str) {
        let carpeta = motor::carpeta_recibidos(&self.cfg);
        let _ = std::fs::create_dir_all(&carpeta);
        // mientras baja, sendme tiene su copia y la que deja
        if motor::espacio_libre(&carpeta) < peticion.tam.saturating_mul(2) {
            responder(c, json!({"r": "espacio"}));
            return;
        }
        let titulo = format!("{} te quiere mandar archivos", peticion.de);
        let cuerpo = format!(
            "{}. Desde {}.",
            motor::texto_peticion(peticion),
            ip.trim_start_matches("::ffff:")
        );
        if !preguntar(&self.cfg, &titulo, &cuerpo) {
            responder(c, json!({"r": "rechazado"}));
            return;
        }
        if !responder(c, json!({"r": "aceptado"})) {
            return;
        }
        let _ = c.set_write_timeout(Some(Duration::from_secs(5)));
        let Ok(mut avisos) = c.try_clone() else { return };
        let vivo = std::cell::Cell::new(true);
        let ultimo = std::cell::Cell::new(None::<Instant>);
        let mut avance = |n: u64| {
            let ahora = Instant::now();
            if ultimo
                .get()
                .is_none_or(|u| ahora.duration_since(u) >= Duration::from_secs(1))
            {
                ultimo.set(Some(ahora));
                let ok = responder(&mut avisos, json!({"r": "avance", "bytes": n}));
                vivo.set(vivo.get() && ok);
            }
        };
        let quedaron = motor::recibir(
            &self.cfg,
            &peticion.ticket,
            Some(&mut avance),
            &|| !vivo.get(),
            peticion.tam,
        );
        let Ok(quedaron): Result<Vec<PathBuf>, ErrorShared> = quedaron else {
            responder(c, json!({"r": "error"}));
            return;
        };
        responder(c, json!({"r": "listo"}));
        let _ = c.shutdown(std::net::Shutdown::Both);
        let nombre_de = |p: &PathBuf| {
            p.file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
        };
        let nombre = if quedaron.len() == 1 {
            nombre_de(&quedaron[0])
        } else {
            format!("{} y {} más", nombre_de(&quedaron[0]), quedaron.len() - 1)
        };
        let carpeta_nombre = nombre_de(&carpeta);
        avisar_listo(
            &self.cfg,
            &carpeta,
            &motor::limpiar_nombre(&format!("{nombre}, en Descargas/{carpeta_nombre}")),
        );
    }

    /// Acepta conexiones hasta que pasa `inactivo` sin nadie.
    pub fn correr(&self) {
        let _ = self.oyente.set_nonblocking(true);
        let mut hilos: Vec<JoinHandle<()>> = Vec::new();
        loop {
            hilos.retain(|h| !h.is_finished());
            match self.oyente.accept() {
                Ok((conexion, direccion)) => {
                    let _ = conexion.set_nonblocking(false);
                    *self.ultimo.lock().unwrap() = Instant::now();
                    let este = self.clonar();
                    hilos.push(std::thread::spawn(move || {
                        este.atender(conexion, &direccion.ip().to_string());
                    }));
                }
                Err(_) => {
                    if hilos.is_empty() && self.ultimo.lock().unwrap().elapsed() > self.cfg.inactivo {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    fn clonar(&self) -> Servicio {
        Servicio {
            oyente: self.oyente.try_clone().expect("duplicar el socket"),
            cfg: self.cfg.clone(),
            ocupado: Arc::clone(&self.ocupado),
            ultimo: Arc::clone(&self.ultimo),
        }
    }
}

/// El socket que abrió systemd (`LISTEN_FDS`, el descriptor 3). `None` si no lo arrancó systemd.
pub fn oyente_de_systemd() -> Option<TcpListener> {
    use std::os::fd::FromRawFd;
    let pid = std::env::var("LISTEN_PID").ok()?;
    if pid != std::process::id().to_string() || std::env::var("LISTEN_FDS").ok()? != "1" {
        return None;
    }
    // SAFETY: systemd nos dejó ese descriptor (el 3) abierto y solo nuestro: LISTEN_PID es este proceso.
    Some(unsafe { TcpListener::from_raw_fd(3) })
}

/// `vortex-shared escuchar`.
pub fn main(cfg: Config) -> i32 {
    let Some(oyente) = oyente_de_systemd() else {
        eprintln!("vortex-shared escuchar lo arranca vortex-shared.socket (systemd).");
        return 1;
    };
    Servicio::new(oyente, cfg).correr();
    0
}
