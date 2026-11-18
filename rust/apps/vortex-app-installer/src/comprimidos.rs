//! Comprimidos: qué traen y descomprimirlos sin peligro (`comprimidos.py`).
//!
//! Todo pasa por `bsdtar` (libarchive), que lee zip, 7z, rar, iso, tar y sus variantes comprimidas. Solo si
//! `bsdtar` no puede (un .rar o un .7z con contraseña, un RAR raro) se usa `unrar` o `7z`, si están
//! instalados; si faltan, se dice qué paquete instalar. Los archivos sueltos (`.gz`, `.xz`, `.zst`, `.bz2`)
//! se descomprimen con `gzip`, `xz`, `zstd` y `bzip2` (la de Python usa sus módulos propios).
//!
//! Seguridad al descomprimir:
//! - Se descomprime primero en una carpeta temporal escondida dentro del destino y, al terminar, se mueve a
//!   su nombre final: nunca se escribe encima de nada que ya existiera (si el destino existe,
//!   [`Error::Conflicto`] propone otro nombre y la persona decide).
//! - `bsdtar` ya se niega a escribir rutas con «..» y a seguir enlaces; además, al terminar se quitan los
//!   enlaces simbólicos que apunten fuera de la carpeta, y lo que se saltó se avisa.
//! - Los programas corren sin terminal (nueva sesión), así que nunca se quedan pidiendo una contraseña.

use crate::proc;
use crate::util::{
    abspath, con_nombre, dirname, existe_o_enlace, matar_grupo, normpath, nueva_sesion, partes,
    raiz_de_nombre, sufijo, unir,
};
use crate::{Contexto, Control, Error, Resultado};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Extensiones, de la más larga a la más corta («.tar.gz» antes que «.gz»).
/// Lo que se hace con cada renglón que sale un programa externo (el texto y si vino de stderr).
type Renglon = Arc<dyn Fn(&str, bool) + Send + Sync>;

pub const EXT_TAR: &[&str] = &[
    ".tar.gz",
    ".tar.xz",
    ".tar.zst",
    ".tar.bz2",
    ".tar.lz4",
    ".tar.lzma",
    ".tar.lz",
    ".tgz",
    ".txz",
    ".tzst",
    ".tbz2",
    ".tbz",
    ".tar",
];
pub const EXT_SUELTOS: &[(&str, &str)] = &[
    (".gz", "gzip"),
    (".xz", "xz"),
    (".zst", "zstd"),
    (".bz2", "bzip2"),
];
pub const EXT_OTROS: &[&str] = &[".zip", ".rar", ".7z", ".iso"];

/// Contraseña de relleno: con ella bsdtar nunca pregunta (sin `--passphrase` intentaría leer la terminal).
const RELLENO: &str = "\u{1}vortex-sin-contraseña\u{1}";

fn paquete_de(programa: &str) -> &str {
    match programa {
        "7z" => "7zip",
        otro => otro,
    }
}

impl Error {
    /// «Para abrir este comprimido hace falta “7z”…», con el comando para instalarlo.
    pub fn falta_programa(ctx: &Contexto, programa: &str) -> Error {
        Error::FaltaPrograma {
            programa: programa.into(),
            comando: format!("sudo pacman -S {}", paquete_de(programa)),
            mensaje: ctx.tr("falta_programa", &[("programa", programa)]),
        }
    }

