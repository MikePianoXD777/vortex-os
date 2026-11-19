//! Encontrar las cámaras en tu red, para no tener que saber su IP (`descubrir.py`).
//!
//! Las Imou/Dahua contestan a la búsqueda DHIP: un paquete UDP al puerto 37810 (a 239.255.255.251 y a
//! broadcast) con `{"method": "DHDiscover.search"}`. Cada cámara responde con su número de serie, IP, modelo
//! y MAC, así que con el número de serie de la etiqueta se sabe cuál es cuál aunque cambie de IP.
//!
//! Si no contesta nadie (el firewall de la PC tira las respuestas al grupo y al broadcast, porque no salen
//! de la IP a la que se preguntó; algunos módems también las bloquean), `buscar_todo` escanea el puerto
//! 37777 (el de Dahua) y el 554 (RTSP) en tu red /24 y le hace la misma pregunta a cada IP que contestó,
//! directo: así sí pasa el firewall y se sabe el número de serie.
//!
//! Las cámaras de otras casas (por Tailscale) no salen en ninguna búsqueda: se escribe su IP a mano.

use crate::camaras::limpiar_serie;
use crate::cripto::{base64, md5_hex};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

pub const PUERTO_DHIP: u16 = 37810;
pub const GRUPO_DHIP: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 251);

/// Una cámara que contestó: {serie, ip, modelo, mac, version}.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Hallada {
    pub serie: String,
    pub ip: String,
    pub modelo: String,
    pub mac: String,
    pub version: String,
}

impl Hallada {
    pub fn solo_ip(ip: &str) -> Hallada {
        Hallada {
            ip: ip.into(),
            ..Default::default()
        }
    }

    pub fn a_json(&self) -> serde_json::Map<String, Value> {
        let Value::Object(m) = json!({
            "serie": self.serie, "ip": self.ip, "modelo": self.modelo, "mac": self.mac,
            "version": self.version,
        }) else {
            unreachable!()
        };
        m
    }
}

/// Cabecera DHIP de 32 bytes + la búsqueda en JSON.
pub fn paquete_busqueda() -> Vec<u8> {
    let datos = br#"{"method":"DHDiscover.search","params":{"mac":"","uni":1}}"#;
    let n = datos.len() as u32;
    let mut p = b"\x20\x00\x00\x00DHIP".to_vec();
    p.extend_from_slice(&[0; 8]);
    for x in [n, 0, n, 0] {
        p.extend_from_slice(&x.to_le_bytes());
    }
    p.extend_from_slice(datos);
    p
}

fn texto(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(false)) => String::new(),
        Some(Value::Number(n)) if n.as_f64() == Some(0.0) => String::new(),
        Some(otro) => otro.to_string(),
    }
}

/// La respuesta de una cámara. `None` si no es una respuesta (por ejemplo, nuestra propia búsqueda, que
/// también llega al grupo).
pub fn interpretar(datos: &[u8], origen: &str) -> Option<Hallada> {
    let inicio = datos.iter().position(|&b| b == b'{')?;
    let fin = datos.iter().rposition(|&b| b == b'}')?;
    if fin <= inicio {
        return None;
    }
    let j: Value = serde_json::from_str(&String::from_utf8_lossy(&datos[inicio..=fin])).ok()?;
    let j = j.as_object()?;
    if j.get("method").and_then(Value::as_str) == Some("DHDiscover.search") {
        return None;
    }
    let vacio = serde_json::Map::new();
    let params = j.get("params").and_then(Value::as_object).unwrap_or(&vacio);
    let info = params
        .get("deviceInfo")
        .and_then(Value::as_object)
        .unwrap_or(params);
    let ipv4 = info
        .get("IPv4Address")
        .and_then(Value::as_object)
        .unwrap_or(&vacio);
    let mut ip = texto(ipv4.get("IPAddress"));
    if ip.is_empty() {
        ip = origen.to_string();
    }
    let serie = limpiar_serie(&texto(info.get("SerialNo")));
    if ip.is_empty() && serie.is_empty() {
        return None;
    }
    let mut modelo = texto(info.get("DeviceType"));
    if modelo.is_empty() {
        modelo = texto(info.get("DeviceClass"));
    }
    Some(Hallada {
        serie,
        ip,
        modelo,
        mac: texto(info.get("Mac")).to_lowercase(),
        version: texto(info.get("Version")),
    })
}

