//! El nombre y el ícono de un .exe de Windows, leídos del propio archivo (formato PE), sin Wine ni
//! programas extra. Igual que `pe.py`.
//!
//! - [`nombre`]: el «ProductName» o «FileDescription» de su información de versión; si no tiene, el
//!   nombre del archivo arreglado («my_game-x64.exe» → «My Game»).
//! - [`guardar_icono`]: saca el ícono más grande (RT_GROUP_ICON) y lo guarda como .ico o .png (los de
//!   256 px suelen venir en PNG). Devuelve la ruta o `None`.
//!
//! Solo lee la cabecera y la sección de recursos (hasta 64 MB), no el .exe entero: hay juegos de varios
//! GB. Cualquier cosa rara (no es un PE, está cortado…) da `None`, nunca un pánico.

use regex::Regex;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const RT_ICON: u32 = 3;
const RT_GROUP_ICON: u32 = 14;
const RT_VERSION: u32 = 16;
const MAX_RECURSOS: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Clave {
    Id(u32),
    Nombre(String),
}

/// `{clave: datos}` en el orden en que aparecen (como un `dict` de Python).
type Recursos = Vec<(Clave, Vec<u8>)>;

struct Pe {
    base: Option<u32>,
    crudo: Vec<u8>,
}

fn leer_en(f: &mut File, pos: u64, n: usize) -> Option<Vec<u8>> {
    f.seek(SeekFrom::Start(pos)).ok()?;
    let mut b = vec![0u8; n];
    f.read_exact(&mut b).ok()?;
    Some(b)
}

fn u16_en(b: &[u8], pos: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        b.get(pos..pos.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn u32_en(b: &[u8], pos: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        b.get(pos..pos.checked_add(4)?)?.try_into().ok()?,
    ))
}

fn utf16(b: &[u8]) -> String {
    let u: Vec<u16> = b
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u16::from_le_bytes(*p))
        .collect();
    String::from_utf16_lossy(&u)
}

/// `datos[a:b]` de Python: nunca se sale.
fn trozo(datos: &[u8], a: usize, b: usize) -> &[u8] {
    let b = b.min(datos.len());
    let a = a.min(b);
    &datos[a..b]
}

impl Pe {
    fn abrir(ruta: &Path) -> Option<Pe> {
        let mut f = File::open(ruta).ok()?;
        Self::leer_cabecera(&mut f)
    }

    fn leer_cabecera(f: &mut File) -> Option<Pe> {
        if leer_en(f, 0, 2)? != b"MZ" {
            return None;
        }
        let pe = u32_en(&leer_en(f, 0x3C, 4)?, 0)? as u64;
        if leer_en(f, pe, 4)? != b"PE\0\0" {
            return None;
        }
        let coff = pe + 4;
        let n_secciones = u16_en(&leer_en(f, coff + 2, 2)?, 0)?;
        let tam_opcional = u16_en(&leer_en(f, coff + 16, 2)?, 0)? as u64;
        let opcional = coff + 20;
        let magia = u16_en(&leer_en(f, opcional, 2)?, 0)?;
        let directorios = opcional + if magia == 0x10B { 96 } else { 112 };
        let d = leer_en(f, directorios + 2 * 8, 8)?;
        let (rva_recursos, tam_recursos) = (u32_en(&d, 0)?, u32_en(&d, 4)?);
        let tabla = opcional + tam_opcional;
        let mut secciones = Vec::new();
        for i in 0..n_secciones.min(96) as u64 {
            let s = leer_en(f, tabla + 40 * i, 40)?;
            let (vtam, vdir, tam_crudo, pos_cruda) =
                (u32_en(&s, 8)?, u32_en(&s, 12)?, u32_en(&s, 16)?, u32_en(&s, 20)?);
            secciones.push((vdir, vtam.max(tam_crudo), pos_cruda));
        }
        let mut pe = Pe {
            base: None,
            crudo: Vec::new(),
        };
        if rva_recursos != 0 {
            for (vdir, tam, pos) in secciones {
                if vdir <= rva_recursos && (rva_recursos as u64) < vdir as u64 + tam as u64 {
                    pe.base = Some(rva_recursos);
                    let dentro = (rva_recursos - vdir) as u64;
                    let inicio = pos as u64 + dentro;
                    let cuanto = (tam_recursos as u64)
                        .max((tam as u64).saturating_sub(dentro))
                        .min(MAX_RECURSOS);
                    f.seek(SeekFrom::Start(inicio)).ok()?;
                    f.take(cuanto).read_to_end(&mut pe.crudo).ok()?;
                    break;
                }
            }
        }
        Some(pe)
    }