    pub fn contrasena(ctx: &Contexto, incorrecta: bool) -> Error {
        Error::Contrasena {
            incorrecta,
            mensaje: ctx.tr(
                if incorrecta {
                    "contrasena_mala"
                } else {
                    "contrasena_pide"
                },
                &[],
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tipo {
    Archivo,
    Carpeta,
    /// Enlace simbólico.
    Enlace,
    Duro,
}

impl Tipo {
    pub fn nombre(self) -> &'static str {
        match self {
            Tipo::Archivo => "archivo",
            Tipo::Carpeta => "carpeta",
            Tipo::Enlace => "enlace",
            Tipo::Duro => "duro",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entrada {
    /// Ruta dentro del comprimido, sin «./» ni «/» al principio.
    pub nombre: String,
    pub tipo: Tipo,
    pub tamano: u64,
    /// A dónde apunta, si es enlace.
    pub destino: String,
    /// «-rwxr-xr-x», si se sabe.
    pub modo: String,
}

impl Entrada {
    pub fn new(nombre: &str, tipo: Tipo, tamano: u64) -> Self {
        Entrada {
            nombre: nombre.into(),
            tipo,
            tamano,
            destino: String::new(),
            modo: String::new(),
        }
    }

    pub fn ejecutable(&self) -> bool {
        self.tipo == Tipo::Archivo && self.modo.len() >= 4 && self.modo[1..4].contains('x')
    }

    pub fn profundidad(&self) -> usize {
        partes(&self.nombre).len()
    }
}

/// Lo que dejó [`extraer`].
#[derive(Debug)]
pub struct Extraido {
    pub destino: PathBuf,
    pub saltados: Vec<String>,
}

// --------------------------------------------------------------------------------------- nombres

fn bajo(s: &str) -> String {
    s.to_lowercase()
}

fn termina_en_tar(n: &str) -> bool {
    EXT_TAR.iter().any(|e| n.ends_with(e))
}

/// «gzip», «xz»… si es un archivo suelto comprimido (no un .tar.gz), o `None`.
pub fn formato_suelto(ctx: &Contexto, ruta: &Path) -> Option<&'static str> {
    let n = bajo(&ruta.to_string_lossy());
    if termina_en_tar(&n) {
        return None;
    }
    for (ext, fmt) in EXT_SUELTOS {
        if n.ends_with(ext) {
            return if es_tar_por_dentro(ctx, ruta, fmt) {
                None
            } else {
                Some(fmt)
            };
        }
    }
    None
}

pub fn nombre_sin_extension(nombre: &str) -> String {
    let b = bajo(nombre);
    let todas = EXT_TAR
        .iter()
        .copied()
        .chain(EXT_OTROS.iter().copied())
        .chain(EXT_SUELTOS.iter().map(|(e, _)| *e))
        .chain([".deb", ".rpm"]);
    for ext in todas {
        if b.ends_with(ext) && nombre.len() > ext.len() && nombre.is_char_boundary(nombre.len() - ext.len()) {
            return nombre[..nombre.len() - ext.len()].to_string();
        }
    }
    let s = raiz_de_nombre(nombre);
    if s.is_empty() { nombre.into() } else { s.into() }
}

/// «Carpeta» → «Carpeta (2)» (o (3)…); «foto.txt» → «foto (2).txt».
pub fn nombre_libre(ruta: &Path) -> PathBuf {
    let nombre = crate::util::nombre_de(ruta);
    let es_carpeta = sufijo(&nombre).is_empty() || ruta.is_dir();
    let (base, ext) = if es_carpeta {
        (nombre.as_str(), "")
    } else {
        (raiz_de_nombre(&nombre), sufijo(&nombre))
    };
    let mut n = 2;
    loop {
        let candidata = con_nombre(ruta, &format!("{base} ({n}){ext}"));
        if !existe_o_enlace(&candidata) {
            return candidata;
        }
        n += 1;
    }
}

fn limpiar(nombre: &str) -> String {
    let mut n = nombre;
    while let Some(r) = n.strip_prefix("./") {
        n = r;
    }
    n.trim_start_matches('/').trim_end_matches('/').to_string()
}

// --------------------------------------------------------------------------------------- análisis

/// `true` si la entrada quiere escribir o apuntar fuera de la carpeta («../», enlaces que salen).
pub fn peligrosa(e: &Entrada) -> bool {
    if partes(&e.nombre).contains(&"..") {
        return true;
    }
    if matches!(e.tipo, Tipo::Enlace | Tipo::Duro) && !e.destino.is_empty() {
        if e.destino.starts_with('/') {
            return e.tipo == Tipo::Enlace; // un enlace duro «/x» bsdtar lo deja dentro (le quita la «/»)
        }
        let base = if e.tipo == Tipo::Enlace {
            dirname(&e.nombre)
        } else {
            String::new()
        };
        let norm = normpath(&unir(&base, &e.destino));
        return norm == ".." || norm.starts_with("../");
    }
    false
}

/// El nombre de la carpeta si todo el comprimido vive en una sola carpeta (así no se anida dos veces).
/// Lo peligroso no cuenta: no se va a descomprimir.
pub fn raiz_unica(entradas: &[Entrada]) -> Option<String> {
    let buenas: Vec<&Entrada> = entradas.iter().filter(|e| !peligrosa(e)).collect();
    if buenas.is_empty() {
        return None;
    }
    let tops: BTreeSet<&str> = buenas
        .iter()
        .filter_map(|e| partes(&e.nombre).first().copied())
        .collect();
    if tops.len() != 1 {
        return None;
    }
    let top = *tops.iter().next()?;
    if top == ".." || top == "." {
        return None;
    }
    let hay_dentro = buenas.iter().any(|e| e.profundidad() > 1);
    let es_carpeta = buenas.iter().any(|e| e.nombre == top && e.tipo == Tipo::Carpeta);
    let solo_carpeta = buenas.iter().all(|e| e.nombre != top || e.tipo == Tipo::Carpeta);
    ((hay_dentro || es_carpeta) && solo_carpeta).then(|| top.to_string())
}

pub fn tamano_total(entradas: &[Entrada]) -> u64 {
    entradas
        .iter()
        .filter(|e| e.tipo == Tipo::Archivo)
        .map(|e| e.tamano)
        .sum()
}

/// Dónde queda lo descomprimido: la carpeta que ya trae, o una con el nombre del archivo.
pub fn destino_sugerido(archivo: &Path, entradas: &[Entrada], padre: &Path) -> PathBuf {
    match raiz_unica(entradas) {
        Some(raiz) => padre.join(raiz),
        None => padre.join(nombre_sin_extension(&crate::util::nombre_de(archivo))),
    }
}

// --------------------------------------------------------------------------------------- procesos

/// Corre un programa con los mensajes en inglés y UTF-8 (los que se reconocen), sin terminal.
pub(crate) fn correr(
    ctx: &Contexto,
    programa: &str,
    args: &[String],
    limite: Option<Duration>,
) -> Resultado<proc::Salida> {
    let mut c = ctx.comando(programa);
    c.args(args).env("LC_ALL", "C.UTF-8").env("LANG", "C.UTF-8");
    match proc::ejecutar(c, None, limite) {
        Ok(s) if s.agotado => Err(Error::Mensaje(ctx.tr("comprimido_tarda", &[]))),
        Ok(s) => Ok(s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::falta_programa(ctx, programa)),
        Err(e) => Err(Error::Io(e)),
    }
}

fn s(x: &Path) -> String {
    x.to_string_lossy().into_owned()
}

pub(crate) fn bsdtar(ctx: &Contexto) -> Resultado<String> {
    ctx.which("bsdtar")
        .map(|p| s(&p))
        .ok_or_else(|| Error::falta_programa(ctx, "bsdtar"))
}

fn pide_contrasena(texto: &str) -> bool {
    let t = bajo(texto);
    t.contains("passphrase") || t.contains("password")
}

fn sin_soporte(texto: &str) -> bool {
    let t = bajo(texto);
    t.contains("encrypt") || t.contains("unsupported") || t.contains("not supported") || t.contains("rar5")
}

/// Lo que se corre para ver un archivo suelto (y su paquete): `gzip -dc`, `xz -dc`, `bzip2 -dc`, `zstd -dcq`.
fn descompresor(fmt: &str) -> (&'static str, &'static str) {
    match fmt {
        "gzip" => ("gzip", "-dc"),
        "xz" => ("xz", "-dc"),
        "bzip2" => ("bzip2", "-dc"),
        _ => ("zstd", "-dcq"),
    }
}

/// ¿Un «.gz» suelto es en realidad un tar? (mira «ustar» en lo descomprimido)
fn es_tar_por_dentro(ctx: &Contexto, ruta: &Path, fmt: &str) -> bool {
    let (exe, opcion) = descompresor(fmt);
    let Some(exe) = ctx.which(exe) else {
        return false;
    };
    let mut c = Command::new(exe);
    c.arg(opcion)
        .arg(ruta)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    nueva_sesion(&mut c);
    let Ok(mut hijo) = c.spawn() else {
        return false;
    };
    let mut cabeza = Vec::new();
    if let Some(o) = hijo.stdout.take() {
        let _ = o.take(512).read_to_end(&mut cabeza);
    }
    let _ = hijo.kill();
    let _ = hijo.wait();
    cabeza.len() > 261 && &cabeza[257..262] == b"ustar"
}

// --------------------------------------------------------------------------------------- listar

fn linea_bsdtar() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"^(?P<modo>[-dlhcbpsD][-rwxsStTl]{9})\S*\s+\d+\s+\S+\s+\S+\s+(?P<tam>\d+)\s+\S+\s+\d+\s+\S+ (?P<nombre>.*)$",
        )
        .expect("regex de bsdtar")
    })
}

fn desde_bsdtar(salida: &str) -> Vec<Entrada> {
    let mut entradas = Vec::new();
    for linea in salida.lines() {
        let Some(m) = linea_bsdtar().captures(linea) else {
            continue;
        };
        let modo = m["modo"].to_string();
        let mut nombre = m["nombre"].to_string();
        let mut destino = String::new();
        let tipo;
        if modo.starts_with('l') && nombre.contains(" -> ") {
            let (n, d) = nombre
                .split_once(" -> ")
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .unwrap_or_default();
            nombre = n;
            destino = d;
            tipo = Tipo::Enlace;
        } else if modo.starts_with('h') && nombre.contains(" link to ") {
            let (n, d) = nombre
                .split_once(" link to ")
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .unwrap_or_default();
            nombre = n;
            destino = d;
            tipo = Tipo::Duro;
        } else if modo.starts_with('d') || nombre.ends_with('/') {
            tipo = Tipo::Carpeta;
        } else {
            tipo = Tipo::Archivo;
        }
        let nombre = limpiar(&nombre);
        if nombre.is_empty() || nombre == "." {
            continue;
        }
        let tam = if tipo == Tipo::Archivo {
            m["tam"].parse().unwrap_or(0)
        } else {
            0
        };
        entradas.push(Entrada {
            nombre,
            tipo,
            tamano: tam,
            destino,
            modo,
        });
    }
    entradas
}

fn desde_7z(salida: &str) -> Vec<Entrada> {
    let mut entradas: Vec<Entrada> = Vec::new();
    let mut bloque: BTreeMap<String, String> = BTreeMap::new();
    let lineas: Vec<&str> = salida.lines().chain(std::iter::once("")).collect();
    for linea in lineas {
        if linea.trim().is_empty() {
            if let Some(path) = bloque.get("Path") {
                let carpeta = bloque.get("Folder").is_some_and(|f| f == "+")
                    || bloque.get("Attributes").is_some_and(|a| a.starts_with('D'));
                let tam = bloque
                    .get("Size")
                    .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
                    .and_then(|t| t.parse().ok())
                    .unwrap_or(0);
                let nombre = limpiar(path);
                if !nombre.is_empty() {
                    entradas.push(Entrada::new(
                        &nombre,
                        if carpeta { Tipo::Carpeta } else { Tipo::Archivo },
                        if carpeta { 0 } else { tam },
                    ));
                }
            }
            bloque.clear();
            continue;
        }
        if let Some((k, v)) = linea.split_once(" = ") {
            bloque.insert(k.trim().to_string(), v.to_string());
        }
    }
    if entradas
        .first()
        .is_some_and(|e| e.nombre.ends_with(".7z") || e.nombre.ends_with(".rar"))
    {
        entradas.remove(0);
    }
    entradas
}

fn desde_unrar(salida: &str) -> Vec<Entrada> {
    let mut bloques: Vec<BTreeMap<String, String>> = Vec::new();
    let mut bloque: BTreeMap<String, String> = BTreeMap::new();
    let lineas: Vec<&str> = salida.lines().chain(std::iter::once("")).collect();
    for linea in lineas {
        let sl = linea.trim();
        if let Some(n) = sl.strip_prefix("Name:") {
            if bloque.contains_key("Name") {
                bloques.push(std::mem::take(&mut bloque));
            }
            bloque.insert("Name".into(), n.trim().into());
        } else if !bloque.is_empty()
            && let Some((k, v)) = sl.split_once(':')
        {
            bloque.insert(k.trim().into(), v.trim().into());
        }
    }
    if bloque.contains_key("Name") {
        bloques.push(bloque);
    }
    bloques
        .iter()
        .filter_map(|b| {
            let carpeta = b.get("Type").is_some_and(|t| t.to_lowercase().starts_with("dir"));
            let tam = b.get("Size").map_or("0", String::as_str);
            let nombre = limpiar(b.get("Name")?);
            (!nombre.is_empty()).then(|| {
                Entrada::new(
                    &nombre,
                    if carpeta { Tipo::Carpeta } else { Tipo::Archivo },
                    if carpeta || tam.is_empty() || !tam.bytes().all(|c| c.is_ascii_digit()) {
                        0
                    } else {
                        tam.parse().unwrap_or(0)
                    },
                )
            })
        })
        .collect()
}

/// Lo que trae el comprimido. Da [`Error::Contrasena`], [`Error::FaltaPrograma`] o [`Error::Mensaje`].
pub fn listar(ctx: &Contexto, archivo: &Path, contrasena: Option<&str>) -> Resultado<Vec<Entrada>> {
    if let Some(fmt) = formato_suelto(ctx, archivo) {
        let mut tam = 0u64;
        if fmt == "gzip" {
            // el tamaño original va en los últimos 4 bytes (módulo 4 GiB)
            if let Ok(mut f) = std::fs::File::open(archivo) {
                use std::io::{Seek, SeekFrom};
                let mut b = [0u8; 4];
                if f.seek(SeekFrom::End(-4)).is_ok() && f.read_exact(&mut b).is_ok() {
                    tam = u64::from(u32::from_le_bytes(b));
                }
            }
        }
        let nombre = nombre_sin_extension(&crate::util::nombre_de(archivo));
        return Ok(vec![Entrada::new(&nombre, Tipo::Archivo, tam)]);
    }
    let r = correr(
        ctx,
        &bsdtar(ctx)?,
        &[
            "-tvf".into(),
            s(archivo),
            "--passphrase".into(),
            contrasena.unwrap_or(RELLENO).into(),
        ],
        Some(Duration::from_secs(600)),
    )?;
    if r.codigo == Some(0) {
        return Ok(desde_bsdtar(&r.texto_out()));
    }
    let error = r.texto_err();
    if pide_contrasena(&error) && !sin_soporte(&error) {
        return Err(Error::contrasena(ctx, contrasena.is_some()));
    }
    if let Some(otro) = otro_programa(archivo) {
        // un .rar o .7z que bsdtar no puede: con unrar o 7z, si están
        return listar_con(ctx, otro, archivo, contrasena);
    }
    Err(Error::Mensaje(ctx.tr("comprimido_danado", &[])))
}

/// El programa de respaldo para este formato: «unrar» para .rar, «7z» para .7z (y para .rar sin unrar).
fn otro_programa(archivo: &Path) -> Option<&'static str> {
    let ext = bajo(sufijo(&crate::util::nombre_de(archivo)));
    match ext.as_str() {
        ".rar" => Some("rar"),
        ".7z" => Some("7z"),
        _ => None,
    }
}

