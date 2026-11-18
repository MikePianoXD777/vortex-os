//! Pequeñas funciones de texto y de rutas que Python trae en su biblioteca estándar (`os.path`, `shlex`,
//! `urllib.parse`, `Path.suffix`…) y Rust no. Todo puro y probado; lo único con `unsafe` son las llamadas al
//! sistema (nueva sesión, atributos extendidos, hora local), cada una con su razón.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

// --------------------------------------------------------------------------------- rutas

/// `posixpath.normpath`: quita «.», «//» y resuelve «..» sin tocar el disco.
pub fn normpath(ruta: &str) -> String {
    if ruta.is_empty() {
        return ".".into();
    }
    let absoluta = ruta.starts_with('/');
    // POSIX: exactamente dos «/» al principio se conservan; tres o más se vuelven uno.
    let dobles = ruta.starts_with("//") && !ruta.starts_with("///");
    let mut partes: Vec<&str> = Vec::new();
    for p in ruta.split('/') {
        if p.is_empty() || p == "." {
            continue;
        }
        if p != ".." || (!absoluta && partes.is_empty()) || partes.last().is_some_and(|u| *u == "..") {
            partes.push(p);
        } else if !partes.is_empty() {
            partes.pop();
        }
    }
    let cuerpo = partes.join("/");
    let prefijo = if dobles {
        "//"
    } else if absoluta {
        "/"
    } else {
        ""
    };
    let r = format!("{prefijo}{cuerpo}");
    if r.is_empty() { ".".into() } else { r }
}

/// `posixpath.join(base, otra)`.
pub fn unir(base: &str, otra: &str) -> String {
    if otra.starts_with('/') || base.is_empty() {
        otra.to_string()
    } else if base.ends_with('/') {
        format!("{base}{otra}")
    } else {
        format!("{base}/{otra}")
    }
}

/// `os.path.abspath`.
pub fn abspath(ruta: &Path) -> PathBuf {
    let s = ruta.to_string_lossy();
    if s.starts_with('/') {
        PathBuf::from(normpath(&s))
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        PathBuf::from(normpath(&unir(&cwd.to_string_lossy(), &s)))
    }
}

/// `os.path.basename` (lo que va después de la última «/»).
pub fn basename(ruta: &str) -> &str {
    ruta.rsplit('/').next().unwrap_or("")
}

/// `posixpath.dirname`.
pub fn dirname(ruta: &str) -> String {
    match ruta.rfind('/') {
        None => String::new(),
        Some(i) => {
            let cabeza = &ruta[..=i];
            if cabeza.chars().all(|c| c == '/') {
                cabeza.to_string()
            } else {
                cabeza.trim_end_matches('/').to_string()
            }
        }
    }
}

/// `PurePosixPath(nombre).parts` para rutas relativas: sin vacíos ni «.».
pub fn partes(nombre: &str) -> Vec<&str> {
    nombre.split('/').filter(|p| !p.is_empty() && *p != ".").collect()
}

/// La extensión de `Path.suffix` («.gz» de «a.tar.gz»; vacía en «.bashrc» o «a.»).
pub fn sufijo(nombre: &str) -> &str {
    match nombre.rfind('.') {
        Some(i) if i > 0 && i + 1 < nombre.len() => &nombre[i..],
        _ => "",
    }
}

/// `Path.stem`.
pub fn raiz_de_nombre(nombre: &str) -> &str {
    &nombre[..nombre.len() - sufijo(nombre).len()]
}

