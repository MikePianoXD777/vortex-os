//! Un cliente HTTP/1.1 mínimo y sin crates: lo que Ollama necesita. Contesta JSON de una
//! vez o línea por línea (streaming, para bajar modelos y para el chat), entiende `Content-Length`,
//! `Transfer-Encoding: chunked` y las respuestas que se cierran al terminar (HTTP/1.0).
//!
//! Cancelar corta de verdad: las lecturas esperan como mucho 100 ms, así que un servidor callado no
//! aguanta el «cancelar» hasta el siguiente trozo (lo mismo que hace la de Python cortando el socket).
//!
//! `https://` (un Ollama detrás de un proxy con certificado) va por `openssl s_client`, que viene en todo
//! Arch: verifica el certificado con las autoridades del sistema y el nombre del servidor, y el resto es
//! el mismo HTTP por su entrada y su salida. Así no hace falta un crate de TLS.

use std::io::{self, ErrorKind, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// La señal de cancelar (el `threading.Event` de Python): se comparte entre hilos.
#[derive(Clone, Default, Debug)]
pub struct Cancelar(Arc<AtomicBool>);

impl Cancelar {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// A dónde hablarle: `http://host:puerto/base`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destino {
    pub https: bool,
    pub host: String,
    pub puerto: u16,
    /// El camino que va antes de cada ruta (sin «/» al final).
    pub base: String,
}

impl Destino {
    pub fn de_url(url: &str) -> Destino {
        let (https, resto) = match url.split_once("://") {
            Some(("https", r)) => (true, r),
            Some((_, r)) => (false, r),
            None => (false, url),
        };
        let (autoridad, camino) = match resto.find('/') {
            Some(i) => (&resto[..i], &resto[i..]),
            None => (resto, ""),
        };
        let autoridad = autoridad.rsplit('@').next().unwrap_or(autoridad);
        let (host, puerto) = if let Some(r) = autoridad.strip_prefix('[') {
            // [::1]:11434
            match r.split_once(']') {
                Some((h, p)) => (h.to_string(), p.strip_prefix(':').and_then(|p| p.parse().ok())),
                None => (r.to_string(), None),
            }
        } else {
            match autoridad.rsplit_once(':') {
                Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().ok()),
                _ => (autoridad.to_string(), None),
            }
        };
        Destino {
            https,
            host: if host.is_empty() { "127.0.0.1".into() } else { host },
            puerto: puerto.unwrap_or(if https { 443 } else { 11434 }),
            base: camino.trim_end_matches('/').to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrHttp {
    /// No se pudo hablar con el servidor (no contesta, se cortó, tiempo agotado).
    Conexion,
    /// Se canceló con la señal.
    Cancelado,
    /// Contestó algo que no es HTTP.
    Raro,
}

/// La conexión: un socket directo o el `openssl s_client` del https.
enum Conexion {
    Tcp(TcpStream),
    Tls(Tls),
}

/// `openssl s_client`: se le escribe por su entrada y un hilo pasa su salida por un canal, para que leer
/// espere como mucho 100 ms (como el socket).
struct Tls {
    hijo: Child,
    entrada: ChildStdin,
    salida: Receiver<Vec<u8>>,
    /// Lo que llegó y todavía no se leyó.
    resto: Vec<u8>,
}

impl Drop for Tls {
    fn drop(&mut self) {
        let _ = self.hijo.kill();
        let _ = self.hijo.wait();
    }
}

impl Read for Conexion {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Conexion::Tcp(s) => s.read(buf),
            Conexion::Tls(t) => {
                if t.resto.is_empty() {
                    match t.salida.recv_timeout(Duration::from_millis(100)) {
                        Ok(v) => t.resto = v,
                        Err(RecvTimeoutError::Timeout) => return Err(ErrorKind::WouldBlock.into()),
                        // El hilo terminó: openssl cerró (el servidor colgó o falló el certificado).
                        Err(RecvTimeoutError::Disconnected) => return Ok(0),
                    }
                }
                let n = buf.len().min(t.resto.len());
                buf[..n].copy_from_slice(&t.resto[..n]);
                t.resto.drain(..n);
                Ok(n)
            }
        }
    }
}

impl Write for Conexion {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Conexion::Tcp(s) => s.write(buf),
            Conexion::Tls(t) => t.entrada.write(buf),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Conexion::Tcp(s) => s.flush(),
            Conexion::Tls(t) => t.entrada.flush(),
        }
    }
}

/// El programa de openssl (`VORTEX_OPENSSL` para las pruebas).
fn openssl() -> String {
    std::env::var("VORTEX_OPENSSL")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "openssl".into())
}

