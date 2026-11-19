//! Mover las cámaras que giran (PTZ), como `ptz.py`.
//!
//! Dos caminos, porque no todas las Imou dejan los dos:
//!   - ONVIF (SOAP en el puerto 80): ContinuousMove mientras dejas apretada la flecha, y Stop al soltarla.
//!     La contraseña va con WS-Security (resumen SHA-1 con nonce): nunca viaja tal cual.
//!   - El CGI de Dahua: `/cgi-bin/ptz.cgi?action=start&code=Left…`, con autenticación digest.
//!
//! El primero que funcione se guarda en la cámara («onvif» o «cgi») para no volver a probar; si ninguno
//! funciona, «no», y la interfaz esconde las flechas.

use crate::camaras::Camara;
use crate::cripto::{azar, base64, sha1};
use crate::http;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const VELOCIDAD: f64 = 0.5;
const ESPERA: Duration = Duration::from_secs(4);

/// La cámara no se deja mover desde la PC (no gira, o no abre ni ONVIF ni el CGI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinPtz(pub String);

impl std::fmt::Display for SinPtz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// dirección → (x, y) para ONVIF y código para el CGI de Dahua.
pub fn direccion(nombre: &str) -> Option<((f64, f64), &'static str)> {
    Some(match nombre {
        "arriba" => ((0.0, VELOCIDAD), "Up"),
        "abajo" => ((0.0, -VELOCIDAD), "Down"),
        "izquierda" => ((-VELOCIDAD, 0.0), "Left"),
        "derecha" => ((VELOCIDAD, 0.0), "Right"),
        _ => return None,
    })
}

pub const DIRECCIONES: [&str; 4] = ["arriba", "abajo", "izquierda", "derecha"];

// --- ONVIF ---

fn escapar_xml(t: &str) -> String {
    t.replace('&', "&amp;").replace('<', "&lt;")
}

/// «2026-11-19T12:34:56.000Z», la hora UTC de WS-Security.
fn ahora_utc() -> String {
    let f = vortex_core::registro::fecha_utc(std::time::SystemTime::now()); // «AAAA-MM-DDTHH:MM:SSZ»
    let base = f.trim_end_matches('Z').replace(' ', "T");
    format!("{}.000Z", &base[..base.len().min(19)])
}

fn seguridad(usuario: &str, clave: &str) -> String {
    let nonce = azar(16);
    let creado = ahora_utc();
    let mut dato = nonce.clone();
    dato.extend_from_slice(creado.as_bytes());
    dato.extend_from_slice(clave.as_bytes());
    let resumen = base64(&sha1(&dato));
    format!(
        concat!(
            r#"<Security s:mustUnderstand="1" xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-secext-1.0.xsd">"#,
            "<UsernameToken><Username>{}</Username>",
            r#"<Password Type="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-username-token-profile-1.0#PasswordDigest">"#,
            "{}</Password>",
            r#"<Nonce EncodingType="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-soap-message-security-1.0#Base64Binary">"#,
            "{}</Nonce>",
            r#"<Created xmlns="http://docs.oasis-open.org/wss/2004/01/oasis-200401-wss-wssecurity-utility-1.0.xsd">"#,
            "{}</Created></UsernameToken></Security>"
        ),
        escapar_xml(usuario),
        resumen,
        base64(&nonce),
        creado
    )
}

pub fn sobre(cuerpo: &str, usuario: &str, clave: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"><s:Header>{}</s:Header><s:Body>{cuerpo}</s:Body></s:Envelope>"#,
        seguridad(usuario, clave)
    )
}

fn soap(ruta: &str, cuerpo: &str, cam: &Camara) -> Result<String, SinPtz> {
    let r = http::pedir(
        "POST",
        &cam.ip,
        ruta,
        &["Content-Type: application/soap+xml; charset=utf-8".into()],
        sobre(cuerpo, &cam.usuario, &cam.codigo).as_bytes(),
        ESPERA,
    )
    .map_err(|e| SinPtz(format!("ONVIF no contesta: {e}")))?;
    if r.estado >= 400 {
        return Err(SinPtz(format!("ONVIF respondió {}", r.estado)));
    }
    Ok(r.texto())
}

fn perfiles() -> &'static Mutex<HashMap<String, String>> {
    static P: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    P.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Olvida los perfiles ONVIF ya pedidos (para las pruebas).
pub fn olvidar_perfiles() {
    if let Ok(mut p) = perfiles().lock() {
        p.clear();
    }
}

