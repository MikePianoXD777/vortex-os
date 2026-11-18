//! Qué es un archivo: por sus primeros bytes (la «firma») y, si no alcanza, por su extensión (`detectar.py`).
//!
//! [`detectar`] devuelve un [`Info`] con el tipo («comprimido», «programa», «appimage», «script», «deb»,
//! «rpm», «flatpakref», «flatpak», «windows», «mod», «jar» o «desconocido»), su tamaño, de dónde vino (si el
//! navegador lo anotó) y lo que hace falta para mostrarlo. No ejecuta nada del archivo.

use crate::comprimidos::{self, Entrada, Tipo};
use crate::util::{abspath, atributo, nombre_de, partes, sitio as sitio_de};
use crate::{Contexto, Error, flatpak, instalar, mods, paquetes};
use serde_json::{Map, Value, json};
use std::io::Read;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

/// Tipos que, al usarlos, corren código del archivo (se avisa si además vienen de internet).
pub const EJECUTAN_CODIGO: &[&str] = &["appimage", "programa", "script", "windows", "jar", "mod"];

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub ruta: String,
    pub nombre: String,
    pub tamano: u64,
    pub tipo: String,
    /// «zip», «tar.gz», «iso»… o «elf», «texto»…
    pub formato: String,
    /// La página de donde se bajó, si se sabe.
    pub origen: String,
    pub de_internet: bool,
    pub entradas: Vec<Entrada>,
    /// Por qué no se pudo leer el contenido (dañado, falta un programa…).
    pub error: String,
    /// El comando para instalar lo que falta («sudo pacman -S 7zip»).
    pub falta_comando: String,
    pub extra: Map<String, Value>,
}

impl Info {
    pub fn ejecuta_codigo(&self) -> bool {
        EJECUTAN_CODIGO.contains(&self.tipo.as_str())
    }

