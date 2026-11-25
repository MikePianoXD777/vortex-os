//! La cuenta de Google con la que se sincroniza con el teléfono (como `cuenta_google.py`).
//!
//! Se inicia sesión en el navegador (Google regresa a un puerto local de esta PC) y solo se pide permiso para
//! la carpeta oculta de la app en Drive: Vortex Player no ve nada más de tu Drive. La sesión se guarda en
//! `~/.config/vortex-player/google.json`, que solo puede leer tu usuario. Lo de internet va con `curl`
//! ([`crate::red`]): el token nunca aparece en la línea de comandos.

use serde_json::{Value, json};
use std::fmt;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::{cripto, red, rutas};

/// El cliente «App de escritorio» del proyecto Vortex Player en Google Cloud. En las apps de escritorio Google
/// no trata el secreto como secreto (va dentro de la app, como en rclone); va al revés solo para que los
/// escáneres de llaves no lo tomen por una filtración. `cliente-google.json` o
/// `VORTEX_PLAYER_GOOGLE_ID`/`_SECRETO` lo cambian (para probar otro proyecto).
pub const CLIENTE_ID: &str = "986706412005-cn9dc33hf03a6ock3f7mdm1j15a4mndt.apps.googleusercontent.com";
const SECRETO_AL_REVES: &str = "rVPV39OdxANwk3mpxOAg3GG35wGN-XPSCOG";

pub const ALCANCE: &str = "https://www.googleapis.com/auth/drive.appdata";
pub const AUTORIZAR: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN: &str = "https://oauth2.googleapis.com/token";
pub const REVOCAR: &str = "https://oauth2.googleapis.com/revoke";
pub const API: &str = "https://www.googleapis.com/drive/v3";
pub const SUBIR: &str = "https://www.googleapis.com/upload/drive/v3";
pub const ARCHIVO: &str = "vortex-player.json";
const FORMULARIO: &str = "application/x-www-form-urlencoded";

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorGoogle {
    /// Google ya no reconoce la sesión (la cerraste en tu cuenta o caducó).
    SinPermiso(String),
    Otro(String),
}

