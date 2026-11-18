//! Paquetes de otras distros (.deb de Debian/Ubuntu, .rpm de Fedora) y programas de Windows (`paquetes.py`).
//!
//! No se instalan: en Arch romperían el sistema. Aquí solo se lee su nombre y su descripción para buscar el
//! mismo programa en Flathub.

use crate::util::{nombre_de, raiz_de_nombre};
use crate::{Contexto, proc};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

fn bsdtar_salida(ctx: &Contexto, args: &[&str], entrada: Option<&[u8]>) -> Vec<u8> {
    let Some(exe) = ctx.which("bsdtar") else {
        return Vec::new();
    };
    let mut c = std::process::Command::new(exe);
    ctx.preparar(&mut c);
    c.args(args).env("LC_ALL", "C.UTF-8").env("LANG", "C.UTF-8");
    match proc::ejecutar(c, entrada, Some(Duration::from_secs(60))) {
        Ok(s) if s.ok() => s.stdout,
        _ => Vec::new(),
    }
}

/// `{nombre, version, descripcion}` del archivo control de un .deb (vacíos si no se pudo).
pub fn leer_deb(ctx: &Contexto, ruta: &Path) -> BTreeMap<&'static str, String> {
    let r = ruta.to_string_lossy();
    let control_tar = bsdtar_salida(ctx, &["-xOf", &r, "control.tar*"], None);
    let mut texto = Vec::new();
    for nombre in ["./control", "control"] {
        texto = if control_tar.is_empty() {
            Vec::new()
        } else {
            bsdtar_salida(ctx, &["-xOf", "-", nombre], Some(&control_tar))
        };
        if !texto.is_empty() {
            break;
        }
    }
    let mut campos: BTreeMap<String, String> = BTreeMap::new();
    let mut clave: Option<String> = None;
    for linea in String::from_utf8_lossy(&texto).lines() {
        if (linea.starts_with(' ') || linea.starts_with('\t')) && clave.is_some() {
            continue;
        }
        if let Some((k, v)) = linea.split_once(':') {
            let k = k.trim().to_lowercase();
            campos.insert(k.clone(), v.trim().to_string());
            clave = Some(k);
        }
    }
    let g = |k: &str| campos.get(k).cloned().unwrap_or_default();
    BTreeMap::from([("nombre", g("package")), ("version", g("version")), ("descripcion", g("description"))])
}

// Etiquetas del encabezado de un RPM
const NOMBRE: u32 = 1000;
const VERSION: u32 = 1001;
const RESUMEN: u32 = 1004;

fn u32_be(d: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(i..i + 4)?.try_into().ok()?))
}

fn encabezado(datos: &[u8], pos: usize) -> Option<(BTreeMap<u32, String>, usize)> {
    if datos.get(pos..pos + 3)? != b"\x8e\xad\xe8" {
        return None;
    }
    let n = u32_be(datos, pos + 8)? as usize;
    let tam = u32_be(datos, pos + 12)? as usize;
    let indice = pos + 16;
    let almacen = indice + n * 16;
    let mut etiquetas = BTreeMap::new();
    for i in 0..n {
        let b = indice + i * 16;
        let etiqueta = u32_be(datos, b)?;
        let tipo = u32_be(datos, b + 4)?;
        let desplazamiento = u32_be(datos, b + 8)? as usize;
        if matches!(tipo, 6 | 8 | 9) && matches!(etiqueta, NOMBRE | VERSION | RESUMEN) {
            let inicio = almacen + desplazamiento;
            let resto = datos.get(inicio..)?;
            let fin = resto.iter().position(|b| *b == 0)?;
            etiquetas.insert(etiqueta, String::from_utf8_lossy(&resto[..fin]).into_owned());
        }
    }
    Some((etiquetas, almacen + tam))
}