    /// (id o nombre, desplazamiento, es_carpeta) de un IMAGE_RESOURCE_DIRECTORY.
    fn entradas(&self, pos: usize) -> Option<Vec<(Clave, usize, bool)>> {
        let nombradas = u16_en(&self.crudo, pos + 12)? as usize;
        let con_id = u16_en(&self.crudo, pos + 14)? as usize;
        let mut salida = Vec::new();
        for i in 0..(nombradas + con_id).min(4096) {
            let e = pos + 16 + 8 * i;
            let (nombre, dato) = (u32_en(&self.crudo, e)?, u32_en(&self.crudo, e + 4)?);
            let clave = if nombre & 0x8000_0000 != 0 {
                let p = (nombre & 0x7FFF_FFFF) as usize;
                let largo = u16_en(&self.crudo, p)? as usize;
                Clave::Nombre(utf16(trozo(&self.crudo, p + 2, p + 2 + 2 * largo)))
            } else {
                Clave::Id(nombre & 0xFFFF)
            };
            salida.push((clave, (dato & 0x7FFF_FFFF) as usize, dato & 0x8000_0000 != 0));
        }
        Some(salida)
    }

    /// `{id: datos}` de un tipo de recurso (el primer idioma de cada uno). `None` si el archivo está raro.
    fn recursos(&self, tipo: u32) -> Option<Recursos> {
        let Some(base) = self.base else {
            return Some(Vec::new());
        };
        if self.crudo.is_empty() {
            return Some(Vec::new());
        }
        let mut salida: Recursos = Vec::new();
        for (clave, pos, carpeta) in self.entradas(0)? {
            if clave != Clave::Id(tipo) || !carpeta {
                continue;
            }
            for (rid, pos2, carpeta2) in self.entradas(pos)? {
                let mut hoja = pos2;
                if carpeta2 {
                    let idiomas = self.entradas(pos2)?;
                    let Some(primero) = idiomas.first() else {
                        continue;
                    };
                    hoja = primero.1;
                }
                let (rva, tam) = (u32_en(&self.crudo, hoja)?, u32_en(&self.crudo, hoja + 4)?);
                let ini = rva as i64 - base as i64;
                if ini >= 0 && ini + tam as i64 <= self.crudo.len() as i64 {
                    let datos = self.crudo[ini as usize..(ini + tam as i64) as usize].to_vec();
                    match salida.iter_mut().find(|(k, _)| *k == rid) {
                        Some(x) => x.1 = datos,
                        None => salida.push((rid, datos)),
                    }
                }
            }
        }
        Some(salida)
    }
}

// ---- información de versión ------------------------------------------------------------------

/// Las cadenas de StringFileInfo (ProductName, FileDescription…) de un VS_VERSIONINFO, en orden.
fn cadenas_version(datos: &[u8]) -> Vec<(String, String)> {
    fn bloque(
        datos: &[u8],
        salida: &mut Vec<(String, String)>,
        mut pos: usize,
        fin: usize,
        prof: u32,
    ) -> Option<()> {
        while pos + 6 <= fin && prof < 6 {
            let largo = u16_en(datos, pos)? as usize;
            let largo_valor = u16_en(datos, pos + 2)? as usize;
            let tipo = u16_en(datos, pos + 4)?;
            if largo == 0 {
                return Some(());
            }
            let final_ = (pos + largo).min(fin);
            let k = pos + 6;
            let mut j = k;
            while j + 1 < final_ && datos.get(j..j + 2)? != [0, 0] {
                j += 2;
            }
            let clave = utf16(trozo(datos, k, j));
            let valor_pos = (j + 2 + 3) & !3;
            let hijos = if tipo == 1 && largo_valor != 0 && prof >= 3 {
                let crudo = trozo(datos, valor_pos, valor_pos + 2 * largo_valor);
                let valor = utf16(crudo);
                let valor = valor.split('\0').next().unwrap_or("").trim().to_string();
                if !salida.iter().any(|(c, _)| *c == clave) {
                    salida.push((clave.clone(), valor));
                }
                final_
            } else if clave == "VS_VERSION_INFO" {
                (valor_pos + largo_valor + 3) & !3
            } else {
                let tam = if tipo == 1 { 2 * largo_valor } else { largo_valor };
                (valor_pos + tam + 3) & !3
            };
            if clave == "VS_VERSION_INFO"
                || clave == "StringFileInfo"
                || (prof == 2 && clave != "VarFileInfo")
            {
                bloque(datos, salida, hijos, final_, prof + 1)?;
            }
            pos = (final_ + 3) & !3;
        }
        Some(())
    }
    let mut salida = Vec::new();
    // Si el bloque está cortado, se queda con lo que alcanzó a leer (como el `except struct.error` de Python).
    let _ = bloque(datos, &mut salida, 0, datos.len(), 0);
    salida
}

fn genericos() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)^(microsoft|windows|setup|installer|launcher|game|application|app|\?+)$").unwrap()
    })
}