fn orden_ip(ip: &str) -> Vec<u32> {
    ip.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// Pregunta en la red quién es cámara y junta las respuestas durante `segundos`. Sin `destinos`, al grupo
/// y a broadcast.
pub fn buscar(segundos: f64, destinos: Option<&[SocketAddr]>) -> Vec<Hallada> {
    let por_defecto = [
        SocketAddr::from((GRUPO_DHIP, PUERTO_DHIP)),
        SocketAddr::from((Ipv4Addr::BROADCAST, PUERTO_DHIP)),
    ];
    let destinos = destinos.unwrap_or(&por_defecto);
    // Algunas cámaras contestan al grupo y no a quien preguntó: se escucha en su puerto si se puede.
    let s = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, PUERTO_DHIP)) {
        Ok(s) => {
            let _ = s.join_multicast_v4(&GRUPO_DHIP, &Ipv4Addr::UNSPECIFIED);
            s
        }
        Err(_) => match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        },
    };
    let _ = s.set_broadcast(true);
    let _ = s.set_multicast_ttl_v4(2);
    let paquete = paquete_busqueda();
    for d in destinos {
        let _ = s.send_to(&paquete, d);
    }
    let mut encontradas: Vec<(String, Hallada)> = Vec::new();
    let limite = Instant::now() + Duration::from_secs_f64(segundos.max(0.0));
    let mut buf = vec![0u8; 65535];
    loop {
        let falta = limite.saturating_duration_since(Instant::now());
        if falta.is_zero() || s.set_read_timeout(Some(falta)).is_err() {
            break;
        }
        let Ok((n, origen)) = s.recv_from(&mut buf) else {
            break;
        };
        if let Some(c) = interpretar(&buf[..n], &origen.ip().to_string()) {
            let clave = if c.serie.is_empty() {
                c.ip.clone()
            } else {
                c.serie.clone()
            };
            match encontradas.iter_mut().find(|(k, _)| *k == clave) {
                Some(e) => e.1 = c,
                None => encontradas.push((clave, c)),
            }
        }
    }
    let mut v: Vec<Hallada> = encontradas.into_iter().map(|(_, c)| c).collect();
    v.sort_by_key(|c| orden_ip(&c.ip));
    v
}

/// La IP de esta PC en tu red (sin mandar nada: connect en UDP solo elige la ruta).
pub fn ip_local() -> String {
    UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))
        .and_then(|s| {
            s.connect(("192.0.2.1", 9))?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_default()
}

fn conectar(ip: &str, puerto: u16, espera: Duration) -> std::io::Result<TcpStream> {
    let dir = (ip, puerto)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "sin dirección"))?;
    TcpStream::connect_timeout(&dir, espera)
}

pub fn abierto(ip: &str, puerto: u16, espera: Duration) -> bool {
    conectar(ip, puerto, espera).is_ok()
}

fn rtsp(
    s: &mut TcpStream,
    metodo: &str,
    url: &str,
    cseq: u32,
    extra: &str,
) -> std::io::Result<(u16, Vec<(String, String)>)> {
    s.write_all(
        format!("{metodo} {url} RTSP/1.0\r\nCSeq: {cseq}\r\nAccept: application/sdp\r\n{extra}\r\n")
            .as_bytes(),
    )?;
    let mut datos = Vec::new();
    let mut buf = [0u8; 4096];
    while !datos.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = s.read(&mut buf)?;
        if n == 0 {
            break;
        }
        datos.extend_from_slice(&buf[..n]);
    }
    let fin = datos
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .unwrap_or(datos.len());
    let cabeza = String::from_utf8_lossy(&datos[..fin]).into_owned();
    let mut lineas = cabeza.split("\r\n");
    let codigo = lineas
        .next()
        .and_then(|l| l.strip_prefix("RTSP/1.0 "))
        .and_then(|r| r.get(..3))
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let mut cabeceras: Vec<(String, String)> = Vec::new();
    for l in lineas {
        let (k, v) = l.split_once(':').unwrap_or((l, ""));
        let k = k.trim().to_lowercase();
        if !cabeceras.iter().any(|(n, _)| *n == k) {
            cabeceras.push((k, v.trim().to_string()));
        }
    }
    Ok((codigo, cabeceras))
}