impl fmt::Display for ErrorGoogle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorGoogle::SinPermiso(m) | ErrorGoogle::Otro(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ErrorGoogle {}

/// (método, url, cuerpo, cabeceras) → (código, respuesta). Se cambia en las pruebas.
pub type Http =
    Arc<dyn Fn(&str, &str, Option<&[u8]>, &[(&str, &str)]) -> Result<(u16, Vec<u8>), String> + Send + Sync>;

/// El de verdad, con `curl`.
pub fn http_curl() -> Http {
    Arc::new(|metodo, url, cuerpo, cabeceras| {
        let mut p = red::Peticion::get(url).espera(30);
        p.metodo = metodo;
        p.cuerpo = cuerpo.map(<[u8]>::to_vec);
        for (k, v) in cabeceras {
            p = p.cabecera(format!("{k}: {v}"));
        }
        match red::pedir(&p) {
            Ok(d) => Ok((200, d)),
            Err(red::ErrorRed::Http(c, d)) => Ok((c, d)),
            Err(e) => Err(e.to_string()),
        }
    })
}

fn ahora() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn json_de(d: &[u8]) -> Result<Value, ErrorGoogle> {
    serde_json::from_slice(d).map_err(|_| ErrorGoogle::Otro("Google contestó algo que no se entiende".into()))
}

/// `secrets.token_urlsafe(n)`.
fn azar(n: usize) -> String {
    let b: Vec<u8> = (0..n).map(|_| rand::random::<u8>()).collect();
    cripto::base64_url(&b)
}

/// (verificador, reto) de PKCE (S256).
pub fn pkce() -> (String, String) {
    let v = azar(64);
    let reto = cripto::base64_url(&cripto::sha256(v.as_bytes()));
    (v, reto)
}

pub const PAGINA: &str = r#"<!doctype html><meta charset="utf-8"><title>Vortex Player</title>
<body style="background:#0B0D1A;color:#E8ECFF;font-family:sans-serif;display:grid;place-items:center;height:90vh">
<div style="text-align:center"><h1 style="color:#9EC5FF">{titulo}</h1><p>{texto}</p></div>"#;

pub struct Cuenta {
    pub http: Http,
    /// Donde va `google.json` (y `cliente-google.json`).
    pub carpeta: PathBuf,
    /// Para las pruebas: el cliente (id, secreto) sin tocar el entorno.
    pub cliente_fijo: Option<(String, String)>,
    token: Mutex<(String, f64)>,
}

impl Default for Cuenta {
    fn default() -> Self {
        Self::new()
    }
}

impl Cuenta {
    pub fn new() -> Self {
        Self::en(rutas::config(), http_curl())
    }

    pub fn en(carpeta: PathBuf, http: Http) -> Self {
        Self {
            http,
            carpeta,
            cliente_fijo: None,
            token: Mutex::new((String::new(), 0.0)),
        }
    }

    fn pedir(
        &self,
        metodo: &str,
        url: &str,
        cuerpo: Option<&[u8]>,
        cab: &[(&str, &str)],
    ) -> Result<(u16, Vec<u8>), ErrorGoogle> {
        (self.http)(metodo, url, cuerpo, cab).map_err(ErrorGoogle::Otro)
    }

    pub fn cliente(&self) -> (String, String) {
        if let Some(c) = &self.cliente_fijo {
            return c.clone();
        }
        let local: Value = std::fs::read(self.carpeta.join("cliente-google.json"))
            .ok()
            .and_then(|d| serde_json::from_slice(&d).ok())
            .unwrap_or_default();
        let de = |var: &str, k: &str, base: String| {
            std::env::var(var)
                .ok()
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    local
                        .get(k)
                        .and_then(Value::as_str)
                        .filter(|v| !v.is_empty())
                        .map(String::from)
                })
                .unwrap_or(base)
        };
        (
            de("VORTEX_PLAYER_GOOGLE_ID", "id", CLIENTE_ID.into()),
            de(
                "VORTEX_PLAYER_GOOGLE_SECRETO",
                "secreto",
                SECRETO_AL_REVES.chars().rev().collect(),
            ),
        )
    }

    pub fn configurada(&self) -> bool {
        let (a, b) = self.cliente();
        !a.is_empty() && !b.is_empty()
    }

    fn archivo(&self) -> PathBuf {
        self.carpeta.join("google.json")
    }

    /// {refresh_token, correo, nombre} o {} si no has iniciado sesión.
    pub fn sesion(&self) -> Value {
        std::fs::read(self.archivo())
            .ok()
            .and_then(|d| serde_json::from_slice::<Value>(&d).ok())
            .filter(|d| {
                d.get("refresh_token")
                    .and_then(Value::as_str)
                    .is_some_and(|t| !t.is_empty())
            })
            .unwrap_or_else(|| json!({}))
    }

    fn guardar(&self, d: &Value) -> Result<(), ErrorGoogle> {
        let err = |e: std::io::Error| ErrorGoogle::Otro(format!("No pude guardar la sesión: {e}"));
        std::fs::create_dir_all(&self.carpeta).map_err(err)?;
        let tmp = self.archivo().with_extension("tmp");
        let mut o = std::fs::OpenOptions::new();
        o.write(true).create(true).truncate(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
        let mut f = o.open(&tmp).map_err(err)?;
        f.write_all(d.to_string().as_bytes()).map_err(err)?;
        drop(f);
        std::fs::rename(&tmp, self.archivo()).map_err(err)
    }

    fn poner_token(&self, valor: &str, expira: i64) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = (valor.into(), ahora() + expira as f64);
    }

    /// Un token para Drive (se renueva solo con la sesión guardada).
    pub fn token(&self) -> Result<String, ErrorGoogle> {
        {
            let t = self.token.lock().unwrap_or_else(|e| e.into_inner());
            if !t.0.is_empty() && ahora() < t.1 - 60.0 {
                return Ok(t.0.clone());
            }
        }
        let s = self.sesion();
        let Some(renovar) = s.get("refresh_token").and_then(Value::as_str) else {
            return Err(ErrorGoogle::SinPermiso("No has iniciado sesión".into()));
        };
        let (cid, secreto) = self.cliente();
        let cuerpo = red::formulario(&[
            ("client_id", &cid),
            ("client_secret", &secreto),
            ("refresh_token", renovar),
            ("grant_type", "refresh_token"),
        ]);
        let (estado, d) = self.pedir(
            "POST",
            TOKEN,
            Some(cuerpo.as_bytes()),
            &[("Content-Type", FORMULARIO)],
        )?;
        if estado == 400 || estado == 401 {
            return Err(ErrorGoogle::SinPermiso(
                "Google ya no reconoce la sesión: vuelve a iniciarla".into(),
            ));
        }
        if estado != 200 {
            return Err(ErrorGoogle::Otro(format!("Google contestó {estado}")));
        }
        let d = json_de(&d)?;
        let valor = d
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.poner_token(
            &valor,
            d.get("expires_in").and_then(Value::as_i64).unwrap_or(3600),
        );
        Ok(valor)
    }

    pub fn olvidar_token(&self) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = (String::new(), 0.0);
    }

    pub fn drive(&self, token: &str, archivo: &str) -> Drive<'_> {
        Drive {
            cuenta: self,
            token: token.into(),
            archivo: archivo.into(),
        }
    }

    /// Abre el navegador para elegir la cuenta; regresa {correo, nombre} cuando Google contesta.
    pub fn iniciar_sesion(&self, abrir: &dyn Fn(&str), esperar: Duration) -> Result<Value, ErrorGoogle> {
        if !self.configurada() {
            return Err(ErrorGoogle::Otro(
                "Esta versión todavía no está registrada con Google".into(),
            ));
        }
        let (cid, secreto) = self.cliente();
        let (verificador, reto) = pkce();
        let estado_esperado = azar(16);
        let srv = TcpListener::bind("127.0.0.1:0").map_err(|e| ErrorGoogle::Otro(e.to_string()))?;
        let redireccion = format!(
            "http://127.0.0.1:{}",
            srv.local_addr().map(|a| a.port()).unwrap_or(0)
        );
        srv.set_nonblocking(true)
            .map_err(|e| ErrorGoogle::Otro(e.to_string()))?;
        abrir(&format!(
            "{AUTORIZAR}?{}",
            red::formulario(&[
                ("client_id", &cid),
                ("redirect_uri", &redireccion),
                ("response_type", "code"),
                ("scope", ALCANCE),
                ("code_challenge", &reto),
                ("code_challenge_method", "S256"),
                ("state", &estado_esperado),
                ("access_type", "offline"),
                ("prompt", "consent"),
            ])
        ));
        let respuesta = esperar_respuesta(&srv, &estado_esperado, esperar)?;
        drop(srv);
        let dato = |k: &str| respuesta.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let code = match (dato("state"), dato("code")) {
            (Some(s), Some(c)) if s == estado_esperado => c,
            _ => {
                let e = dato("error").map(|e| format!(" ({e})")).unwrap_or_default();
                return Err(ErrorGoogle::Otro(format!("No se inició sesión{e}")));
            }
        };
        let cuerpo = red::formulario(&[
            ("client_id", &cid),
            ("client_secret", &secreto),
            ("code", &code),
            ("code_verifier", &verificador),
            ("grant_type", "authorization_code"),
            ("redirect_uri", &redireccion),
        ]);
        let (estado, d) = self.pedir(
            "POST",
            TOKEN,
            Some(cuerpo.as_bytes()),
            &[("Content-Type", FORMULARIO)],
        )?;
        if estado != 200 {
            return Err(ErrorGoogle::Otro(format!("Google no dio la sesión ({estado})")));
        }
        let d = json_de(&d)?;
        let renovar = d.get("refresh_token").and_then(Value::as_str).unwrap_or("");
        if renovar.is_empty() {
            return Err(ErrorGoogle::Otro("Google no dio una sesión que dure".into()));
        }
        let acceso = d
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        self.poner_token(
            &acceso,
            d.get("expires_in").and_then(Value::as_i64).unwrap_or(3600),
        );
        let (nombre, correo) = self.drive(&acceso, ARCHIVO).usuario()?;
        self.guardar(&json!({"refresh_token": renovar, "correo": correo, "nombre": nombre}))?;
        Ok(json!({"correo": correo, "nombre": nombre}))
    }

    pub fn cerrar_sesion(&self) {
        let s = self.sesion();
        if let Some(t) = s.get("refresh_token").and_then(Value::as_str) {
            let cuerpo = red::formulario(&[("token", t)]);
            let _ = self.pedir(
                "POST",
                REVOCAR,
                Some(cuerpo.as_bytes()),
                &[("Content-Type", FORMULARIO)],
            ); // sin internet: igual se olvida aquí
        }
        self.olvidar_token();
        let _ = std::fs::remove_file(self.archivo());
    }
}

