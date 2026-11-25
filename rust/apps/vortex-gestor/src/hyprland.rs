//! El oído del gestor (lo que hacía `src/vortex-eventos.c`, ahora dentro del mismo proceso): escucha el
//! socket de eventos de Hyprland y la presión de memoria del kernel (PSI) y los convierte en `Evento`.
//!
//! Nada despierta la CPU: todo es poll() sobre descriptores, y solo hay un plazo mientras se espera a que
//! el foco se quede quieto (antirrebote: pasar el ratón por encima de varias ventanas no genera trabajo).

use crate::sis;
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    /// Ventana enfocada (pid 0 si no hay ninguna). La dirección va sin «0x».
    Foco {
        pid: u32,
        dir: String,
        tipo: String,
        clase: String,
    },
    Abre {
        dir: String,
        tipo: String,
        clase: String,
    },
    Cierra {
        dir: String,
    },
    /// La presión de memoria pasó el umbral.
    PsiMemoria,
    /// No se puede vigilar la presión.
    PsiNo(String),
    /// Se empezó (true) o se dejó (false) de compartir la pantalla o una ventana (Discord, OBS, el
    /// navegador…): Hyprland lo avisa con `screencast>>ESTADO,DUEÑO` (4.4).
    Compartir(bool),
}

// ---------------------------------------------------------------- la lectura de los JSON de Hyprland

/// Lo que sigue a `"clave":` entre `desde` y `fin` (índice del valor), saltando espacios.
fn valor(b: &[u8], desde: usize, fin: usize, clave: &str) -> Option<usize> {
    if desde >= fin {
        return None;
    }
    let patron = format!("\"{clave}\":");
    let p = patron.as_bytes();
    let hallado = b[desde..fin].windows(p.len()).position(|w| w == p)?;
    let mut q = desde + hallado + p.len();
    while q < fin && matches!(b[q], b' ' | b'\t' | b'\n' | b'\r') {
        q += 1;
    }
    (q < fin).then_some(q)
}

/// Copia un valor de texto JSON sin comillas (los saltos y tabuladores pasan a espacios) con tope de
/// `tam - 1` bytes, igual que el ayudante en C.
fn texto(b: &[u8], v: Option<usize>, fin: usize, tam: usize) -> String {
    let mut sal: Vec<u8> = Vec::new();
    if let Some(mut v) = v.filter(|&v| b[v] == b'"') {
        v += 1;
        while v < fin && b[v] != b'"' && sal.len() + 1 < tam {
            let mut c = b[v];
            if c == b'\\' && v + 1 < fin {
                v += 1;
                c = b[v];
                if c == b'u' {
                    v += if fin - v > 4 { 4 } else { 0 };
                    c = b'?';
                } else if matches!(c, b'n' | b't' | b'r' | b'b' | b'f') {
                    c = b' ';
                }
            }
            if c < 0x20 {
                c = b' ';
            }
            sal.push(c);
            v += 1;
        }
    }
    String::from_utf8_lossy(&sal).into_owned()
}

/// strtol(v, NULL, 10).
fn entero(b: &[u8], v: Option<usize>) -> i64 {
    let Some(mut i) = v else { return 0 };
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let neg = i < b.len() && b[i] == b'-';
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut n: i64 = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        n = n.saturating_mul(10).saturating_add(i64::from(b[i] - b'0'));
        i += 1;
    }
    if neg { -n } else { n }
}

fn sin_0x(d: &str) -> &str {
    d.strip_prefix("0x").unwrap_or(d)
}

/// El foco que dice la respuesta de `j/activewindow`.
pub fn foco_de(resp: &[u8]) -> Evento {
    let fin = resp.len();
    let pid = entero(resp, valor(resp, 0, fin, "pid"));
    if pid > 0 {
        let dir = texto(resp, valor(resp, 0, fin, "address"), fin, 64);
        let tipo = texto(resp, valor(resp, 0, fin, "contentType"), fin, 64);
        let clase = texto(resp, valor(resp, 0, fin, "class"), fin, 256);
        Evento::Foco {
            pid: u32::try_from(pid).unwrap_or(0),
            dir: sin_0x(&dir).to_string(),
            tipo,
            clase,
        }
    } else {
        Evento::Foco {
            pid: 0,
            dir: String::new(),
            tipo: String::new(),
            clase: String::new(),
        }
    }
}

