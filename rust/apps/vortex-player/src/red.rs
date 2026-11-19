//! Internet para el Player (LRCLIB, YouTube Music, la página de Spotify y Google Drive) con `curl`, como la Shop:
//! sin crates de TLS. Lo delicado (encabezados con el token de Google y el cuerpo) nunca va en la línea de
//! comandos, donde cualquiera lo vería con `ps`: la configuración entra por stdin (`-K -`) y el cuerpo por un
//! archivo temporal privado. `VORTEX_PLAYER_SIN_RED=1` (las pruebas) no sale a internet.

use serde_json::Value;
use std::fmt;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorRed {
    /// Sin red, sin `curl`, el servidor no contestó o se tardó demasiado.
    SinConexion(String),
    /// Contestó con un código de error (y lo que mandó, que a veces explica el porqué).
    Http(u16, Vec<u8>),
    /// Contestó algo que no se entiende (no es JSON…).
    Rara(String),
}

impl fmt::Display for ErrorRed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorRed::SinConexion(m) | ErrorRed::Rara(m) => f.write_str(m),
            ErrorRed::Http(c, _) => write!(f, "el servidor contestó {c}"),
        }
    }
}

impl std::error::Error for ErrorRed {}

/// Una petición. `cabeceras` como «Nombre: valor».
#[derive(Debug, Clone, Default)]
pub struct Peticion<'a> {
    pub metodo: &'a str,
    pub url: &'a str,
    pub cabeceras: Vec<String>,
    pub cuerpo: Option<Vec<u8>>,
    /// Segundos.
    pub espera: u64,
}

impl<'a> Peticion<'a> {
    pub fn get(url: &'a str) -> Self {
        Self {
            metodo: "GET",
            url,
            espera: 15,
            ..Self::default()
        }
    }

    pub fn post(url: &'a str, cuerpo: Vec<u8>) -> Self {
        Self {
            metodo: "POST",
            url,
            cuerpo: Some(cuerpo),
            espera: 15,
            ..Self::default()
        }
    }

    pub fn cabecera(mut self, c: impl Into<String>) -> Self {
        self.cabeceras.push(c.into());
        self
    }

    pub fn espera(mut self, s: u64) -> Self {
        self.espera = s;
        self
    }
}

/// Un valor para el archivo de configuración de curl, entre comillas.
fn citar(s: &str) -> String {
    let mut r = String::with_capacity(s.len() + 2);
    r.push('"');
    for c in s.chars() {
        match c {
            '\\' => r.push_str("\\\\"),
            '"' => r.push_str("\\\""),
            '\n' => r.push_str("\\n"),
            '\r' => r.push_str("\\r"),
            '\t' => r.push_str("\\t"),
            _ => r.push(c),
        }
    }
    r.push('"');
    r
}

pub fn sin_red() -> bool {
    std::env::var_os("VORTEX_PLAYER_SIN_RED").is_some_and(|v| !v.is_empty())
}