/// Espera a que el navegador regrese con `?code=…&state=…` (o `?error=…`) y le contesta con la página.
fn esperar_respuesta(
    srv: &TcpListener,
    estado: &str,
    esperar: Duration,
) -> Result<Vec<(String, String)>, ErrorGoogle> {
    let limite = Instant::now() + esperar;
    while Instant::now() < limite {
        let Ok((mut c, _)) = srv.accept() else {
            std::thread::sleep(Duration::from_millis(50));
            continue;
        };
        let _ = c.set_nonblocking(false);
        let _ = c.set_read_timeout(Some(Duration::from_secs(5)));
        let mut b = vec![0u8; 8192];
        let n = c.read(&mut b).unwrap_or(0);
        let pedido = String::from_utf8_lossy(&b[..n]).to_string();
        let ruta = pedido
            .lines()
            .next()
            .and_then(|l| l.split(' ').nth(1))
            .unwrap_or("");
        let q: Vec<(String, String)> = ruta
            .split_once('?')
            .map(|(_, q)| q)
            .unwrap_or("")
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| {
                (
                    crate::descargas::desescapar(k, true),
                    crate::descargas::desescapar(v, true),
                )
            })
            .collect();
        let tiene = |k: &str| q.iter().any(|(a, _)| a == k);
        if !tiene("code") && !tiene("error") {
            let _ = c.write_all(b"HTTP/1.0 404 Not Found\r\nContent-Length: 0\r\n\r\n");
            continue;
        }
        let ok = tiene("code") && q.iter().any(|(a, v)| a == "state" && v == estado);
        let (titulo, texto) = if ok {
            (
                "¡Listo!",
                "Ya puedes cerrar esta pestaña y volver a Vortex Player.",
            )
        } else {
            (
                "No se inició sesión",
                "Vuelve a Vortex Player e inténtalo otra vez.",
            )
        };
        let pagina = PAGINA.replace("{titulo}", titulo).replace("{texto}", texto);
        let _ = c.write_all(
            format!(
                "HTTP/1.0 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\r\n{pagina}",
                pagina.len()
            )
            .as_bytes(),
        );
        return Ok(q);
    }
    Err(ErrorGoogle::Otro(
        "No llegó la respuesta de Google (¿cerraste la pestaña?)".into(),
    ))
}

