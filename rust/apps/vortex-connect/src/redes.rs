//! Las redes wifi: cuáles abiertas hay cerca, en cuál estás y cómo entrar a una. Sin Qt.
//!
//! Linux usa `nmcli`; Windows, `netsh`. Las dos dan lo mismo al motor. Nunca se tocan redes con contraseña:
//! solo se listan las abiertas, y de la red actual solo se pregunta si es abierta.

use crate::ES_WINDOWS;
use crate::sistema::{Salida, Sistema};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::LazyLock;
use std::time::Duration;

pub const PREFIJO: &str = "Vortexine Connect: ";

/// Algo que se le puede decir a la persona tal cual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorRed(pub String);

impl std::fmt::Display for ErrorRed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErrorRed {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Red {
    pub ssid: String,
    /// 0-100
    pub senal: i32,
    pub conectada: bool,
}

impl Red {
    pub fn new(ssid: &str, senal: i32) -> Self {
        Red {
            ssid: ssid.into(),
            senal,
            conectada: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tipo {
    /// no hay nada
    #[default]
    Ninguno,
    Wifi,
    Cable,
}

impl Tipo {
    pub fn como_texto(self) -> &'static str {
        match self {
            Tipo::Ninguno => "",
            Tipo::Wifi => "wifi",
            Tipo::Cable => "cable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Actual {
    pub tipo: Tipo,
    pub ssid: String,
    pub abierta: bool,
}

impl Actual {
    pub fn wifi(ssid: &str, abierta: bool) -> Self {
        Actual {
            tipo: Tipo::Wifi,
            ssid: ssid.into(),
            abierta,
        }
    }

    pub fn cable() -> Self {
        Actual {
            tipo: Tipo::Cable,
            ..Actual::default()
        }
    }
}

/// Lo que el motor necesita de una red (el real, o uno falso en las pruebas).
pub trait Redes: Send + Sync {
    fn actual(&self) -> Actual;
    fn lista(&self, buscar: bool) -> Result<Vec<Red>, ErrorRed>;
    fn hay_wifi(&self) -> bool;
    fn conectar(&self, ssid: &str) -> Result<(), ErrorRed>;
}

impl<T: Redes + ?Sized> Redes for std::sync::Arc<T> {
    fn actual(&self) -> Actual {
        (**self).actual()
    }
    fn lista(&self, buscar: bool) -> Result<Vec<Red>, ErrorRed> {
        (**self).lista(buscar)
    }
    fn hay_wifi(&self) -> bool {
        (**self).hay_wifi()
    }
    fn conectar(&self, ssid: &str) -> Result<(), ErrorRed> {
        (**self).conectar(ssid)
    }
}

/// Cómo se corre un programa: la orden completa y el tiempo límite.
pub type Ejecutor = Box<dyn Fn(&[&str], Duration) -> Salida + Send + Sync>;

fn ejecutor_real(sistema: Sistema) -> Ejecutor {
    Box::new(move |orden, limite| sistema.correr(orden, limite))
}

const S: fn(u64) -> Duration = Duration::from_secs;

/// Una línea de «nmcli -t»: «:» separa y «\:» es un dos puntos de verdad (un SSID puede traerlo).
pub fn partir_nmcli(linea: &str) -> Vec<String> {
    let mut campos = Vec::new();
    let mut actual = String::new();
    let mut cs = linea.chars();
    while let Some(c) = cs.next() {
        match c {
            '\\' => match cs.next() {
                Some(sig) => actual.push(sig),
                None => actual.push('\\'),
            },
            ':' => campos.push(std::mem::take(&mut actual)),
            _ => actual.push(c),
        }
    }
    campos.push(actual);
    campos
}

/// Una fila por nombre (la de mejor señal: un hotel tiene muchas antenas) y las más fuertes primero.
fn unir(filas: Vec<Red>) -> Vec<Red> {
    let mut mejores: BTreeMap<String, Red> = BTreeMap::new();
    let mut orden: Vec<String> = Vec::new();
    for f in filas {
        match mejores.get_mut(&f.ssid) {
            None => {
                orden.push(f.ssid.clone());
                mejores.insert(f.ssid.clone(), f);
            }
            Some(previa) => {
                let conectada = previa.conectada || f.conectada;
                if f.senal > previa.senal {
                    *previa = Red { conectada, ..f };
                } else {
                    previa.conectada = conectada;
                }
            }
        }
    }
    let mut v: Vec<Red> = orden.into_iter().filter_map(|k| mejores.remove(&k)).collect();
    v.sort_by(|a, b| {
        b.senal
            .cmp(&a.senal)
            .then_with(|| a.ssid.to_lowercase().cmp(&b.ssid.to_lowercase()))
    });
    v
}

/// El usuario de la sesión (para que el perfil de la red solo sea suyo).
fn usuario() -> String {
    ["LOGNAME", "USER", "LNAME", "USERNAME"]
        .iter()
        .find_map(|v| std::env::var(v).ok().filter(|u| !u.is_empty()))
        .or_else(|| {
            let s = Sistema::real().correr(&["id", "-un"], S(5));
            s.bien().then(|| s.salida.trim().to_string())
        })
        .unwrap_or_default()
}

type Fila = (bool, String, i32, String);

pub struct RedesLinux {
    correr: Ejecutor,
}

impl RedesLinux {
    pub fn new(sistema: Sistema) -> Self {
        Self::con(ejecutor_real(sistema))
    }

    pub fn con(correr: Ejecutor) -> Self {
        RedesLinux { correr }
    }

    fn filas(&self, buscar: bool) -> Vec<Fila> {
        let orden = [
            "nmcli",
            "-t",
            "-e",
            "yes",
            "-f",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
            "--rescan",
            if buscar { "yes" } else { "no" },
        ];
        let r = (self.correr)(&orden, S(30));
        if r.codigo != 0 {
            return vec![];
        }
        r.salida
            .lines()
            .filter_map(|l| {
                let c = partir_nmcli(l);
                if c.len() < 4 || c[1].is_empty() {
                    return None; // sin nombre: red oculta
                }
                Some((
                    c[0].trim() == "*",
                    c[1].clone(),
                    c[2].trim().parse().unwrap_or(0),
                    c[3].trim().to_string(),
                ))
            })
            .collect()
    }
}

fn sin_seguridad(seg: &str) -> bool {
    seg.is_empty() || seg == "--"
}

impl Redes for RedesLinux {
    fn lista(&self, buscar: bool) -> Result<Vec<Red>, ErrorRed> {
        Ok(unir(
            self.filas(buscar)
                .into_iter()
                .filter(|f| sin_seguridad(&f.3))
                .map(|(usa, ssid, senal, _)| Red {
                    ssid,
                    senal,
                    conectada: usa,
                })
                .collect(),
        ))
    }

    fn actual(&self) -> Actual {
        let r = (self.correr)(&["nmcli", "-t", "-f", "TYPE,STATE", "device"], S(10));
        if r.codigo != 0 {
            return Actual::default();
        }
        let conectados: Vec<Vec<String>> = r.salida.lines().map(partir_nmcli).collect();
        let hay = |t: &str| {
            conectados
                .iter()
                .any(|c| c.len() >= 2 && c[0] == t && c[1] == "connected")
        };
        if hay("ethernet") {
            return Actual::cable();
        }
        if !hay("wifi") {
            return Actual::default();
        }
        for (usa, ssid, _, seg) in self.filas(false) {
            if usa {
                return Actual::wifi(&ssid, sin_seguridad(&seg));
            }
        }
        Actual {
            tipo: Tipo::Wifi,
            ..Actual::default()
        }
    }

    fn hay_wifi(&self) -> bool {
        let r = (self.correr)(&["nmcli", "-t", "-f", "TYPE", "device"], S(10));
        r.codigo == 0 && r.salida.lines().any(|l| l == "wifi")
    }

    /// Con un perfil propio: MAC al azar y sin volver a entrar solo, para que otra red que se llame igual no
    /// te atrape después. Si ya existe, se reutiliza.
    fn conectar(&self, ssid: &str) -> Result<(), ErrorRed> {
        let perfil = format!("{PREFIJO}{ssid}");
        let r = (self.correr)(&["nmcli", "-t", "-f", "NAME", "connection", "show"], S(10));
        let existe = r.codigo == 0
            && r.salida
                .lines()
                .any(|l| partir_nmcli(l).first().is_some_and(|n| *n == perfil));
        if !existe {
            let permisos = format!("user:{}", usuario());
            let r = (self.correr)(
                &[
                    "nmcli",
                    "connection",
                    "add",
                    "type",
                    "wifi",
                    "con-name",
                    &perfil,
                    "ssid",
                    ssid,
                    "connection.autoconnect",
                    "no",
                    "wifi.cloned-mac-address",
                    "random",
                    "connection.permissions",
                    &permisos,
                ],
                S(15),
            );
            if r.codigo != 0 {
                let motivo = r.error.trim();
                let motivo = if motivo.is_empty() { "nmcli falló" } else { motivo };
                return Err(ErrorRed(format!("No pude preparar «{ssid}»: {motivo}")));
            }
        }
        let r = (self.correr)(&["nmcli", "--wait", "40", "connection", "up", &perfil], S(45));
        if r.codigo != 0 {
            (self.correr)(&["nmcli", "connection", "delete", &perfil], S(10));
            return Err(ErrorRed(format!(
                "No se pudo entrar a «{ssid}». Puede estar muy lejos."
            )));
        }
        Ok(())
    }
}

// --- Windows ---

static AUTENTICACION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^\s*(?:Authentication|Autenticaci\S*n)\s*:\s*(.+?)\s*$").unwrap());
static ABIERTA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(?:Open|Abierta|Abierto)$").unwrap());
static SIN_CIFRAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^\s*(?:Encryption|Cifrado)\s*:\s*(?:None|Ninguno)\s*$").unwrap());
static CABECERA_SSID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*SSID\s+\d+\s*:[ \t]*(.*)$").unwrap());
static SSID_SUELTO: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*SSID\s*:[ \t]*(.+?)\s*$").unwrap());
static SENAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":\s*(\d{1,3})\s*%").unwrap());
static NOMBRE_INTERFAZ: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?im)^\s*(?:Name|Nombre)\s*:").unwrap());

/// «SSID 1 : nombre» y lo que sigue hasta el próximo. La salida viene traducida; el SSID es igual siempre.
pub fn bloques_netsh(salida: &str) -> Vec<(String, String)> {
    let marcas: Vec<_> = CABECERA_SSID.captures_iter(salida).collect();
    marcas
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let todo = c.get(0).unwrap();
            let fin = marcas
                .get(i + 1)
                .map_or(salida.len(), |s| s.get(0).unwrap().start());
            (c[1].trim().to_string(), salida[todo.end()..fin].to_string())
        })
        .collect()
}

