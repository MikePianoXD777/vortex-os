//! Un cliente HTTP/1.1 mínimo para hablar con las cámaras de tu red (ONVIF y el CGI de Dahua): sin TLS ni
//! redirecciones, que las cámaras no usan. Entiende `Content-Length`, `chunked` y conexiones que se cierran,
//! y la autenticación Digest (con y sin `qop`), como el `urllib` de la de Python.

use crate::cripto::{azar, hex, md5_hex};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Debug)]
pub struct Respuesta {
    pub estado: u16,
    /// Cabeceras con el nombre en minúsculas.
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: Vec<u8>,
}

impl Respuesta {
    pub fn cabecera(&self, nombre: &str) -> Option<&str> {
        self.cabeceras
            .iter()
            .find(|(k, _)| k == nombre)
            .map(|(_, v)| v.as_str())
    }

    pub fn texto(&self) -> String {
        String::from_utf8_lossy(&self.cuerpo).into_owned()
    }
}

/// «192.168.1.20» → puerto 80; «127.0.0.1:8080» (como lo escriben las pruebas) → ese puerto; «fd7a::1» → [fd7a::1]:80.
pub fn direccion(host: &str) -> std::io::Result<SocketAddr> {
    if let Ok(a) = host.parse::<SocketAddr>() {
        return Ok(a);
    }
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return Ok(SocketAddr::new(ip, 80));
    }
    let con_puerto = if host.contains(':') {
        host.to_string()
    } else {
        format!("{host}:80")
    };
    con_puerto
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no se encontró la cámara"))
}

fn leer_linea(datos: &[u8], desde: usize) -> Option<(usize, &[u8])> {
    let fin = datos[desde..].windows(2).position(|w| w == b"\r\n")? + desde;
    Some((fin + 2, &datos[desde..fin]))
}

fn quitar_chunked(datos: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    let mut i = 0;
    while let Some((sig, linea)) = leer_linea(datos, i) {
        let tam = std::str::from_utf8(linea)
            .ok()
            .and_then(|t| usize::from_str_radix(t.split(';').next().unwrap_or("").trim(), 16).ok());
        let Some(tam) = tam else { break };
        if tam == 0 || sig + tam > datos.len() {
            break;
        }
        v.extend_from_slice(&datos[sig..sig + tam]);
        i = sig + tam + 2;
    }
    v
}

/// Una petición. `ruta` con su `?consulta`; `cabeceras` como «Nombre: valor».
pub fn pedir(
    metodo: &str,
    host: &str,
    ruta: &str,
    cabeceras: &[String],
    cuerpo: &[u8],
    espera: Duration,
) -> std::io::Result<Respuesta> {
    let mut s = TcpStream::connect_timeout(&direccion(host)?, espera)?;
    s.set_read_timeout(Some(espera))?;
    s.set_write_timeout(Some(espera))?;
    let mut p = format!(
        "{metodo} {ruta} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nUser-Agent: Vortexine-Camera\r\n"
    );
    for c in cabeceras {
        p.push_str(c);
        p.push_str("\r\n");
    }
    if !cuerpo.is_empty() || metodo == "POST" {
        p.push_str(&format!("Content-Length: {}\r\n", cuerpo.len()));
    }
    p.push_str("\r\n");
    s.write_all(p.as_bytes())?;
    s.write_all(cuerpo)?;
    let mut datos = Vec::new();
    let mut buf = [0u8; 8192];
    // Se lee hasta tener la cabecera y el cuerpo completo (o hasta que la cámara cierre).
    loop {
        let n = match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if datos.is_empty() => return Err(e),
            Err(_) => break,
        };
        datos.extend_from_slice(&buf[..n]);
        if let Some(fin) = datos.windows(4).position(|w| w == b"\r\n\r\n") {
            let cab = String::from_utf8_lossy(&datos[..fin]).to_ascii_lowercase();
            let largo = cab.lines().find_map(|l| {
                l.strip_prefix("content-length:")
                    .and_then(|v| v.trim().parse::<usize>().ok())
            });
            if largo.is_some_and(|l| datos.len() >= fin + 4 + l) {
                break;
            }
            if cab.contains("transfer-encoding: chunked") && datos.ends_with(b"0\r\n\r\n") {
                break;
            }
        }
        if datos.len() > 4 * 1024 * 1024 {
            break; // una cámara no manda tanto: algo raro
        }
    }
    let Some(fin) = datos.windows(4).position(|w| w == b"\r\n\r\n") else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "respuesta incompleta",
        ));
    };
    let cabeza = String::from_utf8_lossy(&datos[..fin]).into_owned();
    let mut lineas = cabeza.split("\r\n");
    let estado = lineas
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "no es HTTP"))?;
    let cabeceras: Vec<(String, String)> = lineas
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    let mut cuerpo = datos[fin + 4..].to_vec();
    let r = Respuesta {
        estado,
        cabeceras,
        cuerpo: Vec::new(),
    };
    if r.cabecera("transfer-encoding")
        .is_some_and(|t| t.eq_ignore_ascii_case("chunked"))
    {
        cuerpo = quitar_chunked(&cuerpo);
    } else if let Some(l) = r.cabecera("content-length").and_then(|v| v.parse::<usize>().ok()) {
        cuerpo.truncate(l);
    }
    Ok(Respuesta { cuerpo, ..r })
}