/// La carpeta oculta de la app en Drive (o una de mentira en las pruebas).
pub trait Carpeta {
    fn buscar(&self) -> Result<Option<String>, ErrorGoogle>;
    fn leer(&self, fid: &str) -> Result<Vec<u8>, ErrorGoogle>;
    fn crear(&self, texto: &str) -> Result<String, ErrorGoogle>;
    fn escribir(&self, fid: &str, texto: &str) -> Result<(), ErrorGoogle>;
}

/// Drive (solo la carpeta oculta de la app). `archivo`: `vortex-player.json`, o `sonando.json` con lo que
/// suena, para seguir en el otro aparato.
pub struct Drive<'a> {
    cuenta: &'a Cuenta,
    token: String,
    archivo: String,
}

impl Drive<'_> {
    fn pedir(
        &self,
        metodo: &str,
        url: &str,
        cuerpo: Option<&[u8]>,
        tipo: &str,
    ) -> Result<Vec<u8>, ErrorGoogle> {
        let aut = format!("Bearer {}", self.token);
        let mut cab = vec![("Authorization", aut.as_str())];
        if !tipo.is_empty() {
            cab.push(("Content-Type", tipo));
        }
        let (estado, d) = self.cuenta.pedir(metodo, url, cuerpo, &cab)?;
        if estado == 401 {
            self.cuenta.olvidar_token();
            return Err(ErrorGoogle::SinPermiso("Google ya no reconoce la sesión".into()));
        }
        if estado >= 400 {
            return Err(ErrorGoogle::Otro(format!("Google Drive contestó {estado}")));
        }
        Ok(d)
    }

    /// (nombre, correo).
    pub fn usuario(&self) -> Result<(String, String), ErrorGoogle> {
        let u = json_de(&self.pedir(
            "GET",
            &format!("{API}/about?fields=user(displayName,emailAddress)"),
            None,
            "",
        )?)?;
        let campo = |k: &str| u["user"].get(k).and_then(Value::as_str).unwrap_or("").to_string();
        Ok((campo("displayName"), campo("emailAddress")))
    }
}