pub fn es_abierta_netsh(bloque: &str) -> bool {
    AUTENTICACION
        .captures(bloque)
        .is_some_and(|a| ABIERTA.is_match(&a[1]))
        && SIN_CIFRAR.is_match(bloque)
}

fn escapar_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Perfil de red abierta, en manual: Windows no vuelve a entrar solo a otra que se llame igual.
pub fn perfil_windows(ssid: &str) -> String {
    let nombre = escapar_xml(ssid);
    format!(
        r#"<?xml version="1.0"?>
<WLANProfile xmlns="http://www.microsoft.com/networking/WLAN/profile/v1">
  <name>{nombre}</name>
  <SSIDConfig><SSID><name>{nombre}</name></SSID></SSIDConfig>
  <connectionType>ESS</connectionType>
  <connectionMode>manual</connectionMode>
  <MSM><security>
    <authEncryption><authentication>open</authentication><encryption>none</encryption><useOneX>false</useOneX></authEncryption>
  </security></MSM>
</WLANProfile>
"#
    )
}

pub struct RedesWindows {
    correr: Ejecutor,
}

impl RedesWindows {
    pub fn new(sistema: Sistema) -> Self {
        Self::con(ejecutor_real(sistema))
    }

    pub fn con(correr: Ejecutor) -> Self {
        RedesWindows { correr }
    }

