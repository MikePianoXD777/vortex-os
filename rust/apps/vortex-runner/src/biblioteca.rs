//! La biblioteca: los programas y juegos de Windows que agregaste, en `biblioteca.json` (el mismo archivo y
//! formato que `biblioteca.py`, así que la app de Python y la de Rust comparten la biblioteca).
//!
//! Cada entrada: `{id, nombre, exe, prefijo, proton (id o "" = el de serie), argumentos, icono, agregado,
//! ultima_vez, minutos, lanzador}`. El prefijo es la carpeta de Wine de ese programa (su «disco C:» y su
//! registro): `prefijos/<id>`. Un programa instalado con «Instalar programa» vive dentro de su prefijo.
//!
//! Guardar es atómico (archivo nuevo y renombrar): si se corta la luz, no se pierde la biblioteca. Las
//! claves que esta app no conoce (de una versión más nueva) se conservan al guardar.

use crate::pe;
use crate::rutas::{self, Rutas, normalizar, resolver};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub const EXTENSIONES: [&str; 4] = [".exe", ".msi", ".bat", ".lnk"];
pub const CAMPOS_EDITABLES: [&str; 4] = ["nombre", "proton", "argumentos", "exe"];

/// Algo que se le puede decir tal cual a la persona.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorBiblioteca(pub String);

impl fmt::Display for ErrorBiblioteca {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for ErrorBiblioteca {}

/// Un programa de la biblioteca.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Entrada {
    pub id: String,
    pub nombre: String,
    pub exe: String,
    pub prefijo: String,
    pub proton: String,
    pub argumentos: String,
    pub icono: String,
    pub agregado: i64,
    pub ultima_vez: i64,
    pub minutos: i64,
    pub lanzador: bool,
    #[serde(flatten)]
    pub otras: Map<String, Value>,
}

impl Entrada {
    /// Lee una entrada del JSON con tolerancia (cualquier clave puede faltar o venir con otro tipo).
    fn de_json(v: &Value) -> Option<Entrada> {
        let m = v.as_object()?;
        let texto = |k: &str| m.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let numero = |k: &str| {
            m.get(k)
                .and_then(|v| v.as_i64().or_else(|| v.as_f64().map(|f| f as i64)))
                .unwrap_or(0)
        };
        let id = texto("id");
        if id.is_empty() {
            return None;
        }
        const CONOCIDAS: [&str; 11] = [
            "id",
            "nombre",
            "exe",
            "prefijo",
            "proton",
            "argumentos",
            "icono",
            "agregado",
            "ultima_vez",
            "minutos",
            "lanzador",
        ];
        Some(Entrada {
            id,
            nombre: texto("nombre"),
            exe: texto("exe"),
            prefijo: texto("prefijo"),
            proton: texto("proton"),
            argumentos: texto("argumentos"),
            icono: texto("icono"),
            agregado: numero("agregado"),
            ultima_vez: numero("ultima_vez"),
            minutos: numero("minutos"),
            lanzador: m.get("lanzador").and_then(Value::as_bool).unwrap_or(false),
            otras: m
                .iter()
                .filter(|(k, _)| !CONOCIDAS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        })
    }
}

fn no_programa() -> &'static Regex {
    // Lo que deja un instalador y no es el programa: desinstaladores, otros instaladores, ayudantes.
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(unins|uninst|setup|install|redist|vcredist|vc_redist|directx|dxsetup|dotnet|ndp\d|crash|report|helper|update|updater|elevate|launcher_helper|cefsharp|ue4prereq|prereq|easyanticheat_setup|notification_helper|unity ?crash)",
        )
        .unwrap()
    })
}

fn de_serie_re() -> &'static Regex {
    // Los programas que Wine/Proton ponen de serie en cada prefijo nuevo (WordPad, Internet Explorer…).
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)^program files( \(x86\))?/(common files|internet explorer|windows nt|windows media player|windows mail|windows photo viewer|windows defender|windows sidebar|windows journal|windowspowershell|microsoft\.net|msbuild|reference assemblies)/",
        )
        .unwrap()
    })
}