/// La cabecera Authorization para el desafío de la cámara (Digest MD5 o Basic).
pub fn autorizacion(desafio: &str, metodo: &str, url: &str, usuario: &str, clave: &str) -> String {
    if desafio.to_lowercase().starts_with("digest") {
        let campos = crate::http::campos_digest(desafio);
        let c = |k: &str| {
            campos
                .iter()
                .find(|(n, _)| n == k)
                .map_or("", |(_, v)| v.as_str())
        };
        let ha1 = md5_hex(&format!("{usuario}:{}:{clave}", c("realm")));
        let ha2 = md5_hex(&format!("{metodo}:{url}"));
        let resp = md5_hex(&format!("{ha1}:{}:{ha2}", c("nonce")));
        return format!(
            "Authorization: Digest username=\"{usuario}\", realm=\"{}\", nonce=\"{}\", uri=\"{url}\", response=\"{resp}\"\r\n",
            c("realm"),
            c("nonce")
        );
    }
    format!(
        "Authorization: Basic {}\r\n",
        base64(format!("{usuario}:{clave}").as_bytes())
    )
}

/// Cómo contestó una cámara a la prueba de RTSP.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prueba {
    Ok,
    /// No aceptó el código.
    Codigo,
    SinConexion,
    Error,
}

/// ¿Contesta la cámara y acepta el código? Un DESCRIBE, como el que hace el reproductor, sin traerse el video.
pub fn probar_rtsp(
    ip: &str,
    puerto: u16,
    ruta: &str,
    usuario: &str,
    clave: &str,
    espera: Duration,
) -> Prueba {
    let host = if ip.contains(':') {
        format!("[{ip}]")
    } else {
        ip.to_string()
    };
    let url = format!("rtsp://{host}:{puerto}{ruta}");
    let intento = (|| -> std::io::Result<u16> {
        let mut s = conectar(ip, puerto, espera)?;
        s.set_read_timeout(Some(espera))?;
        s.set_write_timeout(Some(espera))?;
        let (mut codigo, cab) = rtsp(&mut s, "DESCRIBE", &url, 1, "")?;
        if codigo == 401
            && let Some((_, desafio)) = cab.iter().find(|(k, _)| k == "www-authenticate")
        {
            let a = autorizacion(desafio, "DESCRIBE", &url, usuario, clave);
            codigo = rtsp(&mut s, "DESCRIBE", &url, 2, &a)?.0;
        }
        Ok(codigo)
    })();
    match intento {
        Err(_) => Prueba::SinConexion,
        Ok(200) => Prueba::Ok,
        Ok(401 | 403) => Prueba::Codigo,
        Ok(0) => Prueba::SinConexion,
        Ok(_) => Prueba::Error,
    }
}

/// Primero la búsqueda al grupo; si nadie contesta, escanear y preguntar a cada IP directo.
pub fn buscar_todo(segundos: f64) -> Vec<Hallada> {
    buscar_todo_con(segundos, buscar, escanear)
}

/// `buscar_todo` con la búsqueda y el escaneo cambiables (para las pruebas).
pub fn buscar_todo_con(
    segundos: f64,
    buscar: impl Fn(f64, Option<&[SocketAddr]>) -> Vec<Hallada>,
    escanear: impl Fn() -> Vec<Hallada>,
) -> Vec<Hallada> {
    let halladas = buscar(segundos, None);
    if !halladas.is_empty() {
        return halladas;
    }
    let ips = escanear();
    if ips.is_empty() {
        return Vec::new();
    }
    let destinos: Vec<SocketAddr> = ips
        .iter()
        .filter_map(|c| c.ip.parse::<Ipv4Addr>().ok())
        .map(|ip| SocketAddr::from((ip, PUERTO_DHIP)))
        .collect();
    let directas = buscar(segundos, Some(&destinos));
    ips.into_iter()
        .map(|c| directas.iter().find(|d| d.ip == c.ip).cloned().unwrap_or(c))
        .collect()
}