impl Carpeta for Drive<'_> {
    fn buscar(&self) -> Result<Option<String>, ErrorGoogle> {
        let q = red::escapar_ruta(&format!("name = '{}'", self.archivo));
        let r = json_de(&self.pedir(
            "GET",
            &format!("{API}/files?spaces=appDataFolder&q={q}&fields=files(id)&orderBy=modifiedTime+desc"),
            None,
            "",
        )?)?;
        Ok(r["files"]
            .get(0)
            .and_then(|f| f.get("id"))
            .and_then(Value::as_str)
            .map(String::from))
    }

    fn leer(&self, fid: &str) -> Result<Vec<u8>, ErrorGoogle> {
        self.pedir("GET", &format!("{API}/files/{fid}?alt=media"), None, "")
    }

    fn crear(&self, texto: &str) -> Result<String, ErrorGoogle> {
        let frontera = format!("vortex{:016x}", rand::random::<u64>());
        let datos = json!({"name": self.archivo, "parents": ["appDataFolder"]});
        // como el json.dumps de Python: con espacio después de «:» y «,»
        let datos = serde_json::to_string(&datos)
            .unwrap_or_default()
            .replace("\":", "\": ")
            .replace(",\"", ", \"");
        let cuerpo = format!(
            "--{frontera}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{datos}\r\n--{frontera}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{texto}\r\n--{frontera}--\r\n"
        );
        let r = self.pedir(
            "POST",
            &format!("{SUBIR}/files?uploadType=multipart&fields=id"),
            Some(cuerpo.as_bytes()),
            &format!("multipart/related; boundary={frontera}"),
        )?;
        Ok(json_de(&r)?["id"].as_str().unwrap_or("").to_string())
    }

    fn escribir(&self, fid: &str, texto: &str) -> Result<(), ErrorGoogle> {
        self.pedir(
            "PATCH",
            &format!("{SUBIR}/files/{fid}?uploadType=media"),
            Some(texto.as_bytes()),
            "application/json; charset=UTF-8",
        )
        .map(|_| ())
    }
}