/// Los archivos que Proton copió al crear el prefijo (su lista «tracked_files»), en minúsculas.
fn de_serie(prefijo: &Path) -> BTreeSet<String> {
    std::fs::read(prefijo.join("tracked_files"))
        .map(|b| {
            String::from_utf8_lossy(&b)
                .lines()
                .map(|l| l.trim().to_lowercase())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Letra sin acento (el `NFKD` + ASCII de Python, para los acentos del español y del latín común).
fn sin_acento(c: char) -> Option<char> {
    if c.is_ascii() {
        return Some(c);
    }
    let base = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' => 'E',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
        'ñ' => 'n',
        'Ñ' => 'N',
        'ç' => 'c',
        'Ç' => 'C',
        'ý' | 'ÿ' => 'y',
        'Ý' => 'Y',
        _ => return None,
    };
    Some(base)
}

fn slug(texto: &str) -> String {
    let mut s = String::new();
    let mut guion = false;
    for c in texto.chars().filter_map(sin_acento) {
        if c.is_ascii_alphanumeric() {
            if guion && !s.is_empty() {
                s.push('-');
            }
            guion = false;
            s.push(c.to_ascii_lowercase());
        } else {
            guion = true;
        }
    }
    let s: String = s.chars().take(32).collect();
    if s.is_empty() { "programa".into() } else { s }
}

fn nuevo_id(nombre: &str, exe: &str) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut h = DefaultHasher::new();
    (
        exe,
        ns,
        CONTADOR.fetch_add(1, Ordering::SeqCst),
        std::process::id(),
    )
        .hash(&mut h);
    format!("{}-{}", slug(nombre), &format!("{:016x}", h.finish())[..6])
}

pub fn parece_instalador(ruta: &Path) -> bool {
    let n = ruta
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    n.ends_with(".msi") || n.contains("setup") || n.contains("install") || n.contains("instal")
}

fn ahora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// La ruta con la que se abre Runner desde un acceso: el propio programa si se llama «vortex-runner…», o
/// `vortex-runner` a secas (el PATH), entre comillas por si tiene espacios.
pub fn comando_por_defecto() -> String {
    match std::env::current_exe() {
        Ok(exe)
            if exe
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("vortex-runner")) =>
        {
            format!("\"{}\"", exe.display())
        }
        _ => "vortex-runner".into(),
    }
}

/// Un .exe que dejó un instalador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExeNuevo {
    pub exe: String,
    pub nombre: String,
    pub tam: u64,
}

/// La biblioteca de una persona (o de una prueba: lo único que cambia son las rutas).
#[derive(Debug, Clone)]
pub struct Biblioteca {
    pub rutas: Rutas,
    /// Lo que va en `Exec=` de los accesos, antes de `--correr ID`.
    pub comando: String,
}

impl Biblioteca {
    pub fn new(rutas: Rutas) -> Self {
        Biblioteca {
            rutas,
            comando: comando_por_defecto(),
        }
    }

    pub fn cargar(&self) -> Vec<Entrada> {
        let Ok(texto) = std::fs::read(self.rutas.biblioteca()) else {
            return Vec::new();
        };
        let Ok(Value::Object(datos)) = serde_json::from_slice::<Value>(&texto) else {
            return Vec::new();
        };
        match datos.get("programas") {
            Some(Value::Array(l)) => l.iter().filter_map(Entrada::de_json).collect(),
            _ => Vec::new(),
        }
    }