/// El token del primer perfil de video (Dahua suele llamarlo «MediaProfile000»).
fn perfil(cam: &Camara) -> Result<String, SinPtz> {
    if let Some(p) = perfiles().lock().ok().and_then(|p| p.get(&cam.id).cloned()) {
        return Ok(p);
    }
    let xml = soap(
        "/onvif/media_service",
        r#"<GetProfiles xmlns="http://www.onvif.org/ver10/media/wsdl"/>"#,
        cam,
    )?;
    let re = regex::Regex::new(r#"<[^>]*Profiles[^>]*\btoken="([^"]+)""#).expect("regex fija");
    let token = re
        .captures(&xml)
        .map(|c| c[1].to_string())
        .ok_or_else(|| SinPtz("ONVIF no dio ningún perfil".into()))?;
    if let Ok(mut p) = perfiles().lock() {
        p.insert(cam.id.clone(), token.clone());
    }
    Ok(token)
}

fn numero(x: f64) -> String {
    // Como el str(float) de Python: 0.5 → «0.5», -0.5 → «-0.5», 0 → «0.0».
    if x.fract() == 0.0 {
        format!("{x:.1}")
    } else {
        x.to_string()
    }
}

pub fn onvif_mover(cam: &Camara, dir: &str) -> Result<(), SinPtz> {
    let ((x, y), _) = direccion(dir).ok_or_else(|| SinPtz("dirección rara".into()))?;
    let cuerpo = format!(
        concat!(
            r#"<ContinuousMove xmlns="http://www.onvif.org/ver20/ptz/wsdl">"#,
            "<ProfileToken>{}</ProfileToken><Velocity>",
            r#"<PanTilt x="{}" y="{}" xmlns="http://www.onvif.org/ver10/schema"/>"#,
            "</Velocity></ContinuousMove>"
        ),
        perfil(cam)?,
        numero(x),
        numero(y)
    );
    soap("/onvif/ptz_service", &cuerpo, cam).map(|_| ())
}

pub fn onvif_parar(cam: &Camara) -> Result<(), SinPtz> {
    let cuerpo = format!(
        r#"<Stop xmlns="http://www.onvif.org/ver20/ptz/wsdl"><ProfileToken>{}</ProfileToken><PanTilt>true</PanTilt><Zoom>true</Zoom></Stop>"#,
        perfil(cam)?
    );
    soap("/onvif/ptz_service", &cuerpo, cam).map(|_| ())
}

// --- CGI de Dahua ---

fn cgi(cam: &Camara, accion: &str, codigo: &str) -> Result<(), SinPtz> {
    let ruta = format!(
        "/cgi-bin/ptz.cgi?action={accion}&channel={}&code={codigo}&arg1=0&arg2=4&arg3=0",
        cam.canal
    );
    let r = http::get_digest(&cam.ip, &ruta, &cam.usuario, &cam.codigo, ESPERA)
        .map_err(|e| SinPtz(format!("el CGI no contesta: {e}")))?;
    if r.estado >= 400 {
        return Err(SinPtz(format!("el CGI respondió {}", r.estado)));
    }
    let texto = r.texto();
    if !texto.contains("OK") {
        return Err(SinPtz(format!(
            "el CGI no lo hizo: {}",
            texto.trim().chars().take(80).collect::<String>()
        )));
    }
    Ok(())
}

// --- lo que usa la interfaz ---

/// Empieza a girar. Devuelve el método que funcionó («onvif» o «cgi»).
pub fn mover(cam: &Camara, dir: &str) -> Result<String, SinPtz> {
    let (_, codigo) = direccion(dir).ok_or_else(|| SinPtz(format!("dirección rara: {dir}")))?;
    if cam.ptz == "no" {
        return Err(SinPtz("esta cámara no se deja mover".into()));
    }
    let metodos: &[&str] = match cam.ptz.as_str() {
        "onvif" => &["onvif"],
        "cgi" => &["cgi"],
        _ => &["onvif", "cgi"],
    };
    let mut ultimo = SinPtz("esta cámara no se deja mover".into());
    for &m in metodos {
        let r = if m == "onvif" {
            onvif_mover(cam, dir)
        } else {
            cgi(cam, "start", codigo)
        };
        match r {
            Ok(()) => return Ok(m.to_string()),
            Err(e) => ultimo = e,
        }
    }
    Err(ultimo)
}

/// Deja de girar, con el método que ya se sabe que funciona (`cam.ptz`).
pub fn parar(cam: &Camara, dir: &str) -> Result<(), SinPtz> {
    match cam.ptz.as_str() {
        "onvif" => onvif_parar(cam),
        "cgi" => {
            let codigos: Vec<&str> = match direccion(dir) {
                Some((_, c)) => vec![c],
                None => DIRECCIONES
                    .iter()
                    .filter_map(|d| direccion(d).map(|x| x.1))
                    .collect(),
            };
            for c in codigos {
                cgi(cam, "stop", c)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn hora_de_ws_security() {
        let h = ahora_utc();
        assert_eq!(h.len(), 24, "{h}");
        assert!(h.ends_with(".000Z") && h.as_bytes()[10] == b'T', "{h}");
    }

    #[test]
    fn numeros_como_python() {
        assert_eq!(numero(-0.5), "-0.5");
        assert_eq!(numero(0.0), "0.0");
    }
}