/// Resuelve «rar» a `unrar` si está instalado, si no `7z`.
fn programa_de_respaldo(ctx: &Contexto, p: &'static str) -> &'static str {
    if p == "rar" {
        if ctx.which("unrar").is_some() {
            "unrar"
        } else {
            "7z"
        }
    } else {
        p
    }
}

fn exe_7z(ctx: &Contexto) -> Resultado<String> {
    for n in ["7z", "7zz", "7za"] {
        if let Some(p) = ctx.which(n) {
            return Ok(s(&p));
        }
    }
    Err(Error::falta_programa(ctx, "7z"))
}

fn listar_con(
    ctx: &Contexto,
    programa: &'static str,
    archivo: &Path,
    contrasena: Option<&str>,
) -> Resultado<Vec<Entrada>> {
    let programa = programa_de_respaldo(ctx, programa);
    if programa == "unrar" {
        let exe = ctx
            .which("unrar")
            .ok_or_else(|| Error::falta_programa(ctx, "unrar"))?;
        let r = correr(
            ctx,
            &s(&exe),
            &[
                "lt".into(),
                format!("-p{}", contrasena.unwrap_or("-")),
                s(archivo),
            ],
            Some(Duration::from_secs(600)),
        )?;
        if r.codigo == Some(0) {
            return Ok(desde_unrar(&r.texto_out()));
        }
        if r.codigo == Some(11) || pide_contrasena(&(r.texto_out() + &r.texto_err())) {
            return Err(Error::contrasena(ctx, contrasena.is_some()));
        }
        return Err(Error::Mensaje(ctx.tr("comprimido_danado", &[])));
    }
    let r = correr(
        ctx,
        &exe_7z(ctx)?,
        &[
            "l".into(),
            "-slt".into(),
            format!("-p{}", contrasena.unwrap_or(RELLENO)),
            s(archivo),
        ],
        Some(Duration::from_secs(600)),
    )?;
    if r.codigo == Some(0) {
        return Ok(desde_7z(&r.texto_out()));
    }
    if pide_contrasena(&(r.texto_out() + &r.texto_err())) {
        return Err(Error::contrasena(ctx, contrasena.is_some()));
    }
    Err(Error::Mensaje(ctx.tr("comprimido_danado", &[])))
}

// --------------------------------------------------------------------------------------- extraer

/// Quita los enlaces simbólicos que apuntan fuera de `raiz`. Devuelve los que quitó.
fn limpiar_enlaces(raiz: &Path) -> Vec<String> {
    let base = abspath(raiz);
    let mut quitados = Vec::new();
    fn recorrer(carpeta: &Path, base: &Path, quitados: &mut Vec<String>) {
        let Ok(lista) = std::fs::read_dir(carpeta) else {
            return;
        };
        let mut hijos: Vec<PathBuf> = lista.flatten().map(|e| e.path()).collect();
        hijos.sort();
        for p in hijos {
            let Ok(m) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if m.file_type().is_symlink() {
                let Ok(destino) = std::fs::read_link(&p) else {
                    continue;
                };
                let d = destino.to_string_lossy().into_owned();
                let real = if d.starts_with('/') {
                    normpath(&d)
                } else {
                    normpath(&unir(&carpeta.to_string_lossy(), &d))
                };
                let b = base.to_string_lossy();
                if real != b && !real.starts_with(&format!("{b}/")) {
                    let _ = std::fs::remove_file(&p);
                    quitados.push(p.strip_prefix(base).unwrap_or(&p).to_string_lossy().into_owned());
                }
            } else if m.is_dir() {
                recorrer(&p, base, quitados);
            }
        }
    }
    recorrer(&base, &base, &mut quitados);
    quitados
}

/// Un .iso trae carpetas de solo lectura: sin esto no se podrían mover ni mandar a la papelera.
fn permiso_de_escritura(raiz: &Path) {
    fn recorrer(carpeta: &Path) {
        let Ok(lista) = std::fs::read_dir(carpeta) else {
            return;
        };
        for e in lista.flatten() {
            let p = e.path();
            if let Ok(m) = std::fs::symlink_metadata(&p)
                && m.is_dir()
            {
                let _ = std::fs::set_permissions(
                    &p,
                    std::fs::Permissions::from_mode(m.permissions().mode() | 0o700),
                );
                recorrer(&p);
            }
        }
    }
    recorrer(raiz);
    if let Ok(m) = std::fs::metadata(raiz) {
        let _ = std::fs::set_permissions(
            raiz,
            std::fs::Permissions::from_mode(m.permissions().mode() | 0o700),
        );
    }
}

fn borrar_arbol(raiz: &Path) {
    permiso_de_escritura(raiz);
    let _ = std::fs::remove_dir_all(raiz);
}

#[derive(Default)]
struct Avance {
    hecho: u64,
    actual: Option<String>,
    contados: BTreeSet<String>,
}

struct Terminado {
    texto: String,
    codigo: Option<i32>,
}

/// Lee la salida de `bsdtar -v` (o 7z / unrar) mientras avanza; devuelve todo lo que dijo.
///
/// bsdtar escribe «x nombre» al empezar cada archivo, pero el salto de línea hasta terminarlo: por eso se
/// lee a pedazos y el renglón a medias ya dice qué archivo se está escribiendo (y su tamaño en disco da el
/// avance dentro de él).
fn vigilar(
    hijo: &mut Child,
    temporal: &Path,
    tamanos: &BTreeMap<String, u64>,
    control: &Control,
) -> Resultado<Terminado> {
    let total = tamanos.values().sum::<u64>().max(1);
    let avance = Arc::new(Mutex::new(Avance::default()));
    let partes_leidas = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut flujos: Vec<Box<dyn Read + Send>> = Vec::new();
    if let Some(e) = hijo.stderr.take() {
        flujos.push(Box::new(e));
    }
    if let Some(o) = hijo.stdout.take() {
        flujos.push(Box::new(o)); // 7z y unrar dan el avance por stdout
    }
    let pid = hijo.id();
    let renglon: Renglon = {
        let avance = Arc::clone(&avance);
        let tamanos = tamanos.clone();
        let control = control.clone();
        Arc::new(move |texto: &str, completo: bool| {
            if let Some(resto) = texto.strip_prefix("x ") {
                let mut nombre = resto;
                if !tamanos.contains_key(nombre)
                    && let Some((a, _)) = nombre.split_once(": ")
                {
                    nombre = a;
                }
                if completo || tamanos.contains_key(&limpiar(nombre)) {
                    let nombre = limpiar(nombre);
                    let Ok(mut a) = avance.lock() else { return };
                    if a.actual.as_deref() == Some(nombre.as_str()) {
                        return;
                    }
                    if let Some(previo) = a.actual.clone()
                        && !a.contados.contains(&previo)
                    {
                        a.hecho += tamanos.get(&previo).copied().unwrap_or(0);
                        a.contados.insert(previo);
                    }
                    a.actual = Some(nombre);
                }
            } else if completo && control.hay_progreso() {
                static PORCIENTO: OnceLock<Regex> = OnceLock::new();
                let r = PORCIENTO.get_or_init(|| Regex::new(r"(\d{1,3})%").expect("regex de porciento"));
                if let Some(m) = r.captures(texto)
                    && let Ok(n) = m[1].parse::<u32>()
                {
                    control.avanzar((f64::from(n) / 100.0).min(1.0));
                }
            }
        })
    };
    let lectores: Vec<_> = flujos
        .into_iter()
        .map(|mut f| {
            let renglon = Arc::clone(&renglon);
            let todo = Arc::clone(&partes_leidas);
            std::thread::spawn(move || {
                let mut resto: Vec<u8> = Vec::new();
                let mut buf = vec![0u8; 65536];
                loop {
                    let n = match f.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    if let Ok(mut t) = todo.lock() {
                        t.extend_from_slice(&buf[..n]);
                    }
                    resto.extend_from_slice(&buf[..n]);
                    let mut trozos: Vec<Vec<u8>> = resto
                        .split(|b| *b == b'\n' || *b == b'\r')
                        .map(<[u8]>::to_vec)
                        .collect();
                    resto = trozos.pop().unwrap_or_default();
                    for c in trozos {
                        renglon(&String::from_utf8_lossy(&c), true);
                    }
                    if !resto.is_empty() {
                        renglon(&String::from_utf8_lossy(&resto), false);
                    }
                }
            })
        })
        .collect();
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(e)) => break Some(e),
            Ok(None) => {}
            Err(_) => break None,
        }
        if control.cancelado() {
            matar_grupo(pid, libc::SIGKILL);
            let _ = hijo.kill();
            let _ = hijo.wait();
            for l in lectores {
                let _ = l.join();
            }
            return Err(Error::Cancelado);
        }
        if control.hay_progreso() {
            let actual = avance.lock().ok().and_then(|a| a.actual.clone());
            if let Some(actual) = actual {
                let parcial = std::fs::symlink_metadata(temporal.join(&actual)).map_or(0, |m| m.len());
                let a = avance.lock().ok();
                let (hecho, contado) = a
                    .as_ref()
                    .map_or((0, true), |a| (a.hecho, a.contados.contains(&actual)));
                let ya = hecho
                    + if contado {
                        0
                    } else {
                        parcial.min(tamanos.get(&actual).copied().unwrap_or(0))
                    };
                control.avanzar((ya as f64 / total as f64).min(1.0));
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    };
    for l in lectores {
        let _ = l.join();
    }
    let texto =
        String::from_utf8_lossy(&partes_leidas.lock().map(|p| p.clone()).unwrap_or_default()).into_owned();
    Ok(Terminado {
        texto,
        codigo: estado.and_then(|e| e.code()),
    })
}

