//! Discord Rich Presence (como `presencia_discord.py`): «Escuchando Vortex Player», con la canción, el artista,
//! la portada y el tiempo.
//!
//! Habla con la app de escritorio de Discord por su socket local. Sin Discord abierto no hace nada y vuelve a
//! intentar cada 15 s mientras suena música. En pausa se borra, y al cerrar la app Discord la quita solo. Todo
//! va en un hilo aparte ([`Presencia`]): la ventana solo le pasa lo que suena ([`Sonando`]) y le avisa cuando
//! algo cambió.

use std::env;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::biblioteca;

pub const CLIENT_ID: &str = "1568275063319437433";
pub const LOGO: &str = "https://raw.githubusercontent.com/MikePianoXD777/vortex-os/main/apps/vortex-player/empaquetado/logo-discord.png";

pub const SALUDO: u32 = 0;
pub const MARCO: u32 = 1;
pub const CIERRE: u32 = 2;
pub const PING: u32 = 3;
pub const PONG: u32 = 4;
pub const ESCUCHANDO: u64 = 2;
/// Discord permite 5 actualizaciones cada 20 s.
pub const ENTRE_ENVIOS: f64 = 4.0;
/// Si el final se movió más que esto, hubo un salto en la canción.
pub const TOLERANCIA_MS: i64 = 3000;

/// Empaqueta un opcode y un JSON como marco IPC de Discord (`<II` little-endian + UTF-8).
pub fn marco(op: u32, datos: &Value) -> Vec<u8> {
    let cuerpo = serde_json::to_vec(datos).unwrap_or_default();
    let len = cuerpo.len() as u32;
    let mut salida = Vec::with_capacity(8 + cuerpo.len());
    salida.extend_from_slice(&op.to_le_bytes());
    salida.extend_from_slice(&len.to_le_bytes());
    salida.extend_from_slice(&cuerpo);
    salida
}

/// Decodifica todos los marcos completos del búfer y devuelve los restantes bytes incompletos.
pub fn leer_marcos(buffer: &[u8]) -> (Vec<(u32, Value)>, Vec<u8>) {
    let mut marcos = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= buffer.len() {
        let op = u32::from_le_bytes([buffer[pos], buffer[pos + 1], buffer[pos + 2], buffer[pos + 3]]);
        let largo =
            u32::from_le_bytes([buffer[pos + 4], buffer[pos + 5], buffer[pos + 6], buffer[pos + 7]]) as usize;
        if pos + 8 + largo > buffer.len() {
            break;
        }
        if let Ok(val) = serde_json::from_slice::<Value>(&buffer[pos + 8..pos + 8 + largo]) {
            marcos.push((op, val));
        }
        pos += 8 + largo;
    }
    (marcos, buffer[pos..].to_vec())
}

/// Posibles sockets IPC locales donde escucha Discord.
pub fn rutas_socket() -> Vec<String> {
    if cfg!(windows) {
        return (0..10).map(|i| format!("discord-ipc-{i}")).collect();
    }
    let raiz = env::var("XDG_RUNTIME_DIR")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/tmp".to_string());
    let carpetas = [
        "",
        "app/com.discordapp.Discord",
        ".flatpak/dev.vencord.Vesktop/xdg-run",
        "snap.discord",
    ];
    let mut salida = Vec::new();
    for c in carpetas {
        for i in 0..10 {
            let ruta = if c.is_empty() {
                Path::new(&raiz).join(format!("discord-ipc-{i}"))
            } else {
                Path::new(&raiz).join(c).join(format!("discord-ipc-{i}"))
            };
            if ruta.exists() {
                salida.push(ruta.to_string_lossy().to_string());
            }
        }
    }
    salida
}

/// Discord pide de 2 a 128 caracteres; con menos o con más rechaza la actividad entera.
pub fn texto(t: &str) -> String {
    let mut limpio: String = t
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(128)
        .collect();
    if limpio.chars().count() == 1 {
        limpio.push('\u{200b}');
    }
    limpio
}