/// Una `Abre` por cada ventana de la respuesta de `j/clients`.
pub fn abiertas_de(resp: &[u8]) -> Vec<Evento> {
    let fin = resp.len();
    let mut sal = Vec::new();
    // «address» es la primera clave de cada ventana: lo que va hasta la siguiente es suyo.
    let mut p = valor(resp, 0, fin, "address");
    while let Some(ini) = p {
        let sig = valor(resp, ini, fin, "address");
        let tope = sig.unwrap_or(fin);
        let dir = texto(resp, Some(ini), tope, 64);
        let tipo = texto(resp, valor(resp, ini, tope, "contentType"), tope, 64);
        let clase = texto(resp, valor(resp, ini, tope, "class"), tope, 256);
        if !dir.is_empty() {
            sal.push(Evento::Abre {
                dir: sin_0x(&dir).to_string(),
                tipo,
                clase,
            });
        }
        p = sig;
    }
    sal
}

// ---------------------------------------------------------------- el oído

pub struct Oido {
    eventos: UnixStream,
    socket_ordenes: std::path::PathBuf,
    /// El descriptor del aviso de presión (o ninguno).
    presion: Option<std::fs::File>,
    inyectado: Option<std::fs::File>,
    antirrebote: Duration,
    /// Hay un cambio de foco esperando a que pase el antirrebote, y cuándo informarlo.
    pendiente: Option<Instant>,
    /// La ventana del último evento de foco mientras se espera.
    pedida: String,
    /// La dirección de la última ventana enfocada que se informó.
    ultima: String,
    buf: Vec<u8>,
    respuesta: Vec<u8>,
}

/// Lo que da cada vuelta de espera.
pub enum Vuelta {
    Eventos(Vec<Evento>),
    /// Se cerró el socket de Hyprland: la sesión terminó.
    Cerrado,
    /// Nada de Hyprland: despertó otro descriptor (señales) o se cumplió el plazo pedido.
    Nada,
}

fn sin_os_error(e: &std::io::Error) -> String {
    let t = e.to_string();
    match t.find(" (os error") {
        Some(i) => t[..i].to_string(),
        None => t,
    }
}

impl Oido {
    /// Conecta con Hyprland y registra el aviso de presión. Devuelve el oído y los eventos de arranque
    /// (la presión que no se pudo vigilar y una `Abre` por cada ventana ya abierta).
    #[allow(clippy::too_many_arguments)]
    pub fn conectar(
        socket_eventos: &Path,
        socket_ordenes: &Path,
        psi: Option<&Path>,
        umbral_us: i64,
        ventana_us: i64,
        antirrebote_ms: i64,
        inyectar: Option<&Path>,
    ) -> std::io::Result<(Oido, Vec<Evento>)> {
        let eventos = UnixStream::connect(socket_eventos)?;
        let mut arranque = Vec::new();
        let mut presion = None;
        if let Some(psi) = psi {
            // El kernel avisa (POLLPRI) cuando, dentro de una ventana, las tareas pasaron más de UMBRAL
            // esperando memoria. Un aviso por ventana como mucho.
            let disparador = format!("some {umbral_us} {ventana_us}\0");
            let abierto = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(psi)
                .and_then(|mut f| f.write_all(disparador.as_bytes()).map(|()| f));
            match abierto {
                Ok(f) => presion = Some(f),
                Err(e) => arranque.push(Evento::PsiNo(sin_os_error(&e))),
            }
        }
        let inyectado = inyectar.and_then(|p| {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
                .open(p)
                .ok()
        });
        let mut o = Oido {
            eventos,
            socket_ordenes: socket_ordenes.to_path_buf(),
            presion,
            inyectado,
            antirrebote: Duration::from_millis(antirrebote_ms.max(0) as u64),
            pendiente: None,
            pedida: String::new(),
            ultima: String::new(),
            buf: Vec::new(),
            respuesta: Vec::new(),
        };
        o.pedir("j/clients");
        arranque.extend(abiertas_de(&o.respuesta));
        o.respuesta = Vec::new(); // la lista de ventanas puede ser grande: no se queda en memoria
        o.pendiente = Some(Instant::now()); // y la ventana enfocada ahora mismo
        Ok((o, arranque))
    }

    /// Pregunta algo a Hyprland por el socket de órdenes y lee la respuesta entera (Hyprland cierra la
    /// conexión al terminar). Con tope de 2 s para no quedarse colgado nunca.
    fn pedir(&mut self, orden: &str) {
        self.respuesta.clear();
        let Ok(mut s) = UnixStream::connect(&self.socket_ordenes) else {
            return;
        };
        let tope = Some(Duration::from_secs(2));
        let _ = s.set_read_timeout(tope);
        let _ = s.set_write_timeout(tope);
        if s.write_all(orden.as_bytes()).is_err() {
            return;
        }
        let _ = s.take(512 * 1024 - 1).read_to_end(&mut self.respuesta);
    }