fn lanzar_leyendo(ctx: &Contexto, programa: &str, args: &[String], unido: bool) -> Resultado<Child> {
    let mut c = ctx.comando(programa);
    c.args(args)
        .env("LC_ALL", "C.UTF-8")
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::null());
    if unido {
        // unrar da el avance por stdout: se mira junto con lo de error
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        c.stdout(Stdio::null()).stderr(Stdio::piped());
    }
    nueva_sesion(&mut c);
    c.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::falta_programa(ctx, programa)
        } else {
            Error::Io(e)
        }
    })
}

/// Convierte lo que dijo bsdtar en el error que toca, o no hace nada si solo se saltó lo peligroso.
fn error_extraer(
    ctx: &Contexto,
    texto: &str,
    codigo: Option<i32>,
    contrasena: Option<&str>,
    nombres: &BTreeSet<String>,
) -> Resultado<()> {
    let t = bajo(texto);
    if t.contains("no space left") {
        return Err(Error::Mensaje(ctx.tr("sin_espacio", &[])));
    }
    if t.contains("passphrase") || t.contains("password") {
        return Err(Error::contrasena(ctx, contrasena.is_some()));
    }
    const INOFENSIVOS: [&str; 6] = [
        "path contains '..'",
        "cannot extract through symlink",
        "removing leading",
        "error exit delayed",
        "hard-link target",
        "absolute path",
    ];
    if codigo == Some(0) {
        return Ok(());
    }
    for linea in t.lines() {
        let l = linea.trim();
        if l.is_empty() || (l.starts_with("x ") && (!l.contains(": ") || nombres.contains(&l[2..]))) {
            continue; // solo el nombre de lo que se descomprimió: bien
        }
        if INOFENSIVOS.iter().any(|i| l.contains(i)) {
            continue; // lo peligroso que bsdtar se negó a escribir (se avisa aparte)
        }
        return Err(Error::Mensaje(ctx.tr("comprimido_danado", &[])));
    }
    Ok(())
}