pub fn nombre_de(ruta: &Path) -> String {
    ruta.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `Path.with_name`.
pub fn con_nombre(ruta: &Path, nombre: &str) -> PathBuf {
    ruta.with_file_name(nombre)
}

/// ¿Existe, aunque sea un enlace roto? (`os.path.lexists`)
pub fn existe_o_enlace(ruta: &Path) -> bool {
    std::fs::symlink_metadata(ruta).is_ok()
}

/// El texto de una ruta para mostrarla o escribirla en un archivo.
pub fn texto(ruta: &Path) -> String {
    ruta.to_string_lossy().into_owned()
}

// --------------------------------------------------------------------------------- shlex

/// `shlex.split` (POSIX, sin comentarios). `None` si las comillas no cierran.
pub fn dividir_comandos(entrada: &str) -> Option<Vec<String>> {
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut hay = false;
    let mut it = entrada.chars();
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() => {
                if hay {
                    salida.push(std::mem::take(&mut actual));
                    hay = false;
                }
            }
            '\'' => {
                hay = true;
                loop {
                    match it.next() {
                        Some('\'') => break,
                        Some(x) => actual.push(x),
                        None => return None,
                    }
                }
            }
            '"' => {
                hay = true;
                loop {
                    match it.next() {
                        Some('"') => break,
                        Some('\\') => match it.next() {
                            Some(x @ ('"' | '\\' | '$' | '`')) => actual.push(x),
                            Some(x) => {
                                actual.push('\\');
                                actual.push(x);
                            }
                            None => return None,
                        },
                        Some(x) => actual.push(x),
                        None => return None,
                    }
                }
            }
            '\\' => {
                hay = true;
                actual.push(it.next()?);
            }
            c => {
                hay = true;
                actual.push(c);
            }
        }
    }
    if hay {
        salida.push(actual);
    }
    Some(salida)
}

/// `shlex.quote`.
pub fn comillas_shell(s: &str) -> String {
    let seguro = !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "@%+=:,./-_".contains(c));
    if s.is_empty() {
        "''".into()
    } else if seguro {
        s.into()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

// --------------------------------------------------------------------------------- urllib

/// `urllib.parse.unquote`.
pub fn desescapar_url(s: &str) -> String {
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(a), Some(c)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            v.push(a * 16 + c);
            i += 3;
            continue;
        }
        v.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

fn hex(c: u8) -> Option<u8> {
    (c as char).to_digit(16).map(|d| d as u8)
}

/// `urllib.parse.quote` (deja «/» sin tocar).
pub fn escapar_url(s: &str) -> String {
    let mut r = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~/".contains(&b) {
            r.push(b as char);
        } else {
            r.push_str(&format!("%{b:02X}"));
        }
    }
    r
}

/// «https://github.com/x/y» → «github.com»; «https://www.x.org» → «x.org».
pub fn sitio(url: &str) -> String {
    let Some((_, resto)) = url.split_once("://") else {
        return String::new();
    };
    let autoridad = resto.split(['/', '?', '#']).next().unwrap_or("");
    let autoridad = autoridad.rsplit('@').next().unwrap_or("");
    let host = if let Some(r) = autoridad.strip_prefix('[') {
        r.split(']').next().unwrap_or("")
    } else {
        autoridad.split(':').next().unwrap_or("")
    };
    let host = host.to_lowercase();
    host.strip_prefix("www.").unwrap_or(&host).to_string()
}

// --------------------------------------------------------------------------------- texto

/// Letras latinas con acento a su letra base (lo que hace «NFKD y quitar lo que no es ASCII»); lo que no es
/// ASCII ni se descompone se descarta. Sin crate de normalización Unicode: es la tabla de las comunes.
pub fn a_ascii(s: &str) -> String {
    const TABLA: &[(&str, &str)] = &[
        ("àáâãäåāăąǎ", "a"),
        ("ÀÁÂÃÄÅĀĂĄǍ", "A"),
        ("çćĉċč", "c"),
        ("ÇĆĈĊČ", "C"),
        ("ďđ", "d"),
        ("ĎĐ", "D"),
        ("èéêëēĕėęě", "e"),
        ("ÈÉÊËĒĔĖĘĚ", "E"),
        ("ĝğġģ", "g"),
        ("ĜĞĠĢ", "G"),
        ("ĥħ", "h"),
        ("ĤĦ", "H"),
        ("ìíîïĩīĭįǐ", "i"),
        ("ÌÍÎÏĨĪĬĮİǏ", "I"),
        ("ĵ", "j"),
        ("Ĵ", "J"),
        ("ķ", "k"),
        ("Ķ", "K"),
        ("ĺļľŀł", "l"),
        ("ĹĻĽĿŁ", "L"),
        ("ñńņňǹ", "n"),
        ("ÑŃŅŇǸ", "N"),
        ("òóôõöōŏőǒ", "o"),
        ("ÒÓÔÕÖŌŎŐǑ", "O"),
        ("ŕŗř", "r"),
        ("ŔŖŘ", "R"),
        ("śŝşš", "s"),
        ("ŚŜŞŠ", "S"),
        ("ţťŧ", "t"),
        ("ŢŤŦ", "T"),
        ("ùúûüũūŭůűųǔ", "u"),
        ("ÙÚÛÜŨŪŬŮŰŲǓ", "U"),
        ("ŵ", "w"),
        ("Ŵ", "W"),
        ("ýÿŷ", "y"),
        ("ÝŸŶ", "Y"),
        ("źżž", "z"),
        ("ŹŻŽ", "Z"),
    ];
    s.chars()
        .filter_map(|c| {
            if c.is_ascii() {
                Some(c.to_string())
            } else {
                TABLA
                    .iter()
                    .find(|(letras, _)| letras.contains(c))
                    .map(|(_, base)| (*base).to_string())
            }
        })
        .collect()
}