    fn interfaz(&self) -> String {
        let r = (self.correr)(&["netsh", "wlan", "show", "interfaces"], S(10));
        if r.codigo == 0 { r.salida } else { String::new() }
    }

    fn ssid_actual(&self) -> String {
        SSID_SUELTO
            .captures(&self.interfaz())
            .map(|c| c[1].to_string())
            .unwrap_or_default()
    }
}

impl Redes for RedesWindows {
    fn lista(&self, _buscar: bool) -> Result<Vec<Red>, ErrorRed> {
        let r = (self.correr)(&["netsh", "wlan", "show", "networks", "mode=bssid"], S(20));
        if r.codigo != 0 {
            // Windows 11 pide el permiso de ubicación para ver redes wifi.
            let t = format!("{}{}", r.salida, r.error).to_lowercase();
            if t.contains("ubicaci") || t.contains("location") {
                return Err(ErrorRed(
                    "Windows pide activar Ubicación para ver las redes wifi (Configuración → Privacidad)."
                        .into(),
                ));
            }
            return Ok(vec![]);
        }
        let actual = self.ssid_actual();
        let filas = bloques_netsh(&r.salida)
            .into_iter()
            .filter(|(ssid, bloque)| !ssid.is_empty() && es_abierta_netsh(bloque))
            .map(|(ssid, bloque)| {
                let senal = SENAL
                    .captures_iter(&bloque)
                    .filter_map(|c| c[1].parse::<i32>().ok())
                    .max()
                    .unwrap_or(0);
                Red {
                    conectada: ssid == actual,
                    ssid,
                    senal,
                }
            })
            .collect();
        Ok(unir(filas))
    }