fn nueva_carpeta_temporal(padre: &Path) -> std::io::Result<PathBuf> {
    Ok(tempfile::Builder::new()
        .prefix(".vortex-descomprimiendo-")
        .tempdir_in(padre)?
        .keep())
}

/// Descomprime `archivo` en `destino` (que no debe existir). Si todo viene en una sola carpeta, su
/// contenido queda directo en `destino` (no «destino/carpeta»), salvo con `quitar_raiz = false`.
pub fn extraer(
    ctx: &Contexto,
    archivo: &Path,
    destino: &Path,
    entradas: Option<&[Entrada]>,
    contrasena: Option<&str>,
    control: &Control,
    quitar_raiz: bool,
) -> Resultado<Extraido> {
    if existe_o_enlace(destino) {
        return Err(Error::Conflicto {
            ruta: destino.into(),
            sugerida: nombre_libre(destino),
        });
    }
    let padre = destino.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(padre)?;
    let fmt = formato_suelto(ctx, archivo);
    let temporal = nueva_carpeta_temporal(padre)?;
    let r = extraer_en_temporal(
        ctx,
        archivo,
        destino,
        entradas,
        contrasena,
        control,
        quitar_raiz,
        fmt,
        &temporal,
    );
    if temporal.exists() {
        borrar_arbol(&temporal);
    }
    r
}

