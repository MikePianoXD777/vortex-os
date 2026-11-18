//! Lo que no necesita Qt: el protocolo del aviso, mandar y recibir con `sendme`, y dónde queda todo.
//!
//! El aviso es JSON por líneas sobre TCP, en el puerto 1770. Solo lleva nombres, tamaño y el ticket de
//! sendme: el archivo nunca pasa por ahí. Quien manda espera `{"r": "aceptado"}`, luego
//! `{"r": "avance", "bytes": N}` y al final `{"r": "listo"}`. Sendme se corre en una carpeta temporal porque
//! deja ahí su base de datos.

#![allow(unsafe_code)] // killpg y statvfs: la biblioteca estándar no los tiene

use crate::Config;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

pub const VERSION_PROTOCOLO: u64 = 1;
/// sendme manda una sola ruta: varios archivos van en una carpeta con este nombre
pub const LOTE: &str = "vortex-shared-lote";
pub const MAX_LINEA: usize = 64 * 1024;
pub const MAX_NOMBRES: usize = 20;

/// Un problema que se le puede decir tal cual a la persona.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorShared(pub String);

impl std::fmt::Display for ErrorShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErrorShared {}

fn error<T>(texto: impl Into<String>) -> Result<T, ErrorShared> {
    Err(ErrorShared(texto.into()))
}

fn mensaje(clave: &str) -> &'static str {
    match clave {
        "rechazado" => "En la otra compu dijeron que no.",
        "usuario" => "Esa compu no tiene ese usuario.",
        "ocupado" => "La otra compu está ocupada (otro envío o un juego). Intenta en un rato.",
        "espacio" => "A la otra compu no le alcanza el disco.",
        "sin_respuesta" => "Nadie aceptó a tiempo.",
        "error" => "La otra compu no pudo bajar los archivos.",
        _ => "La otra compu no pudo recibirlos.",
    }
}