/// Hace la petición y devuelve el cuerpo si el código es 2xx.
pub fn pedir(p: &Peticion<'_>) -> Result<Vec<u8>, ErrorRed> {
    if sin_red() {
        return Err(ErrorRed::SinConexion("sin red (pruebas)".into()));
    }
    let mut conf = format!(
        "url = {}\nrequest = {}\nmax-time = {}\nconnect-timeout = 10\nlocation\nsilent\nshow-error\ncompressed\n",
        citar(p.url),
        citar(p.metodo),
        p.espera.max(1)
    );
    conf.push_str("write-out = \"\\n%{http_code}\"\n");
    for c in &p.cabeceras {
        conf.push_str(&format!("header = {}\n", citar(c)));
    }
    // El cuerpo, en un archivo que solo tú puedes leer y que se borra solo.
    let mut archivo = None;
    if let Some(cuerpo) = &p.cuerpo {
        let mut f = tempfile::Builder::new()
            .prefix("vortex-player-")
            .tempfile_in(vortex_core::rutas::runtime().unwrap_or_else(std::env::temp_dir))
            .or_else(|_| tempfile::NamedTempFile::new())
            .map_err(|e| ErrorRed::SinConexion(format!("no pude preparar la petición: {e}")))?;
        f.write_all(cuerpo)
            .map_err(|e| ErrorRed::SinConexion(format!("no pude preparar la petición: {e}")))?;
        conf.push_str(&format!(
            "data-binary = {}\n",
            citar(&format!("@{}", f.path().to_string_lossy()))
        ));
        archivo = Some(f);
    }
    let mut hijo = Command::new(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-K", "-"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| ErrorRed::SinConexion("Falta curl para conectarse a internet".into()))?;
    if let Some(mut e) = hijo.stdin.take() {
        let _ = e.write_all(conf.as_bytes());
    }
    let salida = hijo
        .wait_with_output()
        .map_err(|e| ErrorRed::SinConexion(e.to_string()))?;
    drop(archivo);
    let mut datos = salida.stdout;
    let corte = datos.iter().rposition(|b| *b == b'\n');
    let codigo: u16 = corte
        .and_then(|i| std::str::from_utf8(&datos[i + 1..]).ok())
        .and_then(|c| c.trim().parse().ok())
        .unwrap_or(0);
    if let Some(i) = corte {
        datos.truncate(i);
    }
    if codigo == 0 || !salida.status.success() {
        let err = String::from_utf8_lossy(&salida.stderr).trim().to_string();
        return Err(ErrorRed::SinConexion(if err.is_empty() {
            "Sin conexión".into()
        } else {
            err
        }));
    }
    if !(200..300).contains(&codigo) {
        return Err(ErrorRed::Http(codigo, datos));
    }
    Ok(datos)
}

/// Como `pedir`, leyendo la respuesta como JSON.
pub fn json(p: &Peticion<'_>) -> Result<Value, ErrorRed> {
    let datos = pedir(p)?;
    serde_json::from_slice(&datos).map_err(|_| ErrorRed::Rara("la respuesta no es JSON".into()))
}

/// `urllib.parse.quote(texto, safe='')`: todo menos letras, números y `_.-~` va como `%XX`.
pub fn escapar(texto: &str) -> String {
    let mut r = String::with_capacity(texto.len());
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) {
            r.push(b as char);
        } else {
            r.push_str(&format!("%{b:02X}"));
        }
    }
    r
}

/// `urllib.parse.quote(texto)` (deja `/`), para pedazos de ruta.
pub fn escapar_ruta(texto: &str) -> String {
    escapar(texto).replace("%2F", "/")
}

/// `urllib.parse.urlencode(datos)`: `a=1&b=dos+palabras`.
pub fn formulario(datos: &[(&str, &str)]) -> String {
    datos
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                escapar(k).replace("%20", "+"),
                escapar(v).replace("%20", "+")
            )
        })
        .collect::<Vec<_>>()
        .join("&")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn escapa_como_python() {
        assert_eq!(
            escapar("YOASOBI アイドル/1?&"),
            "YOASOBI%20%E3%82%A2%E3%82%A4%E3%83%89%E3%83%AB%2F1%3F%26"
        );
        assert_eq!(escapar_ruta("a b/c"), "a%20b/c");
        assert_eq!(formulario(&[("q", "a b"), ("x", "1&2")]), "q=a+b&x=1%262");
        assert_eq!(citar("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }

    #[test]
    fn contra_un_servidor_de_verdad() {
        use std::io::{Read, Write as _};
        let srv = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/eco", srv.local_addr().unwrap());
        std::thread::spawn(move || {
            for (i, c) in srv.incoming().take(3).enumerate() {
                let mut c = c.unwrap();
                let mut b = vec![0u8; 4096];
                let n = c.read(&mut b).unwrap();
                let pedido = String::from_utf8_lossy(&b[..n]).to_string();
                let r = match i {
                    0 => {
                        let cuerpo = format!(
                            "{{\"auth\": {}, \"post\": {}}}",
                            pedido.contains("Authorization: Bearer secreto"),
                            pedido.starts_with("POST")
                        );
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{cuerpo}",
                            cuerpo.len()
                        )
                    }
                    1 => "HTTP/1.1 404 Not Found\r\nContent-Length: 4\r\n\r\nnada".into(),
                    _ => "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nno js".into(),
                };
                c.write_all(r.as_bytes()).unwrap();
            }
        });
        let v =
            json(&Peticion::post(&url, b"x=1".to_vec()).cabecera("Authorization: Bearer secreto")).unwrap();
        assert_eq!(v, serde_json::json!({"auth": true, "post": true}));
        assert_eq!(
            pedir(&Peticion::get(&url)),
            Err(ErrorRed::Http(404, b"nada".to_vec()))
        );
        assert!(matches!(json(&Peticion::get(&url)), Err(ErrorRed::Rara(_))));
        // Un puerto donde no hay nadie: sin conexión
        let cerrado = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let u = format!("http://{}/", cerrado.local_addr().unwrap());
        drop(cerrado);
        assert!(matches!(
            pedir(&Peticion::get(&u).espera(3)),
            Err(ErrorRed::SinConexion(_))
        ));
    }
}