#[allow(clippy::too_many_arguments)]
fn extraer_en_temporal(
    ctx: &Contexto,
    archivo: &Path,
    destino: &Path,
    entradas: Option<&[Entrada]>,
    contrasena: Option<&str>,
    control: &Control,
    quitar_raiz: bool,
    fmt: Option<&'static str>,
    temporal: &Path,
) -> Resultado<Extraido> {
    let conflicto = || Error::Conflicto {
        ruta: destino.into(),
        sugerida: nombre_libre(destino),
    };
    let nombre_destino = crate::util::nombre_de(destino);
    if let Some(fmt) = fmt {
        extraer_suelto(ctx, archivo, fmt, &temporal.join(&nombre_destino), control)?;
        if existe_o_enlace(destino) {
            return Err(conflicto());
        }
        std::fs::rename(temporal.join(&nombre_destino), destino)?;
        return Ok(Extraido {
            destino: destino.into(),
            saltados: vec![],
        });
    }
    let listadas;
    let entradas = match entradas {
        Some(e) => e,
        None => {
            listadas = listar(ctx, archivo, contrasena)?;
            &listadas
        }
    };
    let tamanos: BTreeMap<String, u64> = entradas
        .iter()
        .filter(|e| e.tipo == Tipo::Archivo)
        .map(|e| (e.nombre.clone(), e.tamano))
        .collect();
    extraer_en(ctx, archivo, temporal, &tamanos, contrasena, control)?;
    let mut saltados: Vec<String> = entradas
        .iter()
        .filter(|e| peligrosa(e))
        .map(|e| e.nombre.clone())
        .collect();
    for q in limpiar_enlaces(temporal) {
        if !saltados.contains(&q) {
            saltados.push(q);
        }
    }
    permiso_de_escritura(temporal);
    let raiz = if quitar_raiz { raiz_unica(entradas) } else { None };
    let origen = match &raiz {
        Some(r) if temporal.join(r).is_dir() && !temporal.join(r).is_symlink() => temporal.join(r),
        _ => temporal.to_path_buf(),
    };
    if existe_o_enlace(destino) {
        return Err(conflicto());
    }
    std::fs::rename(&origen, destino)?;
    if origen == temporal {
        std::fs::set_permissions(destino, std::fs::Permissions::from_mode(0o755))?; // mkdtemp la crea solo para ti (0700)
    }
    control.avanzar(1.0);
    Ok(Extraido {
        destino: destino.into(),
        saltados,
    })
}