// --------------------------------------------------------------------------------- sistema

/// Una fecha y hora local.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fecha {
    pub anio: i32,
    pub mes: u32,
    pub dia: u32,
    pub hora: u32,
    pub minuto: u32,
    pub segundo: u32,
}

impl Fecha {
    /// «2026-11-17T09:30:00» (`datetime.isoformat(timespec="seconds")`).
    pub fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.anio, self.mes, self.dia, self.hora, self.minuto, self.segundo
        )
    }
    /// «17/11/2026».
    pub fn dia_mes_anio(&self) -> String {
        format!("{:02}/{:02}/{:04}", self.dia, self.mes, self.anio)
    }
}

/// La hora local de ahora.
#[allow(unsafe_code)] // localtime_r no tiene versión segura en la biblioteca estándar
pub fn ahora() -> Fecha {
    // SAFETY: `time` y `localtime_r` solo escriben en `t` y `tm`, que viven en esta función.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        Fecha {
            anio: tm.tm_year + 1900,
            mes: (tm.tm_mon + 1) as u32,
            dia: tm.tm_mday as u32,
            hora: tm.tm_hour as u32,
            minuto: tm.tm_min as u32,
            segundo: tm.tm_sec as u32,
        }
    }
}

/// Que el proceso hijo viva en su propia sesión (`start_new_session=True`): sin terminal que le pida
/// cosas y con un grupo propio al que mandar señales.
#[allow(unsafe_code)] // pre_exec es unsafe por definición; setsid es async-signal-safe
pub fn nueva_sesion(cmd: &mut Command) -> &mut Command {
    use std::os::unix::process::CommandExt;
    // SAFETY: solo se llama a setsid(2) entre fork y exec, que es seguro en ese punto.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd
}

/// Manda una señal a todo el grupo de procesos (`os.killpg`).
#[allow(unsafe_code)] // killpg no tiene versión segura en la biblioteca estándar
pub fn matar_grupo(pid: u32, senal: i32) {
    // SAFETY: killpg solo envía una señal; si el grupo ya no existe, falla sin efectos.
    unsafe {
        libc::killpg(pid as libc::pid_t, senal);
    }
}

/// El atributo extendido `clave` de un archivo (`os.getxattr`), si lo tiene.
#[allow(unsafe_code)] // getxattr no tiene versión segura en la biblioteca estándar
pub fn atributo(ruta: &Path, clave: &str) -> Option<Vec<u8>> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let r = CString::new(ruta.as_os_str().as_bytes()).ok()?;
    let c = CString::new(clave).ok()?;
    let mut buf = vec![0u8; 4096];
    // SAFETY: `buf` mide lo que se le dice a getxattr y las dos cadenas terminan en cero.
    let n = unsafe { libc::getxattr(r.as_ptr(), c.as_ptr(), buf.as_mut_ptr().cast(), buf.len()) };
    if n < 0 {
        return None;
    }
    buf.truncate(n as usize);
    Some(buf)
}

/// Pone un atributo extendido (solo lo usan las pruebas). `false` si el sistema de archivos no los guarda.
#[doc(hidden)]
#[allow(unsafe_code)]
pub fn poner_atributo(ruta: &Path, clave: &str, valor: &[u8]) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let (Ok(r), Ok(c)) = (CString::new(ruta.as_os_str().as_bytes()), CString::new(clave)) else {
        return false;
    };
    // SAFETY: los punteros y el largo del valor vienen de referencias válidas durante la llamada.
    unsafe { libc::setxattr(r.as_ptr(), c.as_ptr(), valor.as_ptr().cast(), valor.len(), 0) == 0 }
}