/// Plan B: las IP de tu red /24 que tienen abierto el puerto de Dahua (37777) o el de RTSP (554).
pub fn escanear() -> Vec<Hallada> {
    let propia = ip_local();
    let Some((prefijo, _)) = propia.rsplit_once('.') else {
        return Vec::new();
    };
    let ips: Vec<String> = (1..255)
        .map(|n| format!("{prefijo}.{n}"))
        .filter(|ip| *ip != propia)
        .collect();
    let espera = Duration::from_millis(400);
    // 64 a la vez, como el ThreadPoolExecutor de la de Python: 254 IP en ~2 s.
    let mut abiertas = Vec::new();
    for grupo in ips.chunks(64) {
        let hilos: Vec<_> = grupo
            .iter()
            .cloned()
            .map(|ip| {
                std::thread::spawn(move || {
                    [37777, 554]
                        .iter()
                        .any(|&p| abierto(&ip, p, espera))
                        .then_some(ip)
                })
            })
            .collect();
        abiertas.extend(hilos.into_iter().filter_map(|h| h.join().ok().flatten()));
    }
    abiertas.iter().map(|ip| Hallada::solo_ip(ip)).collect()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn paquete() {
        let p = paquete_busqueda();
        assert_eq!(&p[..8], b"\x20\x00\x00\x00DHIP");
        let n = u32::from_le_bytes(p[16..20].try_into().unwrap()) as usize;
        assert_eq!(p.len(), 32 + n);
        let j: Value = serde_json::from_slice(&p[32..]).unwrap();
        assert_eq!(j["method"], "DHDiscover.search");
    }

    #[test]
    fn interpretar_ignora_lo_propio() {
        assert!(interpretar(&paquete_busqueda(), "").is_none());
        assert!(interpretar(b"DHIP{roto", "").is_none());
    }

    #[test]
    fn busqueda_con_una_camara_que_contesta() {
        let resp = UdpSocket::bind("127.0.0.1:0").unwrap();
        let dir = resp.local_addr().unwrap();
        let hilo = std::thread::spawn(move || {
            let mut b = [0u8; 4096];
            let (n, origen) = resp.recv_from(&mut b).unwrap();
            assert!(String::from_utf8_lossy(&b[..n]).contains("DHDiscover.search"));
            let info = json!({"method": "client.notifyDevInfo", "params": {"deviceInfo": {
                "SerialNo": "9a0b1c2d", "DeviceType": "IPC-K42", "Mac": "AA:BB:CC:00:11:22",
                "IPv4Address": {"IPAddress": "192.168.1.77"}}}});
            let mut p = b"\x20\x00\x00\x00DHIP".to_vec();
            p.extend_from_slice(&[0; 24]);
            p.extend_from_slice(info.to_string().as_bytes());
            resp.send_to(&p, origen).unwrap();
            resp.send_to(b"basura", origen).unwrap();
        });
        let halladas = buscar(1.0, Some(&[dir]));
        hilo.join().unwrap();
        assert_eq!(
            halladas,
            vec![Hallada {
                serie: "9A0B1C2D".into(),
                ip: "192.168.1.77".into(),
                modelo: "IPC-K42".into(),
                mac: "aa:bb:cc:00:11:22".into(),
                version: String::new(),
            }]
        );
    }

    #[test]
    fn si_nadie_contesta_al_grupo_pregunta_directo() {
        let llamadas = std::sync::Mutex::new(Vec::new());
        let halladas = buscar_todo_con(
            2.5,
            |_, destinos| {
                llamadas
                    .lock()
                    .unwrap()
                    .push(destinos.map(<[SocketAddr]>::to_vec));
                match destinos {
                    None => Vec::new(), // el firewall tiró las respuestas al grupo
                    Some(_) => vec![Hallada {
                        serie: "7E04ABC".into(),
                        ip: "192.168.1.7".into(),
                        modelo: "IPC-S22F".into(),
                        ..Default::default()
                    }],
                }
            },
            || vec![Hallada::solo_ip("192.168.1.7"), Hallada::solo_ip("192.168.1.9")],
        );
        assert_eq!(
            *llamadas.lock().unwrap(),
            vec![
                None,
                Some(vec![
                    "192.168.1.7:37810".parse().unwrap(),
                    "192.168.1.9:37810".parse().unwrap()
                ])
            ]
        );
        let pares: Vec<_> = halladas
            .iter()
            .map(|c| (c.serie.as_str(), c.ip.as_str()))
            .collect();
        assert_eq!(pares, [("7E04ABC", "192.168.1.7"), ("", "192.168.1.9")]);
    }

    #[test]
    fn autorizacion_digest() {
        let cab = autorizacion(
            r#"Digest realm="Login to 9A0B", nonce="abc""#,
            "DESCRIBE",
            "rtsp://h/x",
            "admin",
            "L2C",
        );
        let ha1 = md5_hex("admin:Login to 9A0B:L2C");
        let ha2 = md5_hex("DESCRIBE:rtsp://h/x");
        assert!(cab.contains(&format!(r#"response="{}""#, md5_hex(&format!("{ha1}:abc:{ha2}")))));
        assert!(
            autorizacion(r#"Basic realm="x""#, "DESCRIBE", "u", "admin", "a:b")
                .starts_with("Authorization: Basic YWRtaW46YTpi")
        );
    }
}