fn conectar_tls(d: &Destino) -> Result<Conexion, ErrHttp> {
    // El nombre va sin corchetes para SNI y para comprobar el certificado; `-connect` los quiere con ellos.
    let destino = if d.host.contains(':') {
        format!("[{}]:{}", d.host, d.puerto)
    } else {
        format!("{}:{}", d.host, d.puerto)
    };
    let mut hijo = Command::new(openssl())
        .args(["s_client", "-quiet", "-connect", &destino])
        .args(["-servername", &d.host, "-verify_hostname", &d.host])
        // Sin `-verify`, s_client solo informa un certificado malo y sigue: así corta.
        .args(["-verify", "9", "-verify_return_error"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| ErrHttp::Conexion)?;
    let (Some(entrada), Some(mut salida)) = (hijo.stdin.take(), hijo.stdout.take()) else {
        let _ = hijo.kill();
        let _ = hijo.wait();
        return Err(ErrHttp::Conexion);
    };
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16 * 1024];
        loop {
            match salida.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        return;
                    }
                }
            }
        }
    });
    Ok(Conexion::Tls(Tls {
        hijo,
        entrada,
        salida: rx,
        resto: Vec::new(),
    }))
}

fn conectar(d: &Destino, espera: Duration) -> Result<Conexion, ErrHttp> {
    if d.https {
        return conectar_tls(d);
    }
    let direccion = (d.host.as_str(), d.puerto)
        .to_socket_addrs()
        .map_err(|_| ErrHttp::Conexion)?
        .next()
        .ok_or(ErrHttp::Conexion)?;
    let s = TcpStream::connect_timeout(&direccion, espera).map_err(|_| ErrHttp::Conexion)?;
    let _ = s.set_nodelay(true);
    s.set_write_timeout(Some(espera)).map_err(|_| ErrHttp::Conexion)?;
    s.set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| ErrHttp::Conexion)?;
    Ok(Conexion::Tcp(s))
}

/// Lo que va quedando de un trozo («chunk») del cuerpo.
enum Trozo {
    /// Esperando la línea con el tamaño.
    Tamano,
    /// Faltan estos bytes del trozo (y luego su CRLF).
    Datos(usize),
    /// Esperando el CRLF que cierra el trozo.
    Cierre,
    /// El último trozo (tamaño 0): se ignoran los «trailers».
    Fin,
}

/// Una respuesta con el cuerpo todavía llegando.
pub struct Respuesta {
    pub estado: u16,
    s: Conexion,
    crudo: Vec<u8>,
    cuerpo: Vec<u8>,
    chunked: bool,
    largo: Option<usize>,
    recibido: usize,
    trozo: Trozo,
    cerrada: bool,
    inactividad: Duration,
}

impl Respuesta {
    /// Manda la petición y lee los encabezados.
    pub fn enviar(
        d: &Destino,
        metodo: &str,
        ruta: &str,
        cuerpo: Option<&[u8]>,
        inactividad: Duration,
        cancelar: Option<&Cancelar>,
    ) -> Result<Respuesta, ErrHttp> {
        let mut s = conectar(d, inactividad)?;
        let host = if d.host.contains(':') {
            format!("[{}]:{}", d.host, d.puerto)
        } else {
            format!("{}:{}", d.host, d.puerto)
        };
        let mut pedido = format!(
            "{metodo} {}{ruta} HTTP/1.1\r\nHost: {host}\r\nAccept: */*\r\nConnection: close\r\n",
            d.base
        );
        if let Some(c) = cuerpo {
            pedido += &format!(
                "Content-Type: application/json\r\nContent-Length: {}\r\n",
                c.len()
            );
        }
        pedido += "\r\n";
        let mut bytes = pedido.into_bytes();
        if let Some(c) = cuerpo {
            bytes.extend_from_slice(c);
        }
        s.write_all(&bytes).map_err(|_| ErrHttp::Conexion)?;
        let _ = s.flush();
        let mut r = Respuesta {
            estado: 0,
            s,
            crudo: Vec::new(),
            cuerpo: Vec::new(),
            chunked: false,
            largo: None,
            recibido: 0,
            trozo: Trozo::Tamano,
            cerrada: false,
            inactividad,
        };
        r.leer_encabezados(cancelar)?;
        Ok(r)
    }

    /// Lee un poco más del socket. `Ok(false)` si el servidor cerró.
    fn mas(&mut self, cancelar: Option<&Cancelar>, desde: Instant) -> Result<bool, ErrHttp> {
        let mut buf = [0u8; 16 * 1024];
        loop {
            if cancelar.is_some_and(Cancelar::is_set) {
                return Err(ErrHttp::Cancelado);
            }
            match self.s.read(&mut buf) {
                Ok(0) => return Ok(false),
                Ok(n) => {
                    self.crudo.extend_from_slice(&buf[..n]);
                    return Ok(true);
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
                    if desde.elapsed() > self.inactividad {
                        return Err(ErrHttp::Conexion);
                    }
                }
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(_) => return Err(ErrHttp::Conexion),
            }
        }
    }