fn campo<'a>(c: &'a Value, k: &str) -> &'a str {
    c.get(k).and_then(Value::as_str).unwrap_or("")
}

/// Lo que se le manda a Discord de la canción `c` (`ahora`: segundos de la época).
pub fn actividad(c: &Value, posicion: f64, duracion: f64, velocidad: f64, ahora: f64) -> Value {
    let ruta = campo(c, "ruta");
    let vid = biblioteca::id_de(ruta);
    let tallo = Path::new(ruta).file_stem().and_then(|s| s.to_str()).unwrap_or("");
    let titulo = [campo(c, "titulo"), tallo, "Vortex Player"]
        .into_iter()
        .find(|t| !t.is_empty())
        .unwrap_or("");
    let album = Some(campo(c, "album"))
        .filter(|t| !t.is_empty())
        .unwrap_or("Vortex Player");
    let mut a = json!({
        "type": ESCUCHANDO,
        "details": texto(titulo),
        "assets": {
            "large_image": if vid.is_empty() { LOGO.to_string() } else { format!("https://i.ytimg.com/vi/{vid}/mqdefault.jpg") },
            "large_text": texto(album),
        }
    });
    let artista = texto(campo(c, "artista"));
    if !artista.is_empty() {
        a["state"] = json!(artista);
    }
    if duracion > 0.0 && velocidad == 1.0 {
        // practicando más lento, el tiempo de Discord estaría mal
        let inicio = ahora - posicion;
        a["timestamps"] =
            json!({"start": (inicio * 1000.0) as i64, "end": ((inicio + duracion) * 1000.0) as i64});
    }
    if !vid.is_empty() {
        a["buttons"] = json!([{"label": "Escuchar en YouTube Music", "url": format!("https://music.youtube.com/watch?v={vid}")}]);
    }
    a
}

fn sin_tiempo(a: &Value) -> Map<String, Value> {
    let mut m = a.as_object().cloned().unwrap_or_default();
    m.remove("timestamps");
    m
}

/// Igual canción y sin saltos: el tiempo avanza solo en Discord, no hace falta volver a mandarla.
pub fn misma(a: &Value, b: Option<&Value>) -> bool {
    let Some(b) = b else { return false };
    let fin = |v: &Value| {
        v.get("timestamps")
            .and_then(|t| t.get("end"))
            .and_then(Value::as_i64)
    };
    match (fin(a), fin(b)) {
        (Some(x), Some(y)) if (x - y).abs() > TOLERANCIA_MS => return false,
        (Some(_), None) | (None, Some(_)) => return false,
        _ => {}
    }
    sin_tiempo(a) == sin_tiempo(b)
}

fn epoca() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

// --- la conexión -------------------------------------------------------------------------------------------

/// Lo que suena ahora (lo pone la ventana; el hilo lo lee al publicar, como la de Python lee el reproductor).
#[derive(Debug, Clone, Default)]
pub struct Sonando {
    pub cancion: Value,
    pub posicion: f64,
    pub duracion: f64,
    pub pausado: bool,
    pub velocidad: f64,
    /// El interruptor de Ajustes.
    pub activa: bool,
}

type Rutas = Arc<dyn Fn() -> Vec<String> + Send + Sync>;

#[derive(Clone)]
pub struct Opciones {
    /// El Application ID («» = no se conecta).
    pub cliente: String,
    pub entre_envios: Duration,
    /// Cuánto se espera después de un aviso (al cambiar de canción llegan varios juntos).
    pub espera: Duration,
    /// Cada cuánto, mientras suena, reconecta y nota los saltos.
    pub pulso: Duration,
    pub rutas: Rutas,
}

impl Default for Opciones {
    fn default() -> Self {
        Self {
            cliente: CLIENT_ID.into(),
            entre_envios: Duration::from_secs_f64(ENTRE_ENVIOS),
            espera: Duration::from_millis(1000),
            pulso: Duration::from_secs(15),
            rutas: Arc::new(rutas_socket),
        }
    }
}