/// Un ticket de sendme: `blob` y de 10 a 4000 letras minúsculas o números. Va a un comando, así que se
/// revisa carácter por carácter (nada de «--opción» ni de «;»).
pub fn ticket_valido(t: &str) -> bool {
    t.strip_prefix("blob").is_some_and(|r| {
        (10..=4000).contains(&r.len())
            && r.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

pub fn usuario_valido(u: &str) -> bool {
    (1..=64).contains(&u.len())
        && u.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

// Sendme nunca sobrevive a quien lo lanzó: si la ventana se cae no debe seguir compartiendo tus archivos. Este
// sh corre sendme y se queda leyendo su entrada; al morir quien lo lanzó la tubería se cierra y corta a sendme.
const GUARDIA: &str = "exec 3<&0; sendme \"$@\" </dev/null 3<&- & p=$!; (cat <&3 >/dev/null; kill \"$p\" 2>/dev/null) & g=$!; exec 3<&-; wait \"$p\"; c=$?; kill \"$g\" 2>/dev/null; exit \"$c\"";

fn lanzar_sendme(
    cfg: &Config,
    args: &[&str],
    cwd: &Path,
    salida: Stdio,
) -> std::io::Result<Child> {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", GUARDIA, "sendme"])
        .args(args)
        .current_dir(cwd)
        .env("PATH", cfg.sistema.path.clone().unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()))
        .stdin(Stdio::piped())
        .stdout(salida)
        .stderr(Stdio::null())
        .process_group(0);
    cmd.spawn()
}

/// SIGTERM o SIGKILL a todo el grupo del proceso (el guardia, sendme y sus hijos).
fn matar_grupo(hijo: &Child, fuerte: bool) {
    // SAFETY: killpg con el pid de un hijo que nosotros lanzamos como líder de su grupo.
    unsafe {
        libc::killpg(
            hijo.id() as libc::pid_t,
            if fuerte { libc::SIGKILL } else { libc::SIGTERM },
        );
    }
}

fn esperar_con_limite(hijo: &mut Child, limite: Duration) -> bool {
    let fin = Instant::now() + limite;
    loop {
        match hijo.try_wait() {
            Ok(Some(_)) | Err(_) => return true,
            Ok(None) if Instant::now() >= fin => return false,
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Lo que dejó un envío que se cortó de golpe (más de un día).
pub fn limpiar_viejos(cfg: &Config) {
    let Ok(lista) = std::fs::read_dir(&cfg.cache) else {
        return;
    };
    for d in lista.flatten() {
        let nombre = d.file_name().to_string_lossy().into_owned();
        if !(nombre.starts_with("envio-") || nombre.starts_with(".vortex-shared-")) {
            continue;
        }
        let viejo = d
            .metadata()
            .ok()
            .filter(|m| m.is_dir())
            .and_then(|m| m.modified().ok())
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|e| e > Duration::from_secs(86400));
        if viejo {
            let _ = std::fs::remove_dir_all(d.path());
        }
    }
}

// --- Dónde ---

/// `~/Descargas/Vortexine Shared` (o `VORTEX_SHARED_CARPETA`).
pub fn carpeta_recibidos(cfg: &Config) -> PathBuf {
    if let Some(f) = &cfg.carpeta_fija {
        return f.clone();
    }
    let d = cfg
        .sistema
        .correr(&["xdg-user-dir", "DOWNLOAD"], Duration::from_secs(3));
    let d = d.salida.trim();
    let base = if d.is_empty() {
        cfg.home.join("Descargas")
    } else {
        PathBuf::from(d)
    };
    base.join("Vortexine Shared")
}

pub fn humano(n: u64) -> String {
    let mut valor = n as f64;
    for unidad in ["B", "KB", "MB", "GB"] {
        if valor < 1024.0 {
            return if unidad == "B" {
                format!("{valor:.0} {unidad}")
            } else {
                format!("{valor:.1} {unidad}")
            };
        }
        valor /= 1024.0;
    }
    format!("{valor:.1} TB")
}

// --- Quién puede avisar ---

/// Solo la misma red o Tailscale: nadie de internet te puede llenar de avisos.
pub fn ip_permitida(texto: &str) -> bool {
    use std::net::{IpAddr, Ipv4Addr};
    let limpio = texto.split('%').next().unwrap_or("");
    let Ok(ip) = limpio.parse::<IpAddr>() else {
        return false;
    };
    let v4 = |a: Ipv4Addr| {
        let o = a.octets();
        a.is_private()
            || a.is_loopback()
            || a.is_link_local()
            || (o[0] == 100 && (64..128).contains(&o[1])) // Tailscale: 100.64.0.0/10
    };
    match ip {
        IpAddr::V4(a) => v4(a),
        IpAddr::V6(a) => match a.to_ipv4_mapped() {
            Some(m) => v4(m),
            None => a.is_loopback() || a.is_unique_local() || a.is_unicast_link_local(),
        },
    }
}

/// Los nombres vienen de la otra compu y se ven en un aviso: sin saltos, controles ni marcado.
pub fn limpiar_nombre(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .filter(|c| !matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}' | '<' | '>' | '&'))
        .collect();
    let corto: String = limpio.trim().chars().take(80).collect();
    if corto.is_empty() {
        "archivo".into()
    } else {
        corto
    }
}

/// Lo que llega en el aviso, ya revisado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peticion {
    pub ticket: String,
    pub usuario: String,
    pub tam: u64,
    pub nombres: Vec<String>,
    pub n: u64,
    pub de: String,
}

/// La petición ya revisada, o `Err` (con qué falló). El ticket va a un comando: se revisa carácter por carácter.
pub fn validar_peticion(datos: &Value) -> Result<Peticion, &'static str> {
    let Some(d) = datos.as_object() else {
        return Err("versión");
    };
    if d.get("v").and_then(Value::as_u64) != Some(VERSION_PROTOCOLO) {
        return Err("versión");
    }
    let ticket = d
        .get("ticket")
        .and_then(Value::as_str)
        .filter(|t| ticket_valido(t))
        .ok_or("ticket")?;
    let usuario = d
        .get("usuario")
        .and_then(Value::as_str)
        .filter(|u| usuario_valido(u))
        .ok_or("usuario")?;
    let tam = d.get("tam").and_then(Value::as_u64).ok_or("tamaño")?;
    let nombres = d
        .get("nombres")
        .and_then(Value::as_array)
        .filter(|l| !l.is_empty())
        .ok_or("nombres")?;
    let n = d
        .get("n")
        .and_then(Value::as_u64)
        .filter(|n| *n > 0)
        .unwrap_or(nombres.len() as u64);
    let texto = |v: &Value| match v {
        Value::String(s) => s.clone(),
        otro => otro.to_string(),
    };
    Ok(Peticion {
        ticket: ticket.to_string(),
        usuario: usuario.to_string(),
        tam,
        nombres: nombres
            .iter()
            .take(MAX_NOMBRES)
            .map(|x| limpiar_nombre(&texto(x)))
            .collect(),
        n,
        de: limpiar_nombre(&d.get("de").map_or_else(|| "alguien".into(), texto)),
    })
}

pub fn texto_peticion(p: &Peticion) -> String {
    let primero = &p.nombres[0];
    if p.n == 1 {
        format!("{primero} ({})", humano(p.tam))
    } else {
        format!("{primero} y {} más ({})", p.n - 1, humano(p.tam))
    }
}

// --- Qué se manda ---

pub fn tamano(ruta: &Path) -> std::io::Result<u64> {
    let m = std::fs::metadata(ruta)?;
    if !m.is_dir() {
        return Ok(m.len());
    }
    Ok(peso_de(ruta, true))
}

/// Suma lo que pesan los archivos de una carpeta (lo que no se pueda leer cuenta cero).
fn peso_de(carpeta: &Path, seguir: bool) -> u64 {
    let mut total = 0;
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        return 0;
    };
    for e in lista.flatten() {
        let ruta = e.path();
        let m = if seguir {
            std::fs::metadata(&ruta)
        } else {
            std::fs::symlink_metadata(&ruta)
        };
        match m {
            Ok(m) if m.is_dir() => total += peso_de(&ruta, seguir),
            Ok(m) => total += m.len(),
            Err(_) => {}
        }
    }
    total
}

/// Enlace duro: no ocupa disco. Si el archivo está en otro disco, se copia.
fn enlazar(origen: &Path, destino: &Path) -> std::io::Result<()> {
    let real = std::fs::canonicalize(origen)?;
    if std::fs::hard_link(&real, destino).is_err() {
        std::fs::copy(&real, destino)?;
    }
    Ok(())
}

fn copiar_arbol(origen: &Path, destino: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(destino)?;
    for e in std::fs::read_dir(origen)? {
        let e = e?;
        let (o, d) = (e.path(), destino.join(e.file_name()));
        if std::fs::metadata(&o)?.is_dir() {
            copiar_arbol(&o, &d)?;
        } else {
            enlazar(&o, &d)?;
        }
    }
    Ok(())
}

fn absoluta(r: &Path) -> PathBuf {
    std::fs::canonicalize(r)
        .or_else(|_| std::path::absolute(r))
        .unwrap_or_else(|_| r.to_path_buf())
}

fn io(e: &std::io::Error) -> String {
    e.to_string()
        .split(" (os error")
        .next()
        .unwrap_or("")
        .to_string()
}

/// Un sendme mandando esos archivos. Sirve mientras siga vivo; `terminar` limpia todo.
pub struct Envio {
    cfg: Config,
    pub rutas: Vec<PathBuf>,
    pub nombres: Vec<String>,
    pub tam: u64,
    pub ticket: String,
    proc: Option<Child>,
    pub lote: Option<PathBuf>,
    pub cwd: PathBuf,
}

impl Envio {
    pub fn new(cfg: &Config, rutas: &[PathBuf]) -> Result<Self, ErrorShared> {
        let rutas: Vec<PathBuf> = rutas.iter().map(|r| absoluta(r)).collect();
        let faltan: Vec<String> = rutas
            .iter()
            .filter(|r| !r.exists())
            .map(|r| r.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))
            .collect();
        if rutas.is_empty() || !faltan.is_empty() {
            return error(if faltan.is_empty() {
                "Elige algo para mandar.".to_string()
            } else {
                format!("No encuentro {}.", faltan.join(", "))
            });
        }
        if cfg.sistema.buscar("sendme").is_none() {
            return error("Falta sendme (sudo pacman -S sendme).");
        }
        let nombres = rutas
            .iter()
            .map(|r| r.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()))
            .collect();
        let tam = rutas.iter().map(|r| tamano(r).unwrap_or(0)).sum();
        std::fs::create_dir_all(&cfg.cache)
            .map_err(|e| ErrorShared(format!("No se pudo preparar el envío: {}", io(&e))))?;
        limpiar_viejos(cfg);
        let cwd = tempfile::Builder::new()
            .prefix("envio-")
            .tempdir_in(&cfg.cache)
            .map_err(|e| ErrorShared(format!("No se pudo preparar el envío: {}", io(&e))))?
            .keep();
        Ok(Envio {
            cfg: cfg.clone(),
            rutas,
            nombres,
            tam,
            ticket: String::new(),
            proc: None,
            lote: None,
            cwd,
        })
    }

    fn preparar_lote(&mut self) -> std::io::Result<PathBuf> {
        // Junto a los archivos si están en otro disco que la caché: ahí sí se pueden enlazar.
        let base = self.cfg.cache.clone();
        let padre = self.rutas[0].parent().unwrap_or(Path::new("/")).to_path_buf();
        let otro_disco = match (padre.metadata(), base.metadata()) {
            (Ok(a), Ok(b)) => a.dev() != b.dev(),
            _ => false,
        };
        let crear = |donde: &Path| {
            tempfile::Builder::new()
                .prefix(".vortex-shared-")
                .tempdir_in(donde)
                .map(tempfile::TempDir::keep)
        };
        let lote = match otro_disco.then(|| crear(&padre)) {
            Some(Ok(l)) => l,
            _ => crear(&base)?,
        };
        self.lote = Some(lote.clone());
        let destino = lote.join(LOTE);
        std::fs::create_dir(&destino)?;
        let mut usados = BTreeSet::new();
        for ruta in &self.rutas {
            let original = ruta.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let (stem, ext) = match Path::new(&original).extension() {
                Some(e) => (
                    Path::new(&original)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    format!(".{}", e.to_string_lossy()),
                ),
                None => (original.clone(), String::new()),
            };
            let (mut nombre, mut n) = (original, 2);
            while usados.contains(&nombre) {
                // dos «foto.jpg» de carpetas distintas
                nombre = format!("{stem} ({n}){ext}");
                n += 1;
            }
            usados.insert(nombre.clone());
            if ruta.is_dir() {
                copiar_arbol(ruta, &destino.join(&nombre))?;
            } else {
                enlazar(ruta, &destino.join(&nombre))?;
            }
        }
        Ok(destino)
    }

    /// Arranca sendme y devuelve el ticket. Calcular la huella de algo grande tarda un rato.
    pub fn preparar(&mut self, espera: Duration) -> Result<String, ErrorShared> {
        let lanzado = (|| -> std::io::Result<Child> {
            let ruta = if self.rutas.len() == 1 {
                self.rutas[0].clone()
            } else {
                self.preparar_lote()?
            };
            lanzar_sendme(
                &self.cfg,
                &["send", &ruta.to_string_lossy()],
                &self.cwd,
                Stdio::piped(),
            )
        })();
        let mut hijo = match lanzado {
            Ok(h) => h,
            Err(e) => {
                self.terminar();
                return error(format!("No se pudo preparar el envío: {}", io(&e)));
            }
        };
        let salida = hijo.stdout.take().unwrap();
        self.proc = Some(hijo);
        // Un hilo lee todo lo que escribe sendme: el ticket sale por aquí, y si nadie siguiera leyendo, sendme
        // se atoraría cuando se llena la tubería.
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut avisado = false;
            for linea in BufReader::new(salida).lines().map_while(Result::ok) {
                if !avisado && let Some(t) = buscar_ticket(&linea) {
                    avisado = true;
                    let _ = tx.send(t);
                }
            }
        });
        match rx.recv_timeout(espera) {
            Ok(t) => {
                self.ticket = t.clone();
                Ok(t)
            }
            Err(_) => {
                self.terminar();
                error("No se pudo preparar el envío.")
            }
        }
    }

    pub fn peticion(&self, usuario: &str) -> Value {
        json!({
            "v": VERSION_PROTOCOLO, "ticket": self.ticket, "usuario": usuario, "tam": self.tam,
            "nombres": self.nombres.iter().take(MAX_NOMBRES).collect::<Vec<_>>(),
            "n": self.nombres.len(),
            "de": format!("{}@{}", self.cfg.usuario, nombre_de_la_compu()),
        })
    }

    pub fn terminar(&mut self) {
        if let Some(mut p) = self.proc.take() {
            drop(p.stdin.take());
            if p.try_wait().ok().flatten().is_none() {
                matar_grupo(&p, false);
                if !esperar_con_limite(&mut p, Duration::from_secs(5)) {
                    matar_grupo(&p, true);
                    esperar_con_limite(&mut p, Duration::from_secs(5));
                }
            }
            let _ = p.wait();
        }
        let _ = std::fs::remove_dir_all(&self.cwd);
        if let Some(l) = self.lote.take() {
            let _ = std::fs::remove_dir_all(l);
        }
    }
}

