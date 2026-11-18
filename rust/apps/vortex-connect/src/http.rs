//! Un GET mínimo, sin crates: lo justo para las dos preguntas de Connect.
//!
//! - ¿hay internet o un portal? Va por `http://` a propósito (un portal solo puede interceptar lo que no va
//!   cifrado), sin seguir redirecciones y sin proxy: se habla directo por un `TcpStream`.
//! - ¿Cloudflare ve «warp=on»? Eso es `https://`: no hay TLS en la biblioteca estándar, así que esa sola
//!   petición la hace `curl` (viene con Arch y con Windows 10 en adelante).

use crate::sistema::Sistema;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

/// La respuesta, sin seguir ninguna redirección.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Respuesta {
    pub estado: u16,
    pub ubicacion: Option<String>,
    pub cuerpo: String,
}

/// Parte `http://host[:puerto]/ruta` (solo http).
fn partir_url(url: &str) -> Option<(String, u16, String)> {
    let resto = url.strip_prefix("http://")?;
    let (anfitrion, ruta) = match resto.find(['/', '?']) {
        Some(i) if resto.as_bytes()[i] == b'/' => (&resto[..i], resto[i..].to_string()),
        Some(i) => (&resto[..i], format!("/{}", &resto[i..])),
        None => (resto, "/".to_string()),
    };
    let anfitrion = anfitrion.rsplit('@').next()?; // sin «usuario@»
    let (nombre, puerto) = match anfitrion.rsplit_once(':') {
        Some((n, p)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) => {
            (n.to_string(), p.parse().ok()?)
        }
        _ => (anfitrion.to_string(), 80),
    };
    (!nombre.is_empty()).then_some((nombre.trim_matches(['[', ']']).to_string(), puerto, ruta))
}

/// La cabecera y el cuerpo de una respuesta cruda («HTTP/1.x 204 …\r\nCabecera: …\r\n\r\ncuerpo»).
pub fn leer_respuesta(crudo: &[u8], tope_cuerpo: usize) -> Option<Respuesta> {
    let texto = String::from_utf8_lossy(crudo);
    let (cabecera, cuerpo) = texto
        .split_once("\r\n\r\n")
        .or_else(|| texto.split_once("\n\n"))
        .unwrap_or((&texto, ""));
    let mut lineas = cabecera.lines();
    let estado = lineas.next()?.split_whitespace().nth(1)?.parse().ok()?;
    let ubicacion = lineas.find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim()
            .eq_ignore_ascii_case("location")
            .then(|| v.trim().to_string())
    });
    Some(Respuesta {
        estado,
        ubicacion,
        cuerpo: cuerpo.chars().take(tope_cuerpo).collect(),
    })
}

fn get_http(url: &str, limite: Duration, tope: usize) -> Option<Respuesta> {
    let (anfitrion, puerto, ruta) = partir_url(url)?;
    let fin = Instant::now() + limite;
    let dir = (anfitrion.as_str(), puerto).to_socket_addrs().ok()?.next()?;
    let mut s = TcpStream::connect_timeout(&dir, limite).ok()?;
    let queda = || {
        fin.checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    };
    s.set_write_timeout(Some(queda()?)).ok()?;
    let pedido = format!(
        "GET {ruta} HTTP/1.0\r\nHost: {anfitrion}\r\nUser-Agent: vortex-connect\r\nConnection: close\r\n\r\n"
    );
    s.write_all(pedido.as_bytes()).ok()?;
    let mut crudo = Vec::new();
    let mut trozo = [0u8; 2048];
    while crudo.len() < tope + 8192 {
        s.set_read_timeout(Some(queda()?)).ok()?;
        match s.read(&mut trozo) {
            Ok(0) => break,
            Ok(n) => crudo.extend_from_slice(&trozo[..n]),
            Err(_) if !crudo.is_empty() => break,
            Err(_) => return None,
        }
    }
    leer_respuesta(&crudo, tope)
}

/// Un GET sin proxy ni redirecciones. `None` si no se pudo hablar con el servidor.
pub fn obtener(sistema: &Sistema, url: &str, limite: Duration, tope: usize) -> Option<Respuesta> {
    if url.starts_with("https://") {
        let seg = limite.as_secs().max(1).to_string();
        let s = sistema.correr(
            &[
                "curl",
                "-sS",
                "-i",
                "--noproxy",
                "*",
                "--max-time",
                &seg,
                "--",
                url,
            ],
            limite + Duration::from_secs(3),
        );
        return if s.bien() {
            leer_respuesta(s.salida.as_bytes(), tope)
        } else {
            None
        };
    }
    get_http(url, limite, tope)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn partir() {
        assert_eq!(
            partir_url("http://cp.cloudflare.com/generate_204"),
            Some(("cp.cloudflare.com".into(), 80, "/generate_204".into()))
        );
        assert_eq!(
            partir_url("http://127.0.0.1:8080"),
            Some(("127.0.0.1".into(), 8080, "/".into()))
        );
        assert_eq!(
            partir_url("http://h:9/a?b=1"),
            Some(("h".into(), 9, "/a?b=1".into()))
        );
        assert_eq!(partir_url("ftp://x"), None);
        assert_eq!(partir_url("http://"), None);
    }

    #[test]
    fn respuesta() {
        let r = leer_respuesta(
            b"HTTP/1.1 302 Found\r\nLocation: http://portal.test/entra\r\n\r\ncuerpo",
            100,
        )
        .unwrap();
        assert_eq!(r.estado, 302);
        assert_eq!(r.ubicacion.as_deref(), Some("http://portal.test/entra"));
        assert_eq!(r.cuerpo, "cuerpo");
        assert!(leer_respuesta(b"basura", 10).is_none());
    }
}