enum Mensaje {
    Actualizar,
    Marco(u64, u32, Value),
    Desconectado(u64),
    Salir,
}

/// La presencia en Discord, en su hilo. Se apaga sola al soltarla.
pub struct Presencia {
    estado: Arc<Mutex<Sonando>>,
    tx: Sender<Mensaje>,
}

impl Presencia {
    pub fn arrancar(op: Opciones) -> Self {
        let estado = Arc::new(Mutex::new(Sonando {
            velocidad: 1.0,
            activa: true,
            ..Sonando::default()
        }));
        let (tx, rx) = mpsc::channel();
        let mut h = Hilo {
            op,
            estado: estado.clone(),
            tx: tx.clone(),
            #[cfg(unix)]
            socket: None,
            generacion: 0,
            conectada: false,
            lista: false,
            ultima: None,
            ultimo_envio: None,
            rebote: None,
            pulso: None,
        };
        let _ = std::thread::Builder::new()
            .name("vortex-player-discord".into())
            .spawn(move || {
                loop {
                    let ahora = Instant::now();
                    let proximo = [h.rebote, h.pulso].into_iter().flatten().min();
                    let m = match proximo {
                        Some(t) => rx.recv_timeout(t.saturating_duration_since(ahora)),
                        None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    };
                    match m {
                        Ok(Mensaje::Salir) | Err(RecvTimeoutError::Disconnected) => break,
                        Ok(Mensaje::Actualizar) => {
                            if h.rebote.is_none() {
                                h.rebote = Some(Instant::now() + h.op.espera);
                            }
                        }
                        Ok(Mensaje::Marco(g, op, datos)) if g == h.generacion => h.al_marco(op, &datos),
                        Ok(Mensaje::Desconectado(g)) if g == h.generacion => h.al_desconectar(),
                        Ok(_) => {}
                        Err(RecvTimeoutError::Timeout) => {
                            let ahora = Instant::now();
                            if h.rebote.is_some_and(|t| t <= ahora) {
                                h.rebote = None;
                                h.publicar();
                            }
                            if h.pulso.is_some_and(|t| t <= ahora) {
                                h.pulso = Some(ahora + h.op.pulso);
                                h.publicar();
                            }
                        }
                    }
                }
                h.cerrar();
            });
        Self { estado, tx }
    }

    /// Lo que suena (barato: llamarlo con cada cambio de posición está bien).
    pub fn poner(&self, s: Sonando) {
        *self.estado.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }

    /// Algo cambió (canción, pausa, el interruptor): se publica en un momento.
    pub fn actualizar(&self) {
        let _ = self.tx.send(Mensaje::Actualizar);
    }
}

impl Drop for Presencia {
    fn drop(&mut self) {
        let _ = self.tx.send(Mensaje::Salir);
    }
}

struct Hilo {
    op: Opciones,
    estado: Arc<Mutex<Sonando>>,
    tx: Sender<Mensaje>,
    #[cfg(unix)]
    socket: Option<std::os::unix::net::UnixStream>,
    /// Cada conexión lleva su número: lo que llega de una vieja se ignora.
    generacion: u64,
    conectada: bool,
    /// Discord ya contestó el saludo.
    lista: bool,
    /// Lo que Discord está mostrando.
    ultima: Option<Value>,
    ultimo_envio: Option<Instant>,
    rebote: Option<Instant>,
    pulso: Option<Instant>,
}

impl Hilo {
    fn sonando(&self) -> Option<Sonando> {
        let s = self.estado.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let hay = s.cancion.as_object().is_some_and(|m| !m.is_empty());
        (!self.op.cliente.is_empty() && hay && !s.pausado && s.activa).then_some(s)
    }