impl Drop for Envio {
    fn drop(&mut self) {
        self.terminar();
    }
}

fn buscar_ticket(linea: &str) -> Option<String> {
    let mut resto = linea;
    while let Some(i) = resto.find("blob") {
        let antes_ok = resto[..i]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'));
        let cola: String = resto[i + 4..]
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .collect();
        if antes_ok && !cola.is_empty() {
            return Some(format!("blob{cola}"));
        }
        resto = &resto[i + 4..];
    }
    None
}

/// El nombre de esta compu.
pub fn nombre_de_la_compu() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .or_else(|_| std::fs::read_to_string("/etc/hostname"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Lo que dice el destino: una IP o un nombre, sin barras ni espacios.
pub fn destino_de(host: &str, puerto: u16) -> Result<(String, u16), ErrorShared> {
    let host = host.trim().trim_matches(['[', ']']);
    if host.is_empty() || host.chars().any(|c| c.is_whitespace() || c == '/') {
        return error("Escribe la IP o el nombre de la otra compu.");
    }
    let hex = |s: &str| s.len() == 2 && s.bytes().all(|b| b.is_ascii_hexdigit());
    let es_mac = |sep: char| {
        let p: Vec<&str> = host.split(sep).collect();
        p.len() == 6 && p.iter().all(|x| hex(x))
    };
    if es_mac(':') || es_mac('-') {
        return error(
            "Con una MAC no se puede llegar a otra compu: escribe su IP o su nombre en Tailscale.",
        );
    }
    Ok((host.to_string(), puerto))
}

/// Avisa a la otra compu y espera a que baje todo. `ErrorShared` si no se pudo, con el motivo.
/// `al_estado(fase, bytes)`: fase es «esperando» o «mandando».
pub fn mandar(
    cfg: &Config,
    host: &str,
    usuario: &str,
    envio: &Envio,
    al_estado: &mut dyn FnMut(&str, u64),
    cancelado: &dyn Fn() -> bool,
    espera: Duration,
) -> Result<(), ErrorShared> {
    if !usuario_valido(usuario.trim()) {
        return error("Escribe el usuario de la otra compu.");
    }
    let (nombre, puerto) = destino_de(host, cfg.puerto)?;
    let direcciones: Vec<_> = (nombre.as_str(), puerto)
        .to_socket_addrs()
        .map_err(|_| ErrorShared(format!("No encuentro «{host}». Revisa la IP o el nombre.")))?
        .collect();
    let mut conexion = direcciones
        .iter()
        .find_map(|d| TcpStream::connect_timeout(d, Duration::from_secs(8)).ok())
        .ok_or_else(|| {
            ErrorShared(format!(
                "«{host}» no contesta. Si está lejos, usen Tailscale o un código."
            ))
        })?;
    let cortada = || ErrorShared("Se cortó la conexión con la otra compu.".into());
    let _ = conexion.set_read_timeout(Some(Duration::from_secs(1)));
    let mut linea = envio.peticion(usuario.trim()).to_string();
    linea.push('\n');
    conexion.write_all(linea.as_bytes()).map_err(|_| cortada())?;
    al_estado("esperando", 0);
    let mut ultimo = Instant::now();
    let mut limite = espera;
    let mut buffer: Vec<u8> = Vec::new();
    loop {
        if cancelado() {
            return error("Cancelado.");
        }
        if ultimo.elapsed() > limite {
            return Err(cortada());
        }
        let mut trozo = [0u8; 4096];
        match conexion.read(&mut trozo) {
            Ok(0) => return Err(cortada()),
            Ok(n) => buffer.extend_from_slice(&trozo[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                continue;
            }
            Err(_) => return Err(cortada()),
        }
        while let Some(i) = buffer.iter().position(|b| *b == b'\n') {
            let resto = buffer.split_off(i + 1);
            let linea = std::mem::replace(&mut buffer, resto);
            let r: Value = serde_json::from_slice(&linea[..i]).map_err(|_| {
                ErrorShared("La otra compu contestó algo raro. ¿Tiene la misma versión?".into())
            })?;
            let Some(respuesta) = r.get("r").and_then(Value::as_str) else {
                return error("La otra compu contestó algo raro. ¿Tiene la misma versión?");
            };
            ultimo = Instant::now();
            match respuesta {
                "aceptado" => {
                    limite = cfg.sin_avance + Duration::from_secs(30);
                    al_estado("mandando", 0);
                }
                "avance" => al_estado("mandando", r.get("bytes").and_then(Value::as_u64).unwrap_or(0)),
                "listo" => return Ok(()),
                otra => return error(mensaje(otra)),
            }
        }
    }
}

// --- Recibir ---

fn sin_pisar(destino: &Path) -> PathBuf {
    if !destino.exists() {
        return destino.to_path_buf();
    }
    let nombre = destino.file_name().unwrap_or_default().to_string_lossy().into_owned();
    let (stem, ext) = match destino.extension() {
        Some(e) => (
            destino.file_stem().unwrap_or_default().to_string_lossy().into_owned(),
            format!(".{}", e.to_string_lossy()),
        ),
        None => (nombre, String::new()),
    };
    let mut n = 2;
    loop {
        let otro = destino.with_file_name(format!("{stem} ({n}){ext}"));
        if !otro.exists() {
            return otro;
        }
        n += 1;
    }
}

/// Baja todo a la carpeta de recibidos y devuelve lo que quedó. Nunca pisa algo que ya tengas.
pub fn recibir(
    cfg: &Config,
    ticket: &str,
    mut al_avance: Option<&mut dyn FnMut(u64)>,
    cancelado: &dyn Fn() -> bool,
    tam: u64,
) -> Result<Vec<PathBuf>, ErrorShared> {
    let ticket = ticket.trim();
    if !ticket_valido(ticket) {
        return error("Ese código no parece bueno: cópialo completo.");
    }
    if cfg.sistema.buscar("sendme").is_none() {
        return error("Falta sendme (sudo pacman -S sendme).");
    }
    let final_ = carpeta_recibidos(cfg);
    let sin_escribir = || ErrorShared(format!("No se pudo escribir en {}.", final_.display()));
    std::fs::create_dir_all(&final_).map_err(|_| sin_escribir())?;
    let temporal = tempfile::Builder::new()
        .prefix(".recibiendo-")
        .tempdir_in(&final_)
        .map_err(|_| sin_escribir())?
        .keep();
    let mut proc: Option<Child> = None;
    let resultado = (|| -> Result<Vec<PathBuf>, ErrorShared> {
        let hijo = lanzar_sendme(cfg, &["receive", ticket], &temporal, Stdio::null())
            .map_err(|e| ErrorShared(format!("No se pudo preparar el envío: {}", io(&e))))?;
        let p = proc.insert(hijo);
        let (mut ultimo_peso, mut ultimo_cambio) = (u64::MAX, Instant::now());
        let estado = loop {
            match p.try_wait() {
                Ok(Some(e)) => break e,
                Ok(None) => {}
                Err(_) => return error("No se pudo bajar: revisa el código y que la otra compu siga mandando."),
            }
            std::thread::sleep(Duration::from_millis(500));
            if cancelado() {
                return error("Cancelado.");
            }
            let peso = peso_de(&temporal, false);
            if peso != ultimo_peso {
                ultimo_peso = peso;
                ultimo_cambio = Instant::now();
                if let Some(f) = al_avance.as_mut() {
                    // al final sendme tiene su copia y la exportada: no pasarse del total
                    f(if tam > 0 { peso.min(tam) } else { peso });
                }
            } else if ultimo_cambio.elapsed() > cfg.sin_avance {
                return error("No llega nada: la otra compu se apagó o cerró el envío.");
            }
        };
        if !estado.success() {
            return error("No se pudo bajar: revisa el código y que la otra compu siga mandando.");
        }
        let mut items: Vec<PathBuf> = std::fs::read_dir(&temporal)
            .map_err(|_| sin_escribir())?
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                !p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(".sendme-"))
            })
            .collect();
        if items.len() == 1 && items[0].file_name().is_some_and(|n| n == LOTE) && items[0].is_dir() {
            let lote = items.remove(0);
            items = std::fs::read_dir(&lote)
                .map_err(|_| sin_escribir())?
                .flatten()
                .map(|e| e.path())
                .collect();
        }
        if items.is_empty() {
            return error("No llegó nada.");
        }
        items.sort();
        let mut quedaron = Vec::new();
        for item in items {
            let destino = sin_pisar(&final_.join(item.file_name().unwrap_or_default()));
            std::fs::rename(&item, &destino).map_err(|_| sin_escribir())?;
            quedaron.push(destino);
        }
        Ok(quedaron)
    })();
    if let Some(mut p) = proc {
        drop(p.stdin.take());
        if p.try_wait().ok().flatten().is_none() {
            matar_grupo(&p, true);
            esperar_con_limite(&mut p, Duration::from_secs(5));
        }
        let _ = p.wait();
    }
    let _ = std::fs::remove_dir_all(&temporal);
    resultado
}