    fn informar_foco(&mut self) -> Evento {
        self.pedir("j/activewindow");
        let ev = foco_de(&self.respuesta);
        if self.respuesta.capacity() > 16 * 1024 {
            self.respuesta = Vec::new();
        }
        if let Evento::Foco { dir, .. } = &ev {
            self.ultima.clone_from(dir);
        }
        ev
    }

    /// Una línea del socket de eventos.
    fn evento(&mut self, l: &str, sal: &mut Vec<Evento>) {
        if let Some(r) = l.strip_prefix("activewindowv2>>") {
            let dir = sin_0x(r);
            // Hyprland repite el evento cuando cambia el título de la ventana enfocada:
            // si es la misma ventana no hay nada que hacer (ni hay que alargar la espera).
            let vigente = if self.pendiente.is_some() {
                &self.pedida
            } else {
                &self.ultima
            };
            if dir == vigente {
                return;
            }
            // El plazo se corre con cada ventana distinta: se informa cuando el foco se queda quieto.
            self.pendiente = Some(Instant::now() + self.antirrebote);
            self.pedida = dir.chars().take(63).collect();
        } else if let Some(r) = l.strip_prefix("openwindow>>") {
            // DIRECCION,ESPACIO,CLASE,TITULO
            let Some((dir, resto)) = r.split_once(',') else {
                return;
            };
            let Some((_, clase)) = resto.split_once(',') else {
                return;
            };
            let clase = clase.split(',').next().unwrap_or("");
            let clase: String = clase.chars().map(|c| if c < ' ' { ' ' } else { c }).collect();
            sal.push(Evento::Abre {
                dir: sin_0x(dir).to_string(),
                tipo: String::new(),
                clase,
            });
        } else if let Some(r) = l.strip_prefix("closewindow>>") {
            let dir = sin_0x(r);
            // Se cerró la enfocada: lo que venga después (aunque sea «ninguna») se informa.
            if dir == self.ultima {
                self.ultima = "?".into();
            }
            sal.push(Evento::Cierra { dir: dir.to_string() });
        } else if let Some(r) = l.strip_prefix("screencast>>") {
            // ESTADO,DUEÑO: 1 = empezó, 0 = terminó (el dueño dice si es un monitor o una ventana).
            match r.split(',').next() {
                Some("1") => sal.push(Evento::Compartir(true)),
                Some("0") => sal.push(Evento::Compartir(false)),
                _ => {}
            }
        }
    }

    /// Cuánto falta para informar el foco (None = nada pendiente).
    pub fn plazo(&self) -> Option<Instant> {
        self.pendiente
    }

    pub fn fd_eventos(&self) -> i32 {
        self.eventos.as_raw_fd()
    }

    pub fn fd_presion(&self) -> Option<i32> {
        self.presion.as_ref().map(AsRawFd::as_raw_fd)
    }

    pub fn fd_inyectado(&self) -> Option<i32> {
        self.inyectado.as_ref().map(AsRawFd::as_raw_fd)
    }