fn extraer_en(
    ctx: &Contexto,
    archivo: &Path,
    temporal: &Path,
    tamanos: &BTreeMap<String, u64>,
    contrasena: Option<&str>,
    control: &Control,
) -> Resultado<()> {
    let args: Vec<String> = [
        "-xvf",
        &s(archivo),
        "-C",
        &s(temporal),
        "--no-same-owner",
        "--no-acls",
        "--no-xattrs",
        "--no-fflags",
        "--passphrase",
        contrasena.unwrap_or(RELLENO),
    ]
    .iter()
    .map(|x| (*x).to_string())
    .collect();
    let mut hijo = lanzar_leyendo(ctx, &bsdtar(ctx)?, &args, false)?;
    let fin = vigilar(&mut hijo, temporal, tamanos, control)?;
    if fin.codigo == Some(0) {
        return Ok(());
    }
    if let Some(otro) = otro_programa(archivo)
        && sin_soporte(&fin.texto)
    {
        vaciar(temporal);
        return extraer_con(ctx, otro, archivo, temporal, contrasena, control);
    }
    let nombres = tamanos.keys().map(|n| n.to_lowercase()).collect();
    error_extraer(ctx, &fin.texto, fin.codigo, contrasena, &nombres)
}

fn vaciar(carpeta: &Path) {
    permiso_de_escritura(carpeta);
    if let Ok(lista) = std::fs::read_dir(carpeta) {
        for h in lista.flatten() {
            let p = h.path();
            if p.is_dir() && !p.is_symlink() {
                let _ = std::fs::remove_dir_all(&p);
            } else {
                let _ = std::fs::remove_file(&p);
            }
        }
    }
}

fn extraer_con(
    ctx: &Contexto,
    programa: &'static str,
    archivo: &Path,
    temporal: &Path,
    contrasena: Option<&str>,
    control: &Control,
) -> Resultado<()> {
    let programa = programa_de_respaldo(ctx, programa);
    let (exe, args, unido) = if programa == "unrar" {
        let exe = ctx
            .which("unrar")
            .ok_or_else(|| Error::falta_programa(ctx, "unrar"))?;
        (
            s(&exe),
            vec![
                "x".to_string(),
                "-o-".into(),
                "-y".into(),
                format!("-p{}", contrasena.unwrap_or("-")),
                s(archivo),
                format!("{}/", temporal.display()),
            ],
            true,
        )
    } else {
        (
            exe_7z(ctx)?,
            vec![
                "x".to_string(),
                "-y".into(),
                "-bsp2".into(),
                "-bso0".into(),
                "-snl-".into(),
                format!("-p{}", contrasena.unwrap_or(RELLENO)),
                format!("-o{}", temporal.display()),
                s(archivo),
            ],
            false,
        )
    };
    let mut hijo = lanzar_leyendo(ctx, &exe, &args, unido)?;
    let fin = vigilar(&mut hijo, temporal, &BTreeMap::new(), control)?;
    if fin.codigo == Some(0) {
        return Ok(());
    }
    if fin.codigo == Some(11) || pide_contrasena(&fin.texto) {
        return Err(Error::contrasena(ctx, contrasena.is_some()));
    }
    if bajo(&fin.texto).contains("no space") {
        return Err(Error::Mensaje(ctx.tr("sin_espacio", &[])));
    }
    Err(Error::Mensaje(ctx.tr("comprimido_danado", &[])))
}