// --- Las direcciones ---

/// Lo que dice `tailscale status --json` (o `{}` si no hay Tailscale o no contesta).
pub fn tailscale_estado(cfg: &Config) -> Value {
    if cfg.sistema.buscar("tailscale").is_none() {
        return json!({});
    }
    let r = cfg
        .sistema
        .correr(&["tailscale", "status", "--json"], Duration::from_secs(4));
    if !r.bien() {
        return json!({});
    }
    serde_json::from_str(&r.salida).unwrap_or_else(|_| json!({}))
}

fn ipv4(lista: &Value) -> String {
    lista
        .as_array()
        .and_then(|l| l.iter().filter_map(Value::as_str).find(|x| !x.contains(':')))
        .unwrap_or("")
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Direccion {
    pub tipo: String,
    pub ip: String,
}

/// Lo que le das a otra persona para que te mande algo. `local`: la IP de la interfaz de salida.
pub fn mis_direcciones(estado: &Value, local: Option<String>) -> Vec<Direccion> {
    let mut d = Vec::new();
    let ip = ipv4(&estado["Self"]["TailscaleIPs"]);
    if !ip.is_empty() {
        d.push(Direccion {
            tipo: "Tailscale (de lejos)".into(),
            ip: ip.clone(),
        });
    }
    if let Some(l) = local.filter(|l| !l.starts_with("127.") && *l != ip) {
        d.push(Direccion {
            tipo: "Misma red".into(),
            ip: l,
        });
    }
    d
}

/// La IP que usaría esta compu para salir (no manda nada: solo pregunta qué interfaz usaría).
pub fn ip_local() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Compu {
    pub nombre: String,
    pub ip: String,
}

/// Tus otras compus con Linux prendidas en Tailscale, para mandarles con un clic.
pub fn compus_tailscale(estado: &Value) -> Vec<Compu> {
    let mut compus: Vec<Compu> = estado["Peer"]
        .as_object()
        .into_iter()
        .flat_map(|m| m.values())
        .filter(|p| p["Online"].as_bool() == Some(true) && p["OS"].as_str() == Some("linux"))
        .filter_map(|p| {
            let ip = ipv4(&p["TailscaleIPs"]);
            (!ip.is_empty()).then(|| Compu {
                nombre: limpiar_nombre(p["HostName"].as_str().filter(|h| !h.is_empty()).unwrap_or(&ip)),
                ip,
            })
        })
        .collect();
    compus.sort_by_key(|c| c.nombre.to_lowercase());
    compus
}

// --- El cortafuegos ---

fn marca_puerto(cfg: &Config) -> PathBuf {
    cfg.config.join("vortex").join("shared-ufw")
}

/// ¿El cortafuegos está prendido y todavía no se abrió el puerto? Sin eso, recibir no funciona.
pub fn puerto_cerrado(cfg: &Config) -> bool {
    if marca_puerto(cfg).exists() || cfg.sistema.buscar("ufw").is_none() {
        return false;
    }
    cfg.sistema
        .codigo(&["systemctl", "is-active", "--quiet", "ufw"], Duration::from_secs(5))
        == 0
}

/// Con ufw prendido, se abre el 1770 una sola vez y solo para Tailscale y redes privadas (pide la
/// contraseña). Devuelve qué pasó: «listo» o «falta».
pub fn abrir_puerto(cfg: &Config) -> &'static str {
    if !puerto_cerrado(cfg) {
        return "listo";
    }
    let p = format!("to any port {} proto tcp", cfg.puerto);
    let orden = format!(
        "ufw allow in on tailscale0 {p} && ufw allow from 192.168.0.0/16 {p} && ufw allow from 10.0.0.0/8 {p} && ufw allow from 172.16.0.0/12 {p}"
    );
    let r = cfg
        .sistema
        .correr(&["pkexec", "sh", "-c", &orden], Duration::from_secs(600));
    if !r.bien() {
        return "falta";
    }
    let marca = marca_puerto(cfg);
    if let Some(p) = marca.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::File::create(marca);
    "listo"
}

/// Lo que queda libre en el disco de esa carpeta (0 si no se puede saber).
pub fn espacio_libre(carpeta: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(carpeta.as_os_str().as_bytes()) else {
        return 0;
    };
    let mut s = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `c` es una cadena C válida y `s` tiene el tamaño de un statvfs; solo se lee si dio 0.
    unsafe {
        if libc::statvfs(c.as_ptr(), s.as_mut_ptr()) != 0 {
            return 0;
        }
        let s = s.assume_init();
        (s.f_bavail as u64).saturating_mul(s.f_frsize as u64)
    }
}