    /// Un valor de `extra` como texto («» si no está).
    pub fn extra_texto(&self, clave: &str) -> String {
        self.extra
            .get(clave)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn extra_si(&self, clave: &str) -> bool {
        self.extra.get(clave).and_then(Value::as_bool).unwrap_or(false)
    }
}

fn cabeza(ruta: &Path, n: u64) -> Vec<u8> {
    let mut v = Vec::new();
    if let Ok(f) = std::fs::File::open(ruta) {
        let _ = f.take(n).read_to_end(&mut v);
    }
    v
}

/// La dirección de donde se bajó (Chrome, Chromium y otros la guardan en un atributo del archivo).
pub fn origen(ruta: &Path) -> String {
    for clave in ["user.xdg.origin.url", "user.xdg.referrer.url"] {
        if let Some(v) = atributo(ruta, clave) {
            let t = String::from_utf8_lossy(&v).trim().to_string();
            if !t.is_empty() {
                return t;
            }
        }
    }
    String::new()
}

/// «https://github.com/x/y/releases/…» → «github.com».
pub fn sitio(url: &str) -> String {
    sitio_de(url)
}

fn en_descargas(ctx: &Contexto, ruta: &Path) -> bool {
    abspath(ruta).starts_with(abspath(&ctx.rutas.descargas()))
}

pub fn es_texto(cab: &[u8]) -> bool {
    !cab[..cab.len().min(4096)].contains(&0)
}

/// «zip», «tar.gz», «7z»… o «» si no parece un comprimido.
pub fn formato_comprimido(nombre: &str, cab: &[u8]) -> String {
    let bajo = nombre.to_lowercase();
    for ext in comprimidos::EXT_TAR.iter().chain(comprimidos::EXT_OTROS) {
        if bajo.ends_with(ext) {
            return ext[1..].to_string();
        }
    }
    let firmas: [(&[u8], &str); 4] = [
        (b"PK\x03\x04", "zip"),
        (b"PK\x05\x06", "zip"),
        (b"Rar!\x1a\x07", "rar"),
        (b"7z\xbc\xaf\x27\x1c", "7z"),
    ];
    for (firma, fmt) in firmas {
        if cab.starts_with(firma) {
            return fmt.into();
        }
    }
    if cab.len() > 0x8006 && &cab[0x8001..0x8006] == b"CD001" {
        return "iso".into();
    }
    if cab.len() > 262 && &cab[257..262] == b"ustar" {
        return "tar".into();
    }
    for (ext, _) in comprimidos::EXT_SUELTOS {
        if bajo.ends_with(ext) {
            return ext[1..].into();
        }
    }
    let sueltos: [(&[u8], &str); 4] = [
        (b"\x1f\x8b", "gz"),
        (b"\xfd7zXZ\x00", "xz"),
        (b"\x28\xb5\x2f\xfd", "zst"),
        (b"BZh", "bz2"),
    ];
    for (firma, fmt) in sueltos {
        if cab.starts_with(firma) {
            return fmt.into();
        }
    }
    String::new()
}

fn tipo_de(ruta: &Path, cab: &[u8]) -> (&'static str, String) {
    let bajo = nombre_de(ruta).to_lowercase();
    let elf = cab.starts_with(b"\x7fELF");
    if bajo.ends_with(".appimage") || (elf && cab.len() >= 11 && matches!(&cab[8..11], b"AI\x01" | b"AI\x02"))
    {
        return ("appimage", "appimage".into());
    }
    if bajo.ends_with(".flatpakref") {
        return ("flatpakref", "flatpakref".into());
    }
    if bajo.ends_with(".flatpak") {
        return ("flatpak", "flatpak".into());
    }
    if bajo.ends_with(".deb") || cab.starts_with(b"!<arch>\ndebian") {
        return ("deb", "deb".into());
    }
    if bajo.ends_with(".rpm") || cab.starts_with(b"\xed\xab\xee\xdb") {
        return ("rpm", "rpm".into());
    }
    if bajo.ends_with(".exe")
        || bajo.ends_with(".msi")
        || cab.starts_with(b"MZ")
        || (cab.starts_with(b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1") && bajo.ends_with(".msi"))
    {
        return (
            "windows",
            if bajo.ends_with(".msi") { "msi" } else { "exe" }.into(),
        );
    }
    if bajo.ends_with(".jar") {
        return ("jar", "jar".into());
    }
    if bajo.ends_with(".sh")
        || bajo.ends_with(".run")
        || bajo.ends_with(".bin")
        || cab.starts_with(b"#!")
        || elf
    {
        return ("script", if elf { "elf" } else { "texto" }.into());
    }
    let fmt = formato_comprimido(&nombre_de(ruta), cab);
    if !fmt.is_empty() {
        return ("comprimido", fmt);
    }
    ("desconocido", String::new())
}

/// Un comprimido de programa para Linux: trae un .desktop y algún ejecutable (no muy hondo).
fn parece_programa(entradas: &[Entrada]) -> bool {
    let desktop = entradas
        .iter()
        .any(|e| e.tipo == Tipo::Archivo && e.nombre.ends_with(".desktop") && e.profundidad() <= 3);
    let exe = entradas.iter().any(|e| {
        e.ejecutable()
            && e.profundidad() <= 3
            && !partes(&e.nombre).last().is_some_and(|n| n.contains(".so"))
            && !e.nombre.ends_with(".desktop")
    });
    desktop && exe
}

pub fn detectar(ctx: &Contexto, ruta: &Path, contrasena: Option<&str>) -> Info {
    let nuevo = |tamano: u64, tipo: &str| Info {
        ruta: ruta.to_string_lossy().into_owned(),
        nombre: nombre_de(ruta),
        tamano,
        tipo: tipo.into(),
        ..Info::default()
    };
    let Ok(st) = std::fs::metadata(ruta) else {
        return nuevo(0, "falta");
    };
    if st.is_dir() {
        return nuevo(0, "carpeta");
    }
    let cab = cabeza(ruta, 0x8800);
    let (tipo, fmt) = tipo_de(ruta, &cab);
    let url = origen(ruta);
    let mut info = nuevo(st.len(), tipo);
    info.formato = fmt;
    info.de_internet = !url.is_empty() || en_descargas(ctx, ruta);
    info.origen = url;
    info.extra
        .insert("ejecutable".into(), json!(st.mode() & 0o111 != 0));
    match tipo {
        "comprimido" => {
            match comprimidos::listar(ctx, ruta, contrasena) {
                Ok(e) => info.entradas = e,
                Err(Error::Contrasena { .. }) => {
                    info.extra.insert("pide_contrasena".into(), json!(true));
                }
                Err(Error::FaltaPrograma { comando, mensaje, .. }) => {
                    info.error = mensaje;
                    info.falta_comando = comando;
                }
                Err(e) => info.error = e.to_string(),
            }
            if !info.entradas.is_empty() && parece_programa(&info.entradas) {
                info.tipo = "programa".into();
            }
        }
        "script" => info.extra.extend(instalar::vista_script(ctx, ruta, &cab)),
        "deb" | "rpm" => info.extra.extend(paquetes::leer(ctx, ruta, tipo)),
        "flatpakref" => info.extra.extend(flatpak::leer_ref(ruta)),
        "jar" => {
            if let Some(m) = mods::leer_mod(ctx, ruta) {
                info.tipo = "mod".into();
                info.extra.insert("mod".into(), Value::Object(m));
            }
        }
        "windows" => {
            info.extra.insert(
                "nombre_programa".into(),
                json!(paquetes::nombre_de_exe(&nombre_de(ruta))),
            );
        }
        _ => {}
    }
    info
}