    pub fn guardar(&self, programas: &[Entrada]) -> std::io::Result<()> {
        let archivo = self.rutas.biblioteca();
        if let Some(p) = archivo.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = archivo.with_extension("tmp");
        let mut salida = Vec::new();
        let formato = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut salida, formato);
        #[derive(Serialize)]
        struct Archivo<'a> {
            version: u32,
            programas: &'a [Entrada],
        }
        Archivo {
            version: 1,
            programas,
        }
        .serialize(&mut ser)
        .map_err(std::io::Error::other)?;
        std::fs::write(&tmp, salida)?;
        std::fs::rename(tmp, archivo)
    }

    pub fn por_id(&self, pid: &str) -> Option<Entrada> {
        self.cargar().into_iter().find(|e| e.id == pid)
    }

    pub fn por_exe(&self, exe: &Path) -> Option<Entrada> {
        let real = resolver(&rutas::expandir(exe));
        self.cargar()
            .into_iter()
            .find(|e| resolver(Path::new(&e.exe)) == real)
    }

    pub fn nuevo_prefijo(&self, nombre: &str) -> std::io::Result<PathBuf> {
        let p = self.rutas.prefijos().join(nuevo_id(nombre, ""));
        std::fs::create_dir_all(&p)?;
        Ok(p)
    }

    /// Agrega un .exe (o .msi, .bat) a la biblioteca, con su nombre e ícono. Si ya estaba, lo devuelve.
    pub fn agregar(
        &self,
        exe: &Path,
        nombre: Option<&str>,
        prefijo: Option<&Path>,
    ) -> Result<Entrada, ErrorBiblioteca> {
        let exe = rutas::expandir(exe);
        let corto = exe
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !exe.is_file() {
            return Err(ErrorBiblioteca(format!("No encuentro {corto}.")));
        }
        let ext = exe
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy().to_lowercase()))
            .unwrap_or_default();
        if !EXTENSIONES.contains(&ext.as_str()) {
            return Err(ErrorBiblioteca(format!(
                "{corto} no es un programa de Windows (.exe, .msi o .bat)."
            )));
        }
        let mut programas = self.cargar();
        if let Some(ya) = self.por_exe(&exe) {
            return Ok(ya);
        }
        let nombre = nombre
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| pe::nombre(&exe));
        let nombre = match nombre.trim() {
            "" => exe
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            n => n.to_string(),
        };
        let pid = nuevo_id(&nombre, &exe.to_string_lossy());
        let icono = pe::guardar_icono(&exe, &self.rutas.iconos().join(&pid));
        // Un .exe que vive dentro de uno de nuestros prefijos (lo instalaste con Runner) corre en ese, con
        // su registro y sus dependencias; si no, uno nuevo para él.
        let prefijo = match prefijo {
            Some(p) => p.to_path_buf(),
            None => self
                .prefijo_de(&exe)
                .unwrap_or_else(|| self.rutas.prefijos().join(&pid)),
        };
        std::fs::create_dir_all(&prefijo)
            .map_err(|e| ErrorBiblioteca(format!("No pude crear su prefijo: {e}")))?;
        let entrada = Entrada {
            id: pid,
            nombre,
            exe: resolver(&exe).to_string_lossy().into_owned(),
            prefijo: prefijo.to_string_lossy().into_owned(),
            proton: String::new(),
            argumentos: String::new(),
            icono: icono
                .map(|i| i.to_string_lossy().into_owned())
                .unwrap_or_default(),
            agregado: ahora(),
            ultima_vez: 0,
            minutos: 0,
            lanzador: false,
            otras: Map::new(),
        };
        programas.push(entrada.clone());
        self.guardar(&programas)
            .map_err(|e| ErrorBiblioteca(format!("No pude guardar tu biblioteca: {e}")))?;
        Ok(entrada)
    }

    /// Cambia nombre, proton, argumentos o exe (otras claves se ignoran).
    pub fn cambiar(&self, pid: &str, campos: &[(&str, &str)]) -> Result<Entrada, ErrorBiblioteca> {
        let mut programas = self.cargar();
        let Some(e) = programas.iter_mut().find(|e| e.id == pid) else {
            return Err(ErrorBiblioteca(
                "Ese programa ya no está en tu biblioteca.".into(),
            ));
        };
        for (k, v) in campos {
            if !CAMPOS_EDITABLES.contains(k) {
                continue;
            }
            let v = v.trim().to_string();
            match *k {
                "nombre" => {
                    if !v.is_empty() {
                        e.nombre = v
                    }
                }
                "proton" => e.proton = v,
                "argumentos" => e.argumentos = v,
                _ => e.exe = v,
            }
        }
        let mut e = e.clone();
        self.guardar(&programas)
            .map_err(|x| ErrorBiblioteca(format!("No pude guardar tu biblioteca: {x}")))?;
        if e.lanzador {
            let _ = self.crear_lanzador(&mut e);
        }
        Ok(e)
    }

    pub fn sumar_tiempo(&self, pid: &str, segundos: f64, inicio: f64) {
        let mut programas = self.cargar();
        let Some(e) = programas.iter_mut().find(|e| e.id == pid) else {
            return;
        };
        e.ultima_vez = inicio as i64;
        e.minutos += (segundos / 60.0).round().max(0.0) as i64;
        let _ = self.guardar(&programas);
    }

    /// Lo saca de la biblioteca. Con `borrar_prefijo`, también su prefijo (lo instalado y sus partidas).
    pub fn quitar(&self, pid: &str, borrar_prefijo: bool) {
        let programas = self.cargar();
        let Some(e) = programas.iter().find(|e| e.id == pid).cloned() else {
            return;
        };
        let mut e2 = e.clone();
        self.quitar_lanzador(&mut e2);
        let otros: Vec<Entrada> = self.cargar().into_iter().filter(|x| x.id != pid).collect();
        let _ = self.guardar(&otros);
        if !e.icono.is_empty() {
            let _ = std::fs::remove_file(&e.icono);
        }
        let pref = Path::new(&e.prefijo);
        // Solo se borra si es nuestro (dentro de prefijos/) y nadie más lo usa
        let es_nuestro = pref.ancestors().skip(1).any(|a| a == self.rutas.prefijos());
        if borrar_prefijo && es_nuestro && !otros.iter().any(|x| x.prefijo == e.prefijo) {
            let _ = std::fs::remove_dir_all(pref);
        }
    }

    // ---- después de un instalador ----------------------------------------------------------

    /// El prefijo de Runner donde está este .exe (`prefijos/<id>/drive_c/…`), o `None`.
    pub fn prefijo_de(&self, exe: &Path) -> Option<PathBuf> {
        let rel = resolver(&rutas::expandir(exe))
            .strip_prefix(resolver(&self.rutas.prefijos()))
            .ok()?
            .to_path_buf();
        let partes: Vec<_> = rel.components().collect();
        if partes.len() >= 3 && partes[1].as_os_str() == "drive_c" {
            return Some(self.rutas.prefijos().join(partes[0].as_os_str()));
        }
        None
    }

    /// Borra un prefijo que no usa ningún programa de tu biblioteca (un instalador que no dejó nada, o del
    /// que dijiste «Ninguno»). Solo dentro de `prefijos/`. Devuelve si lo borró.
    pub fn descartar_prefijo(&self, prefijo: &Path) -> bool {
        let es_nuestro = prefijo.ancestors().skip(1).any(|a| a == self.rutas.prefijos());
        let lo_usan = self
            .cargar()
            .iter()
            .any(|x| normalizar(&x.prefijo) == normalizar(&prefijo.to_string_lossy()));
        if !es_nuestro || lo_usan {
            return false;
        }
        let _ = std::fs::remove_dir_all(prefijo);
        true
    }

    // ---- lanzador (.desktop) ---------------------------------------------------------------

    fn desktop(&self, e: &Entrada) -> PathBuf {
        self.rutas
            .lanzadores
            .join(format!("vortex-runner-{}.desktop", e.id))
    }

    /// Un acceso en el lanzador de apps: lo abre con Vortexine's Runner, con su Proton y su prefijo.
    pub fn crear_lanzador(&self, e: &mut Entrada) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.rutas.lanzadores)?;
        let nombre = e.nombre.replace('\n', " ");
        let icono = if e.icono.is_empty() {
            "vortex-runner"
        } else {
            &e.icono
        };
        let texto = format!(
            "[Desktop Entry]\nType=Application\nName={nombre}\nComment=Programa de Windows (Vortexine's Runner)\n\
             Exec={} --correr {}\nIcon={icono}\nTerminal=false\nCategories=Game;\nStartupNotify=false\n",
            self.comando, e.id
        );
        let destino = self.desktop(e);
        std::fs::write(&destino, texto)?;
        if !e.lanzador {
            self.marcar_lanzador(&e.id, true);
            e.lanzador = true;
        }
        Ok(destino)
    }

    pub fn quitar_lanzador(&self, e: &mut Entrada) {
        let _ = std::fs::remove_file(self.desktop(e));
        if e.lanzador {
            self.marcar_lanzador(&e.id, false);
            e.lanzador = false;
        }
    }

    fn marcar_lanzador(&self, pid: &str, valor: bool) {
        let mut programas = self.cargar();
        if let Some(x) = programas.iter_mut().find(|x| x.id == pid) {
            x.lanzador = valor;
            let _ = self.guardar(&programas);
        }
    }
}