/// Un archivo suelto: se le pasa por la tubería del programa que lo descomprime, y el avance es lo que ya
/// se leyó del archivo.
fn extraer_suelto(
    ctx: &Contexto,
    archivo: &Path,
    fmt: &str,
    salida: &Path,
    control: &Control,
) -> Resultado<()> {
    let (exe, opcion) = descompresor(fmt);
    let exe = ctx.which(exe).ok_or_else(|| Error::falta_programa(ctx, exe))?;
    let danado = || Error::Mensaje(ctx.tr("comprimido_danado", &[]));
    let total = std::fs::metadata(archivo).map_or(1, |m| m.len()).max(1);
    let mut crudo = std::fs::File::open(archivo)?;
    let fuera = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(salida)
    {
        Ok(f) => f,
        Err(e) if e.raw_os_error() == Some(28) => return Err(Error::Mensaje(ctx.tr("sin_espacio", &[]))),
        Err(_) => return Err(danado()),
    };
    let mut c = Command::new(exe);
    c.arg(opcion)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(fuera))
        .stderr(Stdio::null());
    nueva_sesion(&mut c);
    let mut hijo = c.spawn()?;
    let mut entrada = hijo.stdin.take().ok_or_else(danado)?;
    let mut buf = vec![0u8; 1 << 20];
    let mut leido = 0u64;
    loop {
        if control.cancelado() {
            matar_grupo(hijo.id(), libc::SIGKILL);
            let _ = hijo.kill();
            let _ = hijo.wait();
            return Err(Error::Cancelado);
        }
        let n = crudo.read(&mut buf)?;
        if n == 0 {
            break;
        }
        if entrada.write_all(&buf[..n]).is_err() {
            break; // el programa ya terminó (archivo dañado): abajo se ve su código
        }
        leido += n as u64;
        control.avanzar((leido as f64 / total as f64).min(1.0));
    }
    drop(entrada);
    let estado = hijo.wait()?;
    if !estado.success() {
        return Err(danado());
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn e(nombre: &str, tipo: Tipo, destino: &str) -> Entrada {
        Entrada {
            nombre: nombre.into(),
            tipo,
            tamano: 1,
            destino: destino.into(),
            modo: String::new(),
        }
    }

    #[test]
    fn lo_peligroso() {
        assert!(peligrosa(&e("../fuera.txt", Tipo::Archivo, "")));
        assert!(peligrosa(&e("bien/sube", Tipo::Enlace, "../../..")));
        assert!(peligrosa(&e("bien/absoluto", Tipo::Enlace, "/etc/passwd")));
        assert!(peligrosa(&e("bien/duro", Tipo::Duro, "../fuera.txt")));
        assert!(!peligrosa(&e("bien/ok", Tipo::Enlace, "a.txt")));
        assert!(!peligrosa(&e("bien/duro", Tipo::Duro, "/etc/passwd")));
    }

    #[test]
    fn raiz_y_destino() {
        let a = vec![
            Entrada::new("P-2.0", Tipo::Carpeta, 0),
            Entrada::new("P-2.0/a.txt", Tipo::Archivo, 1),
        ];
        assert_eq!(raiz_unica(&a).as_deref(), Some("P-2.0"));
        assert_eq!(
            destino_sugerido(Path::new("/d/P-2.0.tar.gz"), &a, Path::new("/d")),
            PathBuf::from("/d/P-2.0")
        );
        let b = vec![
            Entrada::new("a.jpg", Tipo::Archivo, 1),
            Entrada::new("b.jpg", Tipo::Archivo, 1),
        ];
        assert_eq!(raiz_unica(&b), None);
        assert_eq!(
            destino_sugerido(Path::new("/d/fotos.zip"), &b, Path::new("/d")),
            PathBuf::from("/d/fotos")
        );
        // un archivo suelto en la raíz que se llama como la carpeta no cuenta como carpeta
        let c = vec![Entrada::new("x", Tipo::Archivo, 1)];
        assert_eq!(raiz_unica(&c), None);
        assert_eq!(tamano_total(&a), 1);
    }

    #[test]
    fn nombres_sin_extension() {
        assert_eq!(nombre_sin_extension("Proyecto-2.0.tar.gz"), "Proyecto-2.0");
        assert_eq!(nombre_sin_extension("a.TGZ"), "a");
        assert_eq!(nombre_sin_extension("notas.txt.gz"), "notas.txt");
        assert_eq!(nombre_sin_extension("google_amd64.deb"), "google_amd64");
        assert_eq!(nombre_sin_extension("x.txt"), "x");
        assert_eq!(nombre_sin_extension(".zip"), ".zip");
    }

    #[test]
    fn lineas_de_bsdtar() {
        let salida = "drwxr-xr-x  0 mike   mike        0 Nov 17 10:00 carpeta/\n\
                      -rwxr-xr-x  0 mike   mike       12 Nov 17 10:00 carpeta/a b.txt\n\
                      lrwxr-xr-x  0 mike   mike        0 Nov 17 10:00 carpeta/enlace -> a b.txt\n\
                      hrwxr-xr-x  0 mike   mike        0 Nov 17 10:00 carpeta/duro link to carpeta/a b.txt\n\
                      basura que no es una línea\n";
        let v = desde_bsdtar(salida);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0].tipo, Tipo::Carpeta);
        assert_eq!(v[0].nombre, "carpeta");
        assert_eq!((v[1].tamano, v[1].ejecutable()), (12, true));
        assert_eq!((v[2].tipo, v[2].destino.as_str()), (Tipo::Enlace, "a b.txt"));
        assert_eq!(
            (v[3].tipo, v[3].destino.as_str()),
            (Tipo::Duro, "carpeta/a b.txt")
        );
    }

    #[test]
    fn salida_de_7z_y_unrar() {
        let sz = "Path = x.7z\nType = 7z\n\nPath = dir\nFolder = +\nSize = 0\n\nPath = dir/a.txt\nFolder = -\nSize = 5\n";
        let v = desde_7z(sz);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].tipo, v[1].tamano), (Tipo::Carpeta, 5));
        let ur = "Name: dir\n Type: Directory\n Size: 0\n\nName: dir/a.txt\n Type: File\n Size: 7\n";
        let v = desde_unrar(ur);
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].tipo, v[1].tamano), (Tipo::Carpeta, 7));
    }

    #[test]
    fn nombres_libres() {
        let t = vortex_core::carpeta_de_prueba("nombre-libre");
        std::fs::create_dir(t.join("Carpeta")).unwrap();
        std::fs::write(t.join("foto.txt"), "x").unwrap();
        assert_eq!(nombre_libre(&t.join("Carpeta")), t.join("Carpeta (2)"));
        assert_eq!(nombre_libre(&t.join("foto.txt")), t.join("foto (2).txt"));
        std::fs::write(t.join("foto (2).txt"), "x").unwrap();
        assert_eq!(nombre_libre(&t.join("foto.txt")), t.join("foto (3).txt"));
        let _ = std::fs::remove_dir_all(&t);
    }
}