    fn actual(&self) -> Actual {
        let texto = self.interfaz();
        if let Some(c) = SSID_SUELTO.captures(&texto) {
            let abierta = AUTENTICACION
                .captures(&texto)
                .is_some_and(|a| ABIERTA.is_match(&a[1]));
            return Actual::wifi(&c[1], abierta);
        }
        // Solo adaptadores físicos: el loopback y los virtuales de WARP o Tailscale siempre están arriba.
        let r = (self.correr)(
            &[
                "powershell",
                "-NoProfile",
                "-Command",
                "@(Get-NetAdapter -Physical | Where-Object { $_.Status -eq 'Up' -and $_.PhysicalMediaType -ne 'Native 802.11' }).Count",
            ],
            S(15),
        );
        let n = r.salida.trim();
        if r.codigo == 0 && !n.is_empty() && n != "0" {
            Actual::cable()
        } else {
            Actual::default()
        }
    }

    fn hay_wifi(&self) -> bool {
        // Sin antena o sin el servicio de wifi, netsh falla o no enseña ninguna interfaz con nombre.
        NOMBRE_INTERFAZ.is_match(&self.interfaz())
    }

    fn conectar(&self, ssid: &str) -> Result<(), ErrorRed> {
        let archivo = std::env::temp_dir().join(format!(
            "vortex-connect-{}-{}.xml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos())
        ));
        std::fs::write(&archivo, perfil_windows(ssid))
            .map_err(|_| ErrorRed(format!("No pude preparar «{ssid}».")))?;
        let ruta = archivo.to_string_lossy().into_owned();
        let resultado = (|| {
            let r = (self.correr)(
                &[
                    "netsh",
                    "wlan",
                    "add",
                    "profile",
                    &format!("filename={ruta}"),
                    "user=current",
                ],
                S(15),
            );
            if r.codigo != 0 {
                return Err(ErrorRed(format!("No pude preparar «{ssid}».")));
            }
            let r = (self.correr)(
                &[
                    "netsh",
                    "wlan",
                    "connect",
                    &format!("name={ssid}"),
                    &format!("ssid={ssid}"),
                ],
                S(20),
            );
            if r.codigo != 0 {
                return Err(ErrorRed(format!(
                    "No se pudo entrar a «{ssid}». Puede estar muy lejos."
                )));
            }
            Ok(())
        })();
        let _ = std::fs::remove_file(&archivo);
        resultado
    }
}

/// Las redes de este sistema.
pub fn crear(sistema: Sistema) -> Box<dyn Redes> {
    if ES_WINDOWS {
        Box::new(RedesWindows::new(sistema))
    } else {
        Box::new(RedesLinux::new(sistema))
    }
}