/// `str.title()` de Python: mayúscula tras cualquier cosa que no sea letra.
fn titulo(s: &str) -> String {
    let mut salida = String::new();
    let mut antes_letra = false;
    for c in s.chars() {
        if c.is_alphabetic() {
            if antes_letra {
                salida.extend(c.to_lowercase());
            } else {
                salida.extend(c.to_uppercase());
            }
            antes_letra = true;
        } else {
            salida.push(c);
            antes_letra = false;
        }
    }
    salida
}

/// «super_juego-x64» → «Super Juego».
pub fn bonito(stem: &str) -> String {
    static SEPARADORES: OnceLock<Regex> = OnceLock::new();
    static SOBRAN: OnceLock<Regex> = OnceLock::new();
    let s = SEPARADORES
        .get_or_init(|| Regex::new(r"[_\-.]+").unwrap())
        .replace_all(stem, " ");
    let s = SOBRAN
        .get_or_init(|| Regex::new(r"(?i)\b(x64|x86|win64|win32|64bit|32bit|shipping)\b").unwrap())
        .replace_all(&s, "");
    // «HollowKnight» → «Hollow Knight» (el `(?<=[a-z])(?=[A-Z])` de Python, sin lookaround)
    let mut partido = String::new();
    let mut previo: Option<char> = None;
    for c in s.chars() {
        if previo.is_some_and(|p| p.is_ascii_lowercase()) && c.is_ascii_uppercase() {
            partido.push(' ');
        }
        partido.push(c);
        previo = Some(c);
    }
    let mut s = partido.split_whitespace().collect::<Vec<_>>().join(" ");
    if !s.is_empty() && s == s.to_lowercase() {
        s = titulo(&s);
    }
    if s.is_empty() { stem.to_string() } else { s }
}

fn stem(ruta: &Path) -> String {
    ruta.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// El nombre del programa: su ProductName/FileDescription, o el del archivo arreglado.
pub fn nombre(ruta: &Path) -> String {
    if let Some(pe) = Pe::abrir(ruta)
        && let Some(versiones) = pe.recursos(RT_VERSION)
    {
        for (_, datos) in versiones {
            let c = cadenas_version(&datos);
            for clave in ["ProductName", "FileDescription"] {
                let v = c
                    .iter()
                    .find(|(k, _)| k == clave)
                    .map(|(_, v)| v.trim())
                    .unwrap_or("");
                if !v.is_empty() && !genericos().is_match(v) && v.chars().count() <= 60 {
                    return v.to_string();
                }
            }
        }
    }
    bonito(&stem(ruta))
}

/// (orden por tamaño y bits, entrada de 12 bytes del grupo, datos del ícono)
type Candidato<'a> = ((u32, u16), Vec<u8>, &'a Vec<u8>);

/// Guarda el ícono más grande del .exe en `destino` (sin extensión: se le pone `.png` o `.ico`).
pub fn guardar_icono(ruta: &Path, destino: &Path) -> Option<PathBuf> {
    let pe = Pe::abrir(ruta)?;
    let grupos = pe.recursos(RT_GROUP_ICON)?;
    if grupos.is_empty() {
        return None;
    }
    let iconos = pe.recursos(RT_ICON)?;
    // (orden, entrada de 12 bytes, datos)
    let mut mejor: Option<Candidato> = None;
    for (_, g) in &grupos {
        if g.len() < 6 {
            continue;
        }
        let n = u16_en(g, 4)? as usize;
        for i in 0..n {
            let p = 6 + 14 * i;
            if p + 14 > g.len() {
                break;
            }
            let ancho = g[p] as u32;
            let bits = u16_en(g, p + 6)?;
            let iid = u16_en(g, p + 12)? as u32;
            let Some((_, datos)) = iconos.iter().find(|(k, _)| *k == Clave::Id(iid)) else {
                continue;
            };
            if datos.is_empty() {
                continue;
            }
            let orden = (if ancho == 0 { 256 } else { ancho }, bits);
            if mejor.as_ref().is_none_or(|m| orden > m.0) {
                mejor = Some((orden, g[p..p + 12].to_vec(), datos));
            }
        }
        if mejor.is_some() {
            break;
        }
    }
    let (_, entrada, datos) = mejor?;
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).ok()?;
    }
    if datos.starts_with(b"\x89PNG\r\n\x1a\n") {
        let final_ = destino.with_extension("png");
        std::fs::write(&final_, datos).ok()?;
        Some(final_)
    } else {
        let final_ = destino.with_extension("ico");
        let mut c = Vec::new();
        c.extend_from_slice(&[0, 0, 1, 0, 1, 0]); // <HHH 0,1,1
        c.extend_from_slice(&entrada[..8]);
        c.extend_from_slice(&(datos.len() as u32).to_le_bytes());
        c.extend_from_slice(&22u32.to_le_bytes());
        c.extend_from_slice(datos);
        std::fs::write(&final_, c).ok()?;
        Some(final_)
    }
}