/// Todos los `.exe` de `prefijo/drive_c` (sin el Windows de mentira de Proton).
pub fn exes_en(prefijo: &Path) -> BTreeSet<String> {
    fn recorrer(carpeta: &Path, raiz: &Path, salida: &mut BTreeSet<String>) {
        let Ok(rd) = std::fs::read_dir(carpeta) else {
            return;
        };
        for e in rd.flatten() {
            let nombre = e.file_name().to_string_lossy().into_owned();
            let Ok(tipo) = e.file_type() else {
                continue;
            };
            if tipo.is_dir() {
                // El Windows de mentira de Proton no es tuyo
                if carpeta == raiz && ["windows", "programdata"].contains(&nombre.to_lowercase().as_str()) {
                    continue;
                }
                recorrer(&e.path(), raiz, salida);
            } else if nombre.to_lowercase().ends_with(".exe") {
                salida.insert(e.path().to_string_lossy().into_owned());
            }
        }
    }
    let raiz = prefijo.join("drive_c");
    let mut salida = BTreeSet::new();
    if raiz.is_dir() {
        recorrer(&raiz, &raiz, &mut salida);
    }
    salida
}

/// Los .exe que dejó un instalador, el programa más probable primero.
pub fn exes_nuevos(prefijo: &Path, antes: &BTreeSet<String>) -> Vec<ExeNuevo> {
    let raiz = prefijo.join("drive_c");
    let de_serie = de_serie(prefijo);
    let mut salida = Vec::new();
    for ruta in exes_en(prefijo).difference(antes) {
        let nombre_archivo = Path::new(ruta)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if no_programa().is_match(&nombre_archivo) {
            continue;
        }
        let relativa = Path::new(ruta)
            .strip_prefix(&raiz)
            .unwrap_or(Path::new(ruta))
            .to_string_lossy()
            .replace('\\', "/");
        if de_serie_re().is_match(&relativa)
            || de_serie.contains(&format!("drive_c/{relativa}").to_lowercase())
        {
            continue;
        }
        let Ok(meta) = std::fs::metadata(ruta) else {
            continue;
        };
        salida.push(ExeNuevo {
            exe: ruta.clone(),
            nombre: pe::nombre(Path::new(ruta)),
            tam: meta.len(),
        });
    }
    salida.sort_by_key(|x| {
        (
            !x.exe.to_lowercase().contains("program files"),
            std::cmp::Reverse(x.tam),
        )
    });
    salida
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn slug_sin_acentos() {
        assert_eq!(slug("Año Ñandú: ¡Juego!"), "ano-nandu-juego");
        assert_eq!(slug("???"), "programa");
        assert!(slug(&"a".repeat(80)).len() <= 32);
    }

    #[test]
    fn ids_distintos() {
        assert_ne!(nuevo_id("x", "y"), nuevo_id("x", "y"));
    }
}