/// Los campos de un desafío `Digest realm="…", nonce="…", qop="auth"` (con o sin comillas).
pub fn campos_digest(desafio: &str) -> Vec<(String, String)> {
    let re = regex::Regex::new(r#"(\w+)="?([^",]*)"?"#).expect("regex fija");
    re.captures_iter(desafio)
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

fn campo<'a>(campos: &'a [(String, String)], k: &str) -> &'a str {
    campos
        .iter()
        .find(|(n, _)| n == k)
        .map_or("", |(_, v)| v.as_str())
}

/// La cabecera `Authorization` de Digest MD5 para HTTP (con `qop=auth` si la cámara lo pide).
pub fn digest_http(desafio: &str, metodo: &str, uri: &str, usuario: &str, clave: &str) -> String {
    let c = campos_digest(desafio);
    let (realm, nonce) = (campo(&c, "realm"), campo(&c, "nonce"));
    let ha1 = md5_hex(&format!("{usuario}:{realm}:{clave}"));
    let ha2 = md5_hex(&format!("{metodo}:{uri}"));
    let mut cab = format!(
        r#"Authorization: Digest username="{usuario}", realm="{realm}", nonce="{nonce}", uri="{uri}""#
    );
    let qop = campo(&c, "qop");
    if qop.split(',').any(|q| q.trim() == "auth") {
        let cnonce = hex(&azar(8));
        let nc = "00000001";
        let resp = md5_hex(&format!("{ha1}:{nonce}:{nc}:{cnonce}:auth:{ha2}"));
        cab.push_str(&format!(
            r#", response="{resp}", qop=auth, nc={nc}, cnonce="{cnonce}""#
        ));
    } else {
        let resp = md5_hex(&format!("{ha1}:{nonce}:{ha2}"));
        cab.push_str(&format!(r#", response="{resp}""#));
    }
    let opaque = campo(&c, "opaque");
    if !opaque.is_empty() {
        cab.push_str(&format!(r#", opaque="{opaque}""#));
    }
    cab.push_str(", algorithm=MD5");
    cab
}

/// GET con Digest: si la cámara contesta 401 con un desafío, se repite una vez con la contraseña.
pub fn get_digest(
    host: &str,
    ruta: &str,
    usuario: &str,
    clave: &str,
    espera: Duration,
) -> std::io::Result<Respuesta> {
    let r = pedir("GET", host, ruta, &[], &[], espera)?;
    if r.estado != 401 {
        return Ok(r);
    }
    let Some(desafio) = r.cabecera("www-authenticate").map(str::to_string) else {
        return Ok(r);
    };
    if !desafio.to_ascii_lowercase().starts_with("digest") {
        return Ok(r);
    }
    let cab = digest_http(&desafio, "GET", ruta, usuario, clave);
    pedir("GET", host, ruta, &[cab], &[], espera)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn direcciones() {
        assert_eq!(direccion("10.0.0.2").unwrap().to_string(), "10.0.0.2:80");
        assert_eq!(direccion("127.0.0.1:8080").unwrap().port(), 8080);
        assert_eq!(direccion("fd7a::1").unwrap().to_string(), "[fd7a::1]:80");
    }

    #[test]
    fn chunked_y_content_length() {
        let srv = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = srv.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            for (i, c) in srv.incoming().take(2).enumerate() {
                let mut c = c.unwrap();
                let mut b = [0u8; 1024];
                let _ = c.read(&mut b);
                let r: &[u8] = if i == 0 {
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nOK\r\n3\r\n!!!\r\n0\r\n\r\n"
                } else {
                    b"HTTP/1.0 404 Not Found\r\nContent-Length: 4\r\n\r\nnadaEXTRA"
                };
                c.write_all(r).unwrap();
            }
        });
        let t = Duration::from_secs(3);
        let r = pedir("GET", &host, "/a", &[], &[], t).unwrap();
        assert_eq!((r.estado, r.texto().as_str()), (200, "OK!!!"));
        let r = pedir("POST", &host, "/b", &[], b"x", t).unwrap();
        assert_eq!((r.estado, r.texto().as_str()), (404, "nada"));
    }

    #[test]
    fn digest_con_qop() {
        let cab = digest_http(
            r#"Digest realm="Login to falsa", nonce="n1", qop="auth""#,
            "GET",
            "/cgi-bin/ptz.cgi?x=1",
            "admin",
            "L2C",
        );
        let c = campos_digest(&cab);
        let ha1 = md5_hex("admin:Login to falsa:L2C");
        let ha2 = md5_hex("GET:/cgi-bin/ptz.cgi?x=1");
        let bien = md5_hex(&format!(
            "{ha1}:n1:{}:{}:auth:{ha2}",
            campo(&c, "nc"),
            campo(&c, "cnonce")
        ));
        assert_eq!(campo(&c, "response"), bien);
    }
}