    fn publicar(&mut self) {
        let Some(s) = self.sonando() else {
            self.pulso = None;
            if self.lista && self.ultima.is_some() {
                self.mandar_actividad(None);
            }
            return;
        };
        if self.pulso.is_none() {
            self.pulso = Some(Instant::now() + self.op.pulso);
        }
        if !self.lista {
            self.conectar();
            return;
        }
        let dur = if s.duracion > 0.0 {
            s.duracion
        } else {
            s.cancion.get("duracion").and_then(Value::as_f64).unwrap_or(0.0)
        };
        let a = actividad(&s.cancion, s.posicion, dur, s.velocidad, epoca());
        if !misma(&a, self.ultima.as_ref()) {
            self.mandar_actividad(Some(a));
        }
    }

    fn mandar_actividad(&mut self, a: Option<Value>) {
        if let Some(t) = self.ultimo_envio {
            let listo = t + self.op.entre_envios;
            if listo > Instant::now() {
                self.rebote = Some(listo);
                return;
            }
        }
        let mut args = json!({"pid": std::process::id()});
        if let Some(a) = &a {
            args["activity"] = a.clone();
        }
        let nonce = format!("{:032x}", rand::random::<u128>());
        self.escribir(
            MARCO,
            &json!({"cmd": "SET_ACTIVITY", "args": args, "nonce": nonce}),
        );
        self.ultima = a;
        self.ultimo_envio = Some(Instant::now());
    }

    fn escribir(&mut self, op: u32, datos: &Value) {
        #[cfg(unix)]
        if let Some(s) = self.socket.as_mut() {
            use std::io::Write;
            let _ = s.write_all(&marco(op, datos));
        }
        #[cfg(not(unix))]
        let _ = (op, datos);
    }

    fn conectar(&mut self) {
        #[cfg(unix)]
        {
            use std::io::Read;
            use std::os::unix::net::UnixStream;
            if self.socket.is_some() {
                return;
            }
            // un socket que quedó de un Discord que tronó no contesta: se pasa al siguiente
            let Some(s) = (self.op.rutas)()
                .into_iter()
                .find_map(|r| UnixStream::connect(r).ok())
            else {
                return;
            };
            let Ok(mut lector) = s.try_clone() else { return };
            self.generacion += 1;
            let (g, tx) = (self.generacion, self.tx.clone());
            let _ = std::thread::Builder::new()
                .name("vortex-player-discord-lee".into())
                .spawn(move || {
                    let (mut resto, mut b) = (Vec::new(), [0u8; 4096]);
                    loop {
                        match lector.read(&mut b) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                resto.extend_from_slice(&b[..n]);
                                let (marcos, sobra) = leer_marcos(&resto);
                                resto = sobra;
                                for (op, d) in marcos {
                                    if tx.send(Mensaje::Marco(g, op, d)).is_err() {
                                        return;
                                    }
                                }
                            }
                        }
                    }
                    let _ = tx.send(Mensaje::Desconectado(g));
                });
            self.socket = Some(s);
            self.conectada = true;
            let saludo = json!({"v": 1, "client_id": self.op.cliente});
            self.escribir(SALUDO, &saludo);
        }
    }

    fn al_marco(&mut self, op: u32, datos: &Value) {
        if op == PING {
            self.escribir(PONG, datos);
        } else if op == CIERRE {
            eprintln!(
                "Vortex Player: Discord cerró la conexión: {}",
                datos.get("message").unwrap_or(&Value::Null)
            );
            self.cerrar();
            self.al_desconectar();
        } else if datos.get("evt").and_then(Value::as_str) == Some("READY") {
            self.lista = true;
            self.publicar();
        } else if datos.get("evt").and_then(Value::as_str) == Some("ERROR") {
            eprintln!(
                "Vortex Player: Discord rechazó la actividad: {}",
                datos.get("data").unwrap_or(&Value::Null)
            );
        }
    }

    /// Discord se cerró: el pulso vuelve a buscarlo.
    fn al_desconectar(&mut self) {
        #[cfg(unix)]
        {
            self.socket = None;
        }
        self.generacion += 1;
        self.conectada = false;
        self.lista = false;
        self.ultima = None;
    }

    fn cerrar(&mut self) {
        #[cfg(unix)]
        if let Some(s) = self.socket.take() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
}