    fn leer_encabezados(&mut self, cancelar: Option<&Cancelar>) -> Result<(), ErrHttp> {
        let inicio = Instant::now();
        let fin = loop {
            if let Some(i) = self.crudo.windows(4).position(|w| w == b"\r\n\r\n") {
                break i;
            }
            if !self.mas(cancelar, inicio)? {
                return Err(ErrHttp::Conexion);
            }
        };
        let cabeza = String::from_utf8_lossy(&self.crudo[..fin]).to_string();
        self.crudo.drain(..fin + 4);
        let mut lineas = cabeza.lines();
        let primera = lineas.next().unwrap_or("");
        let mut partes = primera.split_whitespace();
        if !partes.next().is_some_and(|v| v.starts_with("HTTP/")) {
            return Err(ErrHttp::Raro);
        }
        self.estado = partes.next().and_then(|e| e.parse().ok()).ok_or(ErrHttp::Raro)?;
        for l in lineas {
            if let Some((k, v)) = l.split_once(':') {
                let (k, v) = (k.trim().to_ascii_lowercase(), v.trim().to_ascii_lowercase());
                if k == "transfer-encoding" && v.contains("chunked") {
                    self.chunked = true;
                } else if k == "content-length" {
                    self.largo = v.parse().ok();
                }
            }
        }
        // Sin cuerpo: 1xx, 204 y 304.
        if self.estado == 204 || self.estado == 304 || self.estado < 200 {
            self.largo = Some(0);
        }
        Ok(())
    }

    /// Pasa lo que haya en `crudo` a `cuerpo` (quitando el formato «chunked»). Dice si ya terminó.
    fn decodificar(&mut self) -> bool {
        if !self.chunked {
            let n = match self.largo {
                Some(l) => (l - self.recibido).min(self.crudo.len()),
                None => self.crudo.len(),
            };
            self.cuerpo.extend(self.crudo.drain(..n));
            self.recibido += n;
            return self.largo.is_some_and(|l| self.recibido >= l);
        }
        loop {
            match self.trozo {
                Trozo::Tamano => {
                    let Some(i) = self.crudo.windows(2).position(|w| w == b"\r\n") else {
                        return false;
                    };
                    let linea = String::from_utf8_lossy(&self.crudo[..i]).to_string();
                    self.crudo.drain(..i + 2);
                    let hex = linea.split(';').next().unwrap_or("").trim();
                    let n = usize::from_str_radix(hex, 16).unwrap_or(0);
                    self.trozo = if n == 0 { Trozo::Fin } else { Trozo::Datos(n) };
                }
                Trozo::Datos(n) => {
                    if self.crudo.is_empty() {
                        return false;
                    }
                    let t = n.min(self.crudo.len());
                    self.cuerpo.extend(self.crudo.drain(..t));
                    self.trozo = if t == n {
                        Trozo::Cierre
                    } else {
                        Trozo::Datos(n - t)
                    };
                }
                Trozo::Cierre => {
                    if self.crudo.len() < 2 {
                        return false;
                    }
                    self.crudo.drain(..2);
                    self.trozo = Trozo::Tamano;
                }
                Trozo::Fin => return true,
            }
        }
    }

    /// Lee el cuerpo completo.
    pub fn todo(mut self, cancelar: Option<&Cancelar>) -> Result<Vec<u8>, ErrHttp> {
        let mut desde = Instant::now();
        loop {
            if self.decodificar() || self.cerrada {
                return Ok(self.cuerpo);
            }
            if self.mas(cancelar, desde)? {
                desde = Instant::now();
            } else {
                self.cerrada = true;
                // Lo último que llegó junto con el cierre.
                self.decodificar();
                return Ok(self.cuerpo);
            }
        }
    }

    /// La siguiente línea del cuerpo (sin el salto), o `None` si ya se acabó.
    pub fn linea(&mut self, cancelar: Option<&Cancelar>) -> Result<Option<Vec<u8>>, ErrHttp> {
        let mut desde = Instant::now();
        let mut terminado = false;
        loop {
            if !terminado {
                terminado = self.decodificar();
            }
            if let Some(i) = self.cuerpo.iter().position(|b| *b == b'\n') {
                let mut linea: Vec<u8> = self.cuerpo.drain(..=i).collect();
                linea.pop();
                if linea.last() == Some(&b'\r') {
                    linea.pop();
                }
                return Ok(Some(linea));
            }
            if terminado || self.cerrada {
                if self.cuerpo.is_empty() {
                    return Ok(None);
                }
                return Ok(Some(std::mem::take(&mut self.cuerpo)));
            }
            if self.mas(cancelar, desde)? {
                desde = Instant::now();
            } else {
                self.cerrada = true;
                // El servidor cerró: lo que quedó en `crudo` todavía se decodifica.
                if self.decodificar() && self.cuerpo.is_empty() {
                    return Ok(None);
                }
            }
        }
    }
}

/// Una petición de una vez: `(estado, cuerpo)`.
pub fn pedir(
    d: &Destino,
    metodo: &str,
    ruta: &str,
    cuerpo: Option<&[u8]>,
    espera: Duration,
) -> Result<(u16, Vec<u8>), ErrHttp> {
    let r = Respuesta::enviar(d, metodo, ruta, cuerpo, espera, None)?;
    let estado = r.estado;
    Ok((estado, r.todo(None)?))
}