/// Mueve un archivo o carpeta (`shutil.move`): renombra y, entre discos, copia y borra.
pub fn mover(origen: &Path, destino: &Path) -> io::Result<()> {
    match std::fs::rename(origen, destino) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
            copiar_arbol(origen, destino)?;
            let m = std::fs::symlink_metadata(origen)?;
            if m.is_dir() {
                std::fs::remove_dir_all(origen)
            } else {
                std::fs::remove_file(origen)
            }
        }
        Err(e) => Err(e),
    }
}

/// Copia un archivo, un enlace o una carpeta entera (los enlaces se copian como enlaces).
pub fn copiar_arbol(origen: &Path, destino: &Path) -> io::Result<()> {
    let m = std::fs::symlink_metadata(origen)?;
    if m.file_type().is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(origen)?, destino)
    } else if m.is_dir() {
        std::fs::create_dir(destino)?;
        for e in std::fs::read_dir(origen)? {
            let e = e?;
            copiar_arbol(&e.path(), &destino.join(e.file_name()))?;
        }
        std::fs::set_permissions(destino, m.permissions())
    } else {
        std::fs::copy(origen, destino).map(|_| ())
    }
}

/// ¿Es un ejecutable para esta persona? (`os.access(p, os.X_OK)`)
pub fn es_ejecutable(ruta: &Path) -> bool {
    acceso_x(ruta)
}

#[allow(unsafe_code)] // access(2) no tiene versión segura en la biblioteca estándar
fn acceso_x(ruta: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(r) = CString::new(ruta.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: `r` termina en cero y vive durante la llamada.
    unsafe { libc::access(r.as_ptr(), libc::X_OK) == 0 }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn normpath_como_python() {
        assert_eq!(normpath("a/b/../c"), "a/c");
        assert_eq!(normpath("../../x"), "../../x");
        assert_eq!(normpath("/../x"), "/x");
        assert_eq!(normpath("a/./b//c/"), "a/b/c");
        assert_eq!(normpath(""), ".");
        assert_eq!(normpath("a/.."), ".");
        assert_eq!(normpath("//x"), "//x");
        assert_eq!(normpath("///x"), "/x");
        assert_eq!(unir("bien", "../../.."), "bien/../../..");
        assert_eq!(normpath(&unir("bien", "../../..")), "../..");
    }

    #[test]
    fn nombres() {
        assert_eq!(sufijo("a.tar.gz"), ".gz");
        assert_eq!(sufijo(".bashrc"), "");
        assert_eq!(sufijo("a."), "");
        assert_eq!(raiz_de_nombre("foto.txt"), "foto");
        assert_eq!(dirname("a/b/c"), "a/b");
        assert_eq!(dirname("c"), "");
        assert_eq!(basename("a/b/c.png"), "c.png");
        assert_eq!(partes("a/./b//c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn comandos() {
        assert_eq!(
            dividir_comandos(r#"env A=1 "mi prog" --x 'y z' a\ b"#).unwrap(),
            vec!["env", "A=1", "mi prog", "--x", "y z", "a b"]
        );
        assert!(dividir_comandos("\"sin cerrar").is_none());
        assert_eq!(dividir_comandos("\"\" x").unwrap(), vec!["", "x"]);
        assert_eq!(comillas_shell("/home/a b/x"), "'/home/a b/x'");
        assert_eq!(comillas_shell("/home/ab/x"), "/home/ab/x");
        assert_eq!(comillas_shell("it's"), "'it'\"'\"'s'");
        assert_eq!(comillas_shell(""), "''");
    }

    #[test]
    fn urls() {
        assert_eq!(desescapar_url("/home/a%20b/%C3%B1"), "/home/a b/ñ");
        assert_eq!(escapar_url("/home/a b/ñ"), "/home/a%20b/%C3%B1");
        assert_eq!(sitio("https://www.github.com/x/y/releases"), "github.com");
        assert_eq!(sitio("https://User@Sitio.ORG:8080/x"), "sitio.org");
        assert_eq!(sitio("no es una url"), "");
    }

    #[test]
    fn ascii() {
        assert_eq!(a_ascii("Visualización ñandú Çé"), "Visualizacion nandu Ce");
        assert_eq!(a_ascii("日本 ok"), " ok");
    }
}