    /// Procesa lo que dejó poll(): `rev_eventos`, `rev_presion` y `rev_inyectado` son sus `revents`.
    pub fn atender(&mut self, rev_eventos: i16, rev_presion: i16, rev_inyectado: i16) -> Vuelta {
        let mut sal = Vec::new();
        if rev_eventos != 0 {
            let mut tmp = [0u8; 16 * 1024];
            match self.eventos.read(&mut tmp) {
                Ok(0) => return Vuelta::Cerrado,
                Ok(n) => {
                    self.buf.extend_from_slice(&tmp[..n]);
                    while let Some(nl) = self.buf.iter().position(|&c| c == b'\n') {
                        let linea: Vec<u8> = self.buf.drain(..=nl).collect();
                        let l = String::from_utf8_lossy(&linea[..nl]).into_owned();
                        self.evento(&l, &mut sal);
                    }
                    if self.buf.len() >= 64 * 1024 - 1 {
                        self.buf.clear(); // línea absurda: se descarta
                    }
                }
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::Interrupted | std::io::ErrorKind::WouldBlock
                    ) => {}
                Err(_) => return Vuelta::Cerrado,
            }
        }
        if self.presion.is_some() && rev_presion != 0 {
            if rev_presion & (libc::POLLERR | libc::POLLNVAL) != 0 {
                sal.push(Evento::PsiNo("el kernel cerró el aviso".into()));
                self.presion = None;
            } else if rev_presion & libc::POLLPRI != 0 {
                sal.push(Evento::PsiMemoria);
            }
        }
        if rev_inyectado != 0 {
            self.leer_inyectado(&mut sal);
        }
        if self.pendiente.is_some_and(|p| Instant::now() >= p) {
            self.pendiente = None;
            sal.push(self.informar_foco());
        }
        if sal.is_empty() {
            Vuelta::Nada
        } else {
            Vuelta::Eventos(sal)
        }
    }

    /// Pruebas: líneas `psi\037memoria` o `psi\037no\037MOTIVO` en un FIFO.
    fn leer_inyectado(&mut self, sal: &mut Vec<Evento>) {
        let Some(f) = self.inyectado.as_mut() else {
            return;
        };
        let mut tmp = [0u8; 4096];
        let Ok(n) = f.read(&mut tmp) else { return };
        for l in String::from_utf8_lossy(&tmp[..n]).lines() {
            let mut c = l.split('\u{1f}');
            if c.next() != Some("psi") {
                continue;
            }
            match c.next() {
                Some("memoria") => sal.push(Evento::PsiMemoria),
                Some("no") => sal.push(Evento::PsiNo(c.next().unwrap_or("").to_string())),
                _ => {}
            }
        }
    }

    /// Espera hasta que haya algo (o hasta `limite`) y lo procesa. `senales` se vigila también:
    /// si despierta, devuelve `Nada` sin tocar nada para que quien llama lo lea.
    pub fn esperar(&mut self, senales: &impl AsRawFd, limite: Option<Instant>) -> (Vuelta, bool) {
        let ahora = Instant::now();
        let mut hasta = limite;
        if let Some(p) = self.pendiente {
            hasta = Some(hasta.map_or(p, |h| h.min(p)));
        }
        let ms = match hasta {
            None => -1,
            Some(h) => {
                let d = h.saturating_duration_since(ahora);
                // redondeado hacia arriba: nunca se despierta antes del plazo
                i32::try_from(d.as_micros().div_ceil(1000)).unwrap_or(i32::MAX)
            }
        };
        let mut fds = vec![
            sis::pollfd(self.fd_eventos(), libc::POLLIN),
            sis::pollfd(senales.as_raw_fd(), libc::POLLIN),
        ];
        let ip = self.fd_presion().map(|fd| {
            fds.push(sis::pollfd(fd, libc::POLLPRI));
            fds.len() - 1
        });
        let ii = self.fd_inyectado().map(|fd| {
            fds.push(sis::pollfd(fd, libc::POLLIN));
            fds.len() - 1
        });
        if sis::sondear(&mut fds, ms).is_err() {
            return (Vuelta::Nada, false); // EINTR
        }
        let hubo_senal = fds[1].revents != 0;
        let rp = ip.map_or(0, |i| fds[i].revents);
        let ri = ii.map_or(0, |i| fds[i].revents);
        let v = self.atender(fds[0].revents, rp, ri);
        (v, hubo_senal)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    const ACTIVA: &str = "{\n    \"address\": \"0xaaa1\",\n    \"class\": \"kitty\",\n    \"title\": \"t\",\n    \"pid\": 4001,\n    \"contentType\": \"none\"\n}\n";

    #[test]
    fn foco_con_pid_direccion_tipo_y_clase() {
        assert_eq!(
            foco_de(ACTIVA.as_bytes()),
            Evento::Foco {
                pid: 4001,
                dir: "aaa1".into(),
                tipo: "none".into(),
                clase: "kitty".into()
            }
        );
    }

    #[test]
    fn sin_ventana_pid_cero() {
        for r in ["{}", "", "{\"pid\": -1}"] {
            assert_eq!(
                foco_de(r.as_bytes()),
                Evento::Foco {
                    pid: 0,
                    dir: String::new(),
                    tipo: String::new(),
                    clase: String::new()
                },
                "{r}"
            );
        }
    }

    #[test]
    fn ventanas_abiertas() {
        let j = r#"[{
    "address": "0xaaa1",
    "class": "kitty",
    "contentType": "none",
    "grouped": ["0xaaa1"],
    "swallowing": "0x0"
},{
    "address": "0xeee5",
    "class": "steam_app_1",
    "contentType": "game"
}]"#;
        let v = abiertas_de(j.as_bytes());
        assert_eq!(
            v,
            [
                Evento::Abre {
                    dir: "aaa1".into(),
                    tipo: "none".into(),
                    clase: "kitty".into()
                },
                Evento::Abre {
                    dir: "eee5".into(),
                    tipo: "game".into(),
                    clase: "steam_app_1".into()
                }
            ]
        );
        assert!(abiertas_de(b"[]").is_empty());
    }

    #[test]
    fn texto_con_escapes_y_tope() {
        let j = br#"{"class": "a\"b\tc\u00e9d"}"#;
        let v = valor(j, 0, j.len(), "class");
        assert_eq!(texto(j, v, j.len(), 256), "a\"b c?d");
        assert_eq!(texto(j, v, j.len(), 4), "a\"b");
    }
}