pub fn leer_rpm(ruta: &Path) -> BTreeMap<&'static str, String> {
    let mut datos = Vec::new();
    let mut etiquetas = BTreeMap::new();
    if let Ok(f) = std::fs::File::open(ruta)
        && f.take(1 << 20).read_to_end(&mut datos).is_ok()
        && let Some((_, mut fin)) = encabezado(&datos, 96)
    {
        // la firma, justo después de los 96 bytes iniciales, alineada a 8
        fin += (8 - fin % 8) % 8;
        if let Some((e, _)) = encabezado(&datos, fin) {
            etiquetas = e;
        }
    }
    if etiquetas.get(&NOMBRE).is_none_or(String::is_empty) {
        // respaldo: «nombre-1.2-3.fc40.x86_64.rpm»
        let n = nombre_de(ruta);
        static R: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let r = R.get_or_init(|| Regex::new(r"^(.+?)-\d[^-]*-[^-]+\.[^.]+\.rpm$").expect("regex de rpm"));
        let nombre = r
            .captures(&n)
            .map_or_else(|| raiz_de_nombre(&n).to_string(), |m| m[1].to_string());
        etiquetas.insert(NOMBRE, nombre);
    }
    let g = |k: u32| etiquetas.get(&k).cloned().unwrap_or_default();
    BTreeMap::from([("nombre", g(NOMBRE)), ("version", g(VERSION)), ("descripcion", g(RESUMEN))])
}

/// Lo que se sabe de un .deb o un .rpm: `nombre`, `version`, `descripcion` y `nombre_programa`.
pub fn leer(ctx: &Contexto, ruta: &Path, tipo: &str) -> Map<String, Value> {
    let mut datos = if tipo == "deb" { leer_deb(ctx, ruta) } else { leer_rpm(ruta) };
    if datos.get("nombre").is_none_or(String::is_empty) {
        let stem = raiz_de_nombre(&nombre_de(ruta)).to_string();
        datos.insert("nombre", stem.split('_').next().unwrap_or("").to_string());
    }
    let mut m = Map::new();
    for (k, v) in &datos {
        m.insert((*k).into(), json!(v));
    }
    m.insert("nombre_programa".into(), json!(datos["nombre"]));
    m
}

/// «DiscordSetup-1.2.3-x64.exe» → «Discord» (para buscar una versión de Linux en Flathub).
pub fn nombre_de_exe(nombre_archivo: &str) -> String {
    static EXT: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static SOBRAS: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    static FIN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let ext = EXT.get_or_init(|| Regex::new(r"(?i)\.(exe|msi)$").expect("regex"));
    let sobras = SOBRAS.get_or_init(|| {
        Regex::new(
            r"(?i)[-_ .]?(v?\d[\d.]*\w*|x64|x86|amd64|win(32|64)?|windows|installer|setup|install|full|portable|release|latest)\b",
        )
        .expect("regex")
    });
    let fin = FIN.get_or_init(|| Regex::new(r"(Setup|Installer|Install)$").expect("regex"));
    let n = ext.replace(nombre_archivo, "");
    let n = sobras.replace_all(&n, " ");
    let n = n.trim_matches(|c| " -_.".contains(c));
    let n = fin.replace(n, "");
    let n = separar_mayusculas(&n).replace(['_', '-'], " ");
    let n = n.split_whitespace().collect::<Vec<_>>().join(" ");
    if n.is_empty() { nombre_archivo.to_string() } else { n }
}

/// «MiPrograma» → «Mi Programa» (`(?<=[a-z])(?=[A-Z])`, que la biblioteca de expresiones de Rust no trae).
fn separar_mayusculas(s: &str) -> String {
    let mut r = String::new();
    let mut previo: Option<char> = None;
    for c in s.chars() {
        if previo.is_some_and(|p| p.is_ascii_lowercase()) && c.is_ascii_uppercase() {
            r.push(' ');
        }
        r.push(c);
        previo = Some(c);
    }
    r
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn nombres_de_exe() {
        assert_eq!(nombre_de_exe("DiscordSetup-1.0.9-x64.exe"), "Discord");
        assert_eq!(nombre_de_exe("OBS-Studio-30.0-Full-Installer-x64.exe"), "OBS Studio");
        assert_eq!(nombre_de_exe("MiPrograma.exe"), "Mi Programa");
        assert_eq!(nombre_de_exe("setup.exe"), "setup.exe");
    }
}
