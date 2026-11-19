//! El vault: una carpeta de notas en Markdown, como Obsidian (igual que `vault.py`).
//!
//! Entiende lo mismo que Obsidian: carpetas, `[[enlaces]]` (con `#sección`, `^bloque`, `|alias` y
//! `![[incrustados]]`), `#etiquetas` (también anidadas, `#juegos/roblox`), frontmatter YAML con `tags` y
//! `aliases`, retroenlaces y el grafo. Se puede abrir cualquier carpeta, también un vault de Obsidian que ya
//! uses, sin romperlo: ignora las carpetas que empiezan con punto (`.obsidian/`, `.trash/`, `.git/`), no
//! escribe nada fuera de las notas y lo que se borra va a `.trash/` del vault, como hace Obsidian.
//!
//! Todas las rutas son relativas al vault, con «/» y con «.md» (por ejemplo «Ideas/Juegos.md»).

use crate::texto::{normal, normal_con_mapa};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

pub const PAPELERA: &str = ".trash";

/// Un fallo del vault (con un mensaje que se le puede enseñar a la persona).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorVault {
    /// Una ruta que no se puede usar (por ejemplo, una que se sale del vault).
    Ruta(String),
    NoExiste(String),
    Existe(String),
    Permiso(String),
    Otro(String),
}

impl std::fmt::Display for ErrorVault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorVault::Ruta(s)
            | ErrorVault::NoExiste(s)
            | ErrorVault::Existe(s)
            | ErrorVault::Permiso(s)
            | ErrorVault::Otro(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for ErrorVault {}

impl From<std::io::Error> for ErrorVault {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => ErrorVault::NoExiste(e.to_string()),
            std::io::ErrorKind::PermissionDenied => ErrorVault::Permiso(e.to_string()),
            std::io::ErrorKind::AlreadyExists => ErrorVault::Existe(e.to_string()),
            _ => ErrorVault::Otro(e.to_string()),
        }
    }
}

type R<T> = Result<T, ErrorVault>;

// ------------------------------------------------------------------ lectura de Markdown

static COMENTARIO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)%%.*?%%").unwrap());
static CODIGO_LINEA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\n]+`").unwrap());
static ENLACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(!?)\[\[([^\[\]\n]+?)\]\]").unwrap());
static ETIQUETA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#(\w[\w/-]*)").unwrap());
static PROHIBIDOS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[\\/:*?"<>|#^\[\]]"#).unwrap());

/// Cambia todo menos los saltos de línea por espacios (de la misma longitud en bytes).
fn en_blanco(trozo: &str) -> String {
    trozo
        .chars()
        .flat_map(|c| {
            if c == '\n' {
                "\n".to_string()
            } else {
                " ".repeat(c.len_utf8())
            }
            .chars()
            .collect::<Vec<_>>()
        })
        .collect()
}

/// Los bloques de código con ``` o ~~~ (hasta el cierre con la misma marca al inicio de una línea, o el
/// final del texto), cambiados por espacios.
fn tapar_bloques(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());
    let mut i = 0;
    let b = texto.as_bytes();
    while i < texto.len() {
        let inicio_de_linea = i == 0 || b[i - 1] == b'\n';
        let marca = if inicio_de_linea {
            ["```", "~~~"].into_iter().find(|m| texto[i..].starts_with(m))
        } else {
            None
        };
        let Some(marca) = marca else {
            // al siguiente inicio de línea
            let fin = texto[i..].find('\n').map_or(texto.len(), |p| i + p + 1);
            salida += &texto[i..fin];
            i = fin;
            continue;
        };
        // El cierre: la primera línea (después de la de apertura) que empieza con la misma marca.
        let mut j = i + marca.len();
        let fin = loop {
            match texto[j..].find('\n') {
                None => break texto.len(),
                Some(p) => {
                    let linea = j + p + 1;
                    if texto[linea..].starts_with(marca) {
                        break texto[linea..].find('\n').map_or(texto.len(), |q| linea + q);
                    }
                    j = linea;
                }
            }
        };
        salida += &en_blanco(&texto[i..fin]);
        i = fin;
    }
    salida
}

/// El texto con el código y los comentarios `%%…%%` cambiados por espacios (mismo largo y mismas líneas).
///
/// Así lo que va en código no cuenta como enlace ni etiqueta, y las posiciones siguen sirviendo.
pub fn tapar(texto: &str) -> String {
    let mut t = tapar_bloques(texto);
    for patron in [&*COMENTARIO, &*CODIGO_LINEA] {
        t = patron
            .replace_all(&t, |c: &regex::Captures| en_blanco(&c[0]))
            .to_string();
    }
    t
}

/// Un valor del frontmatter.
#[derive(Debug, Clone, PartialEq)]
pub enum Yaml {
    Nada,
    Bool(bool),
    Texto(String),
    Lista(Vec<String>),
}

pub type Frontmatter = BTreeMap<String, Yaml>;

fn quitar_comillas(valor: &str) -> String {
    let v = valor.trim();
    let c: Vec<char> = v.chars().collect();
    if c.len() >= 2 && c[0] == c[c.len() - 1] && (c[0] == '"' || c[0] == '\'') {
        return c[1..c.len() - 1].iter().collect();
    }
    v.to_string()
}

/// Parte «a, "b, c", d» por comas que no vayan entre comillas.
fn partir_lista(texto: &str) -> Vec<String> {
    let (mut partes, mut actual, mut comilla) = (Vec::new(), String::new(), None::<char>);
    for c in texto.chars() {
        if let Some(q) = comilla {
            actual.push(c);
            if c == q {
                comilla = None;
            }
        } else if c == '"' || c == '\'' {
            comilla = Some(c);
            actual.push(c);
        } else if c == ',' {
            partes.push(std::mem::take(&mut actual));
        } else {
            actual.push(c);
        }
    }
    partes.push(actual);
    partes
}

fn valor_yaml(valor: &str) -> Yaml {
    let v = valor.trim();
    if v.starts_with('[') && v.ends_with(']') {
        return Yaml::Lista(
            partir_lista(&v[1..v.len() - 1])
                .into_iter()
                .filter(|p| !p.trim().is_empty())
                .map(|p| quitar_comillas(&p))
                .collect(),
        );
    }
    if matches!(v, "" | "~" | "null") {
        return Yaml::Nada;
    }
    match v.to_lowercase().as_str() {
        "true" => Yaml::Bool(true),
        "false" => Yaml::Bool(false),
        _ => Yaml::Texto(quitar_comillas(v)),
    }
}

/// `(frontmatter, cuerpo)`. Entiende el YAML sencillo que usa Obsidian: «clave: valor», listas `[a, b]` y
/// listas con «- elemento». Si no hay frontmatter, `({}, texto)`.
pub fn separar_frontmatter(texto: &str) -> (Frontmatter, String) {
    let texto = texto.strip_prefix('\u{feff}').unwrap_or(texto);
    if !texto.starts_with("---") {
        return (Frontmatter::new(), texto.to_string());
    }
    let lineas: Vec<&str> = texto.split('\n').collect();
    if lineas[0].trim() != "---" {
        return (Frontmatter::new(), texto.to_string());
    }
    let Some(fin) = (1..lineas.len()).find(|i| matches!(lineas[*i].trim(), "---" | "...")) else {
        return (Frontmatter::new(), texto.to_string());
    };
    let mut datos = Frontmatter::new();
    let mut clave: Option<String> = None;
    for linea in &lineas[1..fin] {
        let linea = linea.trim_end_matches('\r');
        if linea.trim().is_empty() || linea.trim_start().starts_with('#') {
            continue;
        }
        let suelta = linea.trim();
        if suelta.starts_with("- ") || suelta == "-" {
            if let Some(k) = &clave {
                let actual = datos.get(k).cloned().unwrap_or(Yaml::Nada);
                let mut lista = match actual {
                    Yaml::Lista(l) => l,
                    Yaml::Nada => vec![],
                    Yaml::Texto(t) if t.is_empty() => vec![],
                    Yaml::Texto(t) => vec![t],
                    Yaml::Bool(b) => vec![if b { "True" } else { "False" }.to_string()],
                };
                let elemento = quitar_comillas(suelta[1..].trim());
                if !elemento.is_empty() {
                    lista.push(elemento);
                }
                datos.insert(k.clone(), Yaml::Lista(lista));
            }
            continue;
        }
        if linea.contains(':') && !linea.starts_with(char::is_whitespace) {
            let (k, v) = linea.split_once(':').unwrap();
            let k = k.trim().to_string();
            datos.insert(k.clone(), valor_yaml(v));
            clave = Some(k);
        }
    }
    (datos, lineas[fin + 1..].join("\n"))
}

/// Una lista de textos a partir de lo que venga en el frontmatter (lista, «a, b» o «a b»).
fn lista_de(valor: Option<&Yaml>) -> Vec<String> {
    match valor {
        None | Some(Yaml::Nada) | Some(Yaml::Bool(false)) => vec![],
        Some(Yaml::Lista(l)) => l
            .iter()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
            .collect(),
        Some(Yaml::Texto(t)) => t
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect(),
        Some(Yaml::Bool(true)) => vec!["True".into()],
    }
}

/// Las etiquetas de una nota (del frontmatter `tags` y de los `#etiqueta` del texto), sin «#» y en
/// minúsculas, sin repetir. Las que son solo números (`#2026`) no cuentan, como en Obsidian.
pub fn etiquetas_de(texto: &str) -> Vec<String> {
    let (frente, cuerpo) = separar_frontmatter(texto);
    let mut vistas: Vec<String> = Vec::new();
    let mut meter = |t: String| {
        if !t.is_empty() && !vistas.contains(&t) {
            vistas.push(t);
        }
    };
    for clave in ["tags", "tag", "etiquetas"] {
        for t in lista_de(frente.get(clave)) {
            meter(t.trim_start_matches('#').trim().to_lowercase());
        }
    }
    let tapado = tapar(&cuerpo);
    for m in ETIQUETA.captures_iter(&tapado) {
        let inicio = m.get(0).unwrap().start();
        let antes = tapado[..inicio].chars().next_back();
        if antes.is_some_and(|c| c.is_alphanumeric() || "_#&/\\".contains(c)) {
            continue;
        }
        let t = m[1].trim_end_matches(['/', '-']).to_lowercase();
        let sin_barras = t.replace('/', "");
        if !t.is_empty() && !sin_barras.chars().all(char::is_numeric) {
            meter(t);
        }
    }
    vistas
}

pub fn alias_de(frente: &Frontmatter) -> Vec<String> {
    let mut salida = Vec::new();
    for clave in ["aliases", "alias"] {
        match frente.get(clave) {
            Some(Yaml::Lista(_)) => salida.extend(lista_de(frente.get(clave))),
            otro => {
                let t = match otro {
                    Some(Yaml::Texto(t)) => t.clone(),
                    Some(Yaml::Bool(true)) => "True".into(),
                    _ => String::new(),
                };
                salida.extend(
                    t.split(',')
                        .map(|a| a.trim().to_string())
                        .filter(|a| !a.is_empty()),
                );
            }
        }
    }
    salida
}

/// «Nota#Sección|texto» → `("Nota", "Sección", "texto")`.
pub fn partir_enlace(interior: &str) -> (String, String, String) {
    let (destino, mostrado) = interior.split_once('|').unwrap_or((interior, ""));
    let (mut nombre, mut seccion) = (destino.to_string(), String::new());
    for separador in ['#', '^'] {
        if let Some((n, resto)) = nombre.clone().split_once(separador) {
            nombre = n.to_string();
            seccion = if separador == '^' {
                format!("^{resto}")
            } else {
                resto.to_string()
            };
            break;
        }
    }
    (
        nombre.trim().to_string(),
        seccion.trim().to_string(),
        mostrado.trim().to_string(),
    )
}

/// Un `[[enlace]]` dentro de un texto. `inicio` y `fin` son las posiciones (en bytes) del `[[…]]` completo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnlaceTexto {
    pub nombre: String,
    pub seccion: String,
    pub texto: String,
    pub incrustado: bool,
    pub inicio: usize,
    pub fin: usize,
}

/// Los `[[enlaces]]` de un texto, en orden (sin los que van dentro de código o comentarios).
pub fn enlaces_de(texto: &str) -> Vec<EnlaceTexto> {
    let tapado = tapar(texto);
    ENLACE
        .captures_iter(&tapado)
        .map(|c| {
            let todo = c.get(0).unwrap();
            let interior = c.get(2).unwrap();
            let (nombre, seccion, mostrado) = partir_enlace(&texto[interior.start()..interior.end()]);
            EnlaceTexto {
                nombre,
                seccion,
                texto: mostrado,
                incrustado: !c[1].is_empty(),
                inicio: todo.start(),
                fin: todo.end(),
            }
        })
        .collect()
}

fn buscar_sub(pajar: &[char], aguja: &[char], desde: usize) -> Option<usize> {
    if aguja.is_empty() || pajar.len() < aguja.len() {
        return None;
    }
    (desde..=pajar.len() - aguja.len()).find(|&i| pajar[i..i + aguja.len()] == *aguja)
}

fn contar_sub(pajar: &[char], aguja: &[char]) -> usize {
    let (mut n, mut i) = (0, 0);
    while let Some(p) = buscar_sub(pajar, aguja, i) {
        n += 1;
        i = p + aguja.len();
    }
    n
}

fn recorte(texto: &[char], inicio: usize, fin: usize) -> String {
    let (antes, despues) = (60usize, 110usize);
    let mut a = inicio.saturating_sub(antes);
    let mut b = (fin + despues).min(texto.len());
    if a > 0 {
        // texto.find(" ", a, inicio)
        if let Some(e) = (a..inicio.min(texto.len())).find(|&i| texto[i] == ' ') {
            a = e + 1;
        }
    }
    if b < texto.len() {
        // texto.rfind(" ", fin, b)
        if let Some(e) = (fin..b).rev().find(|&i| texto[i] == ' ')
            && e > 0
        {
            b = e;
        }
    }
    let trozo = texto[a.min(b)..b].iter().collect::<String>();
    let trozo = trozo.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{}{trozo}{}",
        if a > 0 { "…" } else { "" },
        if b < texto.len() { "…" } else { "" }
    )
}

/// Un nombre de archivo que sirve en cualquier sistema y en Obsidian (sin `\ / : * ? " < > | # ^ [ ]`).
pub fn limpiar_nombre(titulo: &str) -> String {
    let sin = PROHIBIDOS.replace_all(titulo, " ");
    let junto = sin.split_whitespace().collect::<Vec<_>>().join(" ");
    let n: String = junto
        .trim()
        .trim_start_matches('.')
        .trim()
        .chars()
        .take(180)
        .collect();
    if n.is_empty() { "Sin título".into() } else { n }
}

// ------------------------------------------------------------------ el vault

/// Lo que dice cada nota (se guarda mientras el archivo no cambie).
#[derive(Debug)]
struct Info {
    ruta: String,
    titulo: String,
    modificado: f64,
    etiquetas: Vec<String>,
    alias: Vec<String>,
    carpeta: String,
    bytes: u64,
    enlaces: Vec<(String, String, String, bool)>, // nombre, sección, texto, incrustado
}

/// Una nota: `{"ruta", "titulo", "modificado", "etiquetas", "alias", "carpeta", "bytes"}`.
#[derive(Debug, Clone, PartialEq)]
pub struct Nota {
    pub ruta: String,
    pub titulo: String,
    pub modificado: f64,
    pub etiquetas: Vec<String>,
    pub alias: Vec<String>,
    pub carpeta: String,
    pub bytes: u64,
}

/// Un nodo del árbol de carpetas y notas.
#[derive(Debug, Clone, PartialEq)]
pub struct Nodo {
    pub nombre: String,
    pub ruta: String,
    pub es_carpeta: bool,
    pub titulo: String,
    pub modificado: f64,
    pub hijos: Vec<Nodo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Resultado {
    pub ruta: String,
    pub titulo: String,
    pub fragmento: String,
    pub coincidencias: usize,
}

/// Un enlace que sale de una nota.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enlace {
    pub nombre: String,
    pub ruta: Option<String>,
    pub existe: bool,
    pub seccion: String,
    pub texto: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retroenlace {
    pub ruta: String,
    pub titulo: String,
    pub fragmento: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodoGrafo {
    pub id: String,
    pub titulo: String,
    pub etiquetas: Vec<String>,
    pub grado: usize,
    pub existe: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grafo {
    pub nodos: Vec<NodoGrafo>,
    pub aristas: Vec<(String, String)>,
}

struct Mapa {
    por_nombre: HashMap<String, Vec<String>>,
    por_ruta: HashMap<String, String>,
    por_alias: HashMap<String, Vec<String>>,
}

/// Lo que se guarda de cada nota: fecha (ns), tamaño y lo que dice.
type EnCache = (i64, u64, Arc<Info>);

/// Una carpeta de notas `.md`. Se crea si no existe.
pub struct Vault {
    pub raiz: PathBuf,
    pub nombre: String,
    cache: Mutex<HashMap<String, EnCache>>,
}

fn mtime_ns(m: &std::fs::Metadata) -> i64 {
    m.mtime() * 1_000_000_000 + m.mtime_nsec()
}

fn mtime_f(m: &std::fs::Metadata) -> f64 {
    m.mtime() as f64 + m.mtime_nsec() as f64 / 1e9
}

fn stem(ruta: &str) -> String {
    Path::new(ruta)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn padre(ruta: &str) -> String {
    match ruta.rsplit_once('/') {
        Some((p, _)) => p.to_string(),
        None => String::new(),
    }
}

impl Vault {
    pub fn new(ruta: impl AsRef<Path>) -> R<Vault> {
        let texto = ruta.as_ref().to_string_lossy().to_string();
        let ruta = match texto.strip_prefix("~/") {
            Some(r) => vortex_core::rutas::home().join(r),
            None => PathBuf::from(&texto),
        };
        std::fs::create_dir_all(&ruta)?;
        let raiz = ruta.canonicalize()?;
        let nombre = raiz.file_name().unwrap_or_default().to_string_lossy().to_string();
        Ok(Vault {
            raiz,
            nombre,
            cache: Mutex::new(HashMap::new()),
        })
    }

    // ---------------------------------------------------------------- rutas

    /// Normaliza una ruta relativa y revisa que no se salga del vault.
    pub fn rel(&self, ruta: &str) -> R<String> {
        let mut texto = ruta.replace('\\', "/").trim().to_string();
        if texto.starts_with('/') {
            let p = Path::new(&texto);
            let resuelta = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
            texto = resuelta
                .strip_prefix(&self.raiz)
                .map_err(|_| ErrorVault::Ruta(format!("«{ruta}» no está dentro del vault.")))?
                .to_string_lossy()
                .to_string();
        }
        let partes: Vec<&str> = texto.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
        if partes.is_empty() || partes.contains(&"..") {
            return Err(ErrorVault::Ruta(format!(
                "«{ruta}» no es una ruta válida dentro del vault."
            )));
        }
        Ok(partes.join("/"))
    }

    pub fn ruta_absoluta(&self, ruta: &str) -> R<PathBuf> {
        Ok(self.raiz.join(self.rel(ruta)?))
    }

    pub fn existe(&self, ruta: &str) -> bool {
        self.ruta_absoluta(ruta).is_ok_and(|p| p.is_file())
    }

    /// Las rutas relativas de todas las notas, sin entrar a carpetas ocultas (primero las de la carpeta, luego
    /// cada subcarpeta en orden).
    fn recorrer(&self) -> Vec<String> {
        fn ir(dir: &Path, base: &str, salida: &mut Vec<String>) {
            let Ok(entradas) = std::fs::read_dir(dir) else {
                return;
            };
            let (mut archivos, mut subs) = (Vec::new(), Vec::new());
            for e in entradas.flatten() {
                let nombre = e.file_name().to_string_lossy().to_string();
                let Ok(ft) = e.file_type() else { continue };
                let es_dir = if ft.is_symlink() {
                    e.path().is_dir()
                } else {
                    ft.is_dir()
                };
                if es_dir {
                    if !ft.is_symlink() && !nombre.starts_with('.') {
                        subs.push(nombre);
                    }
                } else {
                    archivos.push(nombre);
                }
            }
            archivos.sort();
            subs.sort();
            for a in archivos {
                if a.to_lowercase().ends_with(".md") && !a.starts_with('.') {
                    salida.push(if base.is_empty() { a } else { format!("{base}/{a}") });
                }
            }
            for s in subs {
                let hijo = if base.is_empty() {
                    s.clone()
                } else {
                    format!("{base}/{s}")
                };
                ir(&dir.join(&s), &hijo, salida);
            }
        }
        let mut salida = Vec::new();
        ir(&self.raiz, "", &mut salida);
        salida
    }

    // ---------------------------------------------------------------- lo que dice cada nota (con caché)

    fn info(&self, rel: &str) -> Option<Arc<Info>> {
        let ruta = self.raiz.join(rel);
        let Ok(st) = std::fs::metadata(&ruta) else {
            self.cache.lock().unwrap().remove(rel);
            return None;
        };
        if let Some((m, s, i)) = self.cache.lock().unwrap().get(rel)
            && *m == mtime_ns(&st)
            && *s == st.len()
        {
            return Some(i.clone());
        }
        let texto = String::from_utf8_lossy(&std::fs::read(&ruta).ok()?).to_string();
        let (frente, _) = separar_frontmatter(&texto);
        let info = Arc::new(Info {
            ruta: rel.to_string(),
            titulo: stem(rel),
            modificado: mtime_f(&st),
            etiquetas: etiquetas_de(&texto),
            alias: alias_de(&frente),
            carpeta: padre(rel),
            bytes: st.len(),
            enlaces: enlaces_de(&texto)
                .into_iter()
                .map(|e| (e.nombre, e.seccion, e.texto, e.incrustado))
                .collect(),
        });
        self.cache
            .lock()
            .unwrap()
            .insert(rel.to_string(), (mtime_ns(&st), st.len(), info.clone()));
        Some(info)
    }

    fn todas(&self) -> Vec<Arc<Info>> {
        let salida: Vec<Arc<Info>> = self.recorrer().iter().filter_map(|r| self.info(r)).collect();
        let vivas: HashSet<&str> = salida.iter().map(|i| i.ruta.as_str()).collect();
        self.cache
            .lock()
            .unwrap()
            .retain(|k, _| vivas.contains(k.as_str()));
        salida
    }

    /// Todas las notas.
    pub fn notas(&self) -> Vec<Nota> {
        self.todas()
            .iter()
            .map(|i| Nota {
                ruta: i.ruta.clone(),
                titulo: i.titulo.clone(),
                modificado: i.modificado,
                etiquetas: i.etiquetas.clone(),
                alias: i.alias.clone(),
                carpeta: i.carpeta.clone(),
                bytes: i.bytes,
            })
            .collect()
    }

    pub fn frontmatter(&self, ruta: &str) -> R<Frontmatter> {
        Ok(separar_frontmatter(&self.leer(ruta)?).0)
    }

    // ---------------------------------------------------------------- resolver enlaces

    fn mapa(&self, todas: &[Arc<Info>]) -> Mapa {
        let mut m = Mapa {
            por_nombre: HashMap::new(),
            por_ruta: HashMap::new(),
            por_alias: HashMap::new(),
        };
        for i in todas {
            m.por_nombre
                .entry(i.titulo.to_lowercase())
                .or_default()
                .push(i.ruta.clone());
            m.por_ruta
                .insert(i.ruta[..i.ruta.len() - 3].to_lowercase(), i.ruta.clone());
            for a in &i.alias {
                m.por_alias
                    .entry(a.to_lowercase())
                    .or_default()
                    .push(i.ruta.clone());
            }
        }
        m
    }

    fn elegir(candidatas: &[String], desde: Option<&str>) -> String {
        if candidatas.len() == 1 {
            return candidatas[0].clone();
        }
        let carpeta = desde.map(padre).unwrap_or_default();
        let misma: Vec<&String> = candidatas.iter().filter(|c| padre(c) == carpeta).collect();
        let mut base: Vec<&String> = if misma.is_empty() {
            candidatas.iter().collect()
        } else {
            misma
        };
        base.sort_by_key(|c| (c.matches('/').count(), c.chars().count(), (*c).clone()));
        base[0].clone()
    }

    fn resolver_en(&self, mapa: &Mapa, nombre_enlace: &str, desde: Option<&str>) -> Option<String> {
        let limpio = nombre_enlace.trim().trim_start_matches('!');
        let limpio = limpio.strip_prefix("[[").unwrap_or(limpio);
        let limpio = limpio.strip_suffix("]]").unwrap_or(limpio);
        let (nombre, _, _) = partir_enlace(limpio);
        if nombre.is_empty() {
            return desde.map(str::to_string); // [[#Sección]]: un enlace a la misma nota
        }
        let mut clave = nombre.replace('\\', "/").trim_matches('/').to_lowercase();
        if let Some(c) = clave.strip_suffix(".md") {
            clave = c.to_string();
        }
        if clave.contains('/') {
            if let Some(r) = mapa.por_ruta.get(&clave) {
                return Some(r.clone());
            }
            let sufijo: Vec<String> = mapa
                .por_ruta
                .iter()
                .filter(|(k, _)| k.ends_with(&format!("/{clave}")))
                .map(|(_, r)| r.clone())
                .collect();
            if !sufijo.is_empty() {
                return Some(Self::elegir(&sufijo, desde));
            }
            return None;
        }
        if let Some(c) = mapa.por_nombre.get(&clave) {
            return Some(Self::elegir(c, desde));
        }
        if let Some(c) = mapa.por_alias.get(&clave) {
            return Some(Self::elegir(c, desde));
        }
        None
    }

    /// La nota a la que lleva un enlace («Nota», «Nota#sección», «Carpeta/Nota», un alias…), o `None`.
    pub fn resolver(&self, nombre_enlace: &str, desde: Option<&str>) -> Option<String> {
        let todas = self.todas();
        self.resolver_en(&self.mapa(&todas), nombre_enlace, desde)
    }

    // ---------------------------------------------------------------- archivos

    /// Las carpetas y notas, para la vista de archivos (carpetas primero, luego notas).
    pub fn arbol(&self) -> Nodo {
        fn carpeta(ruta_abs: &Path, rel: &str, nombre: String) -> Nodo {
            let mut nodo = Nodo {
                nombre,
                ruta: rel.to_string(),
                es_carpeta: true,
                titulo: String::new(),
                modificado: 0.0,
                hijos: vec![],
            };
            let Ok(entradas) = std::fs::read_dir(ruta_abs) else {
                return nodo;
            };
            let mut hijos: Vec<(String, PathBuf)> = entradas
                .flatten()
                .map(|e| (e.file_name().to_string_lossy().to_string(), e.path()))
                .collect();
            hijos.sort_by_key(|(n, _)| n.to_lowercase());
            for (n, p) in &hijos {
                if p.is_dir() && !n.starts_with('.') {
                    let r = if rel.is_empty() {
                        n.clone()
                    } else {
                        format!("{rel}/{n}")
                    };
                    nodo.hijos.push(carpeta(p, &r, n.clone()));
                }
            }
            for (n, p) in &hijos {
                if p.is_file() && n.to_lowercase().ends_with(".md") && !n.starts_with('.') {
                    let r = if rel.is_empty() {
                        n.clone()
                    } else {
                        format!("{rel}/{n}")
                    };
                    nodo.hijos.push(Nodo {
                        nombre: n.clone(),
                        ruta: r,
                        es_carpeta: false,
                        titulo: stem(n),
                        modificado: p.metadata().map(|m| mtime_f(&m)).unwrap_or(0.0),
                        hijos: vec![],
                    });
                }
            }
            nodo
        }
        carpeta(&self.raiz, "", self.nombre.clone())
    }

    pub fn leer(&self, ruta: &str) -> R<String> {
        let bytes = std::fs::read(self.ruta_absoluta(ruta)?)?;
        Ok(String::from_utf8_lossy(&bytes).to_string())
    }

    /// Guarda la nota sin riesgo de dejarla a medias (temporal + `rename`).
    pub fn guardar(&self, ruta: &str, texto: &str) -> R<()> {
        let destino = self.ruta_absoluta(ruta)?;
        let dir = destino.parent().unwrap_or(&self.raiz).to_path_buf();
        std::fs::create_dir_all(&dir)?;
        let mut t = tempfile::Builder::new()
            .prefix(".")
            .suffix(".tmp")
            .tempfile_in(&dir)?;
        t.write_all(texto.as_bytes())?;
        t.flush()?;
        t.as_file().sync_all()?;
        let modo = destino.metadata().map(|m| m.mode() & 0o777).unwrap_or(0o644);
        let _ = std::fs::set_permissions(t.path(), std::fs::Permissions::from_mode(modo));
        t.persist(&destino).map_err(|e| ErrorVault::from(e.error))?;
        self.cache.lock().unwrap().remove(&self.rel(ruta)?);
        Ok(())
    }

    /// «carpeta/nombre.md», o «nombre 1.md», «nombre 2.md»… si ya existe.
    fn libre(&self, carpeta: &str, nombre: &str) -> String {
        let prefijo = if carpeta.is_empty() {
            String::new()
        } else {
            format!("{carpeta}/")
        };
        let (mut candidata, mut n) = (format!("{prefijo}{nombre}.md"), 1);
        while self.raiz.join(&candidata).exists() {
            candidata = format!("{prefijo}{nombre} {n}.md");
            n += 1;
        }
        candidata
    }

    /// Crea una nota vacía y regresa su ruta.
    pub fn crear(&self, titulo: &str, carpeta: &str) -> R<String> {
        let carpeta = if carpeta.trim_matches(['/', ' ', '.']).is_empty() {
            String::new()
        } else {
            self.rel(carpeta)?
        };
        let rel = self.libre(&carpeta, &limpiar_nombre(titulo));
        self.guardar(&rel, "")?;
        Ok(rel)
    }

    pub fn crear_carpeta(&self, ruta: &str) -> R<String> {
        let rel = self.rel(ruta)?;
        std::fs::create_dir_all(self.raiz.join(&rel))?;
        Ok(rel)
    }

    /// Le cambia el nombre (o la mueve, si `nuevo` es una ruta) y arregla los `[[enlaces]]` que le llegaban.
    pub fn renombrar(&self, ruta: &str, nuevo: &str) -> R<String> {
        let vieja = self.rel(ruta)?;
        if !self.raiz.join(&vieja).is_file() {
            return Err(ErrorVault::NoExiste(vieja));
        }
        let nuevo = nuevo.trim();
        let nueva = if nuevo.contains('/') || nuevo.contains('\\') || nuevo.to_lowercase().ends_with(".md") {
            let mut n = self.rel(nuevo)?;
            if !n.to_lowercase().ends_with(".md") {
                n += ".md";
            }
            let mut partes: Vec<String> = n.split('/').map(str::to_string).collect();
            let ultima = partes.pop().unwrap_or_default();
            let sin: String = ultima
                .chars()
                .take(ultima.chars().count().saturating_sub(3))
                .collect();
            partes.push(format!("{}.md", limpiar_nombre(&sin)));
            partes.join("/")
        } else {
            let carpeta = padre(&vieja);
            format!(
                "{}{}.md",
                if carpeta.is_empty() {
                    String::new()
                } else {
                    format!("{carpeta}/")
                },
                limpiar_nombre(nuevo)
            )
        };
        if nueva == vieja {
            return Ok(vieja);
        }
        let destino = self.raiz.join(&nueva);
        if destino.exists() {
            let misma = vieja.to_lowercase() == nueva.to_lowercase()
                && match (destino.metadata(), self.raiz.join(&vieja).metadata()) {
                    (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
                    _ => false,
                };
            if !misma {
                return Err(ErrorVault::Existe(format!(
                    "Ya hay una nota llamada «{}».",
                    stem(&nueva)
                )));
            }
        }

        // Antes de mover: qué enlaces de qué notas llevan a la vieja (y no por un alias, que sigue sirviendo).
        let todas = self.todas();
        let mapa = self.mapa(&todas);
        let alias_viejos: HashSet<String> = todas
            .iter()
            .filter(|i| i.ruta == vieja)
            .flat_map(|i| i.alias.iter().map(|a| a.to_lowercase()))
            .collect();
        let mut por_cambiar: Vec<(String, Vec<EnlaceTexto>)> = Vec::new();
        for info in &todas {
            if !info.enlaces.iter().any(|e| !e.0.is_empty()) {
                continue;
            }
            let texto = self.leer(&info.ruta)?;
            let mut mios = Vec::new();
            for e in enlaces_de(&texto) {
                if e.nombre.is_empty() || alias_viejos.contains(&e.nombre.to_lowercase()) {
                    continue;
                }
                if self.resolver_en(&mapa, &e.nombre, Some(&info.ruta)).as_deref() == Some(vieja.as_str()) {
                    mios.push(e);
                }
            }
            if !mios.is_empty() {
                por_cambiar.push((info.ruta.clone(), mios));
            }
        }

        if let Some(p) = destino.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::rename(self.raiz.join(&vieja), &destino)?;
        self.cache.lock().unwrap().remove(&vieja);

        let todas_nuevas = self.todas();
        let mapa_nuevo = self.mapa(&todas_nuevas);
        let titulo = stem(&nueva);
        let unica = mapa_nuevo
            .por_nombre
            .get(&titulo.to_lowercase())
            .is_some_and(|v| v.len() == 1);
        for (rel, mut enlaces) in por_cambiar {
            let rel = if rel == vieja { nueva.clone() } else { rel };
            let mut texto = self.leer(&rel)?;
            enlaces.sort_by_key(|e| std::cmp::Reverse(e.inicio));
            for e in enlaces {
                let destino_enlace = if unica && !e.nombre.contains('/') {
                    titulo.clone()
                } else {
                    nueva[..nueva.len() - 3].to_string()
                };
                let mut interior = destino_enlace;
                if !e.seccion.is_empty() {
                    if e.seccion.starts_with('^') {
                        interior += &e.seccion;
                    } else {
                        interior += &format!("#{}", e.seccion);
                    }
                }
                if !e.texto.is_empty() {
                    interior += &format!("|{}", e.texto);
                }
                texto = format!(
                    "{}{}[[{interior}]]{}",
                    &texto[..e.inicio],
                    if e.incrustado { "!" } else { "" },
                    &texto[e.fin..]
                );
            }
            self.guardar(&rel, &texto)?;
        }
        Ok(nueva)
    }

    /// La manda a `.trash/` del vault (como Obsidian; de ahí se puede recuperar). Regresa su ruta ahí.
    pub fn borrar(&self, ruta: &str) -> R<String> {
        let rel = self.rel(ruta)?;
        let origen = self.raiz.join(&rel);
        if !origen.exists() {
            return Err(ErrorVault::NoExiste(rel));
        }
        let papelera = self.raiz.join(PAPELERA);
        std::fs::create_dir_all(&papelera)?;
        let base = Path::new(&rel);
        let nombre = base.file_name().unwrap_or_default().to_string_lossy().to_string();
        let mut destino = papelera.join(&nombre);
        let mut n = 1;
        while destino.exists() {
            let s = base.file_stem().unwrap_or_default().to_string_lossy();
            let ext = base
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            destino = papelera.join(format!("{s} {n}{ext}"));
            n += 1;
        }
        std::fs::rename(&origen, &destino)?;
        self.cache.lock().unwrap().remove(&rel);
        Ok(format!(
            "{PAPELERA}/{}",
            destino.file_name().unwrap_or_default().to_string_lossy()
        ))
    }

    // ---------------------------------------------------------------- buscar

    /// Las notas que tienen todas las palabras (en el título o el texto), sin importar mayúsculas ni
    /// acentos. «#etiqueta» busca por etiqueta.
    pub fn buscar(&self, texto: &str) -> R<Vec<Resultado>> {
        let consulta = texto.trim();
        if consulta.is_empty() {
            return Ok(vec![]);
        }
        struct Crudo {
            r: Resultado,
            t: i64,
            m: f64,
        }
        let mut resultados: Vec<Crudo> = Vec::new();
        if consulta.starts_with('#') && !consulta.contains(' ') {
            let etiqueta = consulta[1..].to_lowercase();
            for info in self.todas() {
                if info
                    .etiquetas
                    .iter()
                    .any(|t| *t == etiqueta || t.starts_with(&format!("{etiqueta}/")))
                {
                    let contenido: Vec<char> = self.leer(&info.ruta)?.chars().collect();
                    let bajo: Vec<char> = contenido.iter().flat_map(|c| c.to_lowercase()).collect();
                    let q: Vec<char> = consulta.to_lowercase().chars().collect();
                    let fragmento = match buscar_sub(&bajo, &q, 0) {
                        Some(i) => recorte(&contenido, i, i + consulta.chars().count()),
                        None => recorte(&contenido, 0, 0),
                    };
                    resultados.push(Crudo {
                        r: Resultado {
                            ruta: info.ruta.clone(),
                            titulo: info.titulo.clone(),
                            fragmento,
                            coincidencias: 1,
                        },
                        t: 0,
                        m: info.modificado,
                    });
                }
            }
        } else {
            let palabras: Vec<Vec<char>> = consulta
                .split_whitespace()
                .map(normal)
                .filter(|p| !p.is_empty())
                .map(|p| p.chars().collect())
                .collect();
            for info in self.todas() {
                let titulo_n: Vec<char> = normal(&info.titulo).chars().collect();
                let contenido_txt = self.leer(&info.ruta)?;
                let contenido: Vec<char> = contenido_txt.chars().collect();
                let (normalizado, mapa) = normal_con_mapa(&contenido_txt);
                let cuentas: Vec<usize> = palabras
                    .iter()
                    .map(|p| contar_sub(&normalizado, p) + contar_sub(&titulo_n, p))
                    .collect();
                if cuentas.contains(&0) {
                    continue;
                }
                let en_titulo = palabras
                    .iter()
                    .filter(|p| buscar_sub(&titulo_n, p, 0).is_some())
                    .count() as i64;
                let mut fragmento = String::new();
                for p in &palabras {
                    if let Some(i) = buscar_sub(&normalizado, p, 0) {
                        fragmento = recorte(&contenido, mapa[i], mapa[i + p.len() - 1] + 1);
                        break;
                    }
                }
                if fragmento.is_empty() {
                    let cuerpo: Vec<char> = separar_frontmatter(&contenido_txt).1.chars().collect();
                    fragmento = recorte(&cuerpo, 0, 0);
                }
                resultados.push(Crudo {
                    r: Resultado {
                        ruta: info.ruta.clone(),
                        titulo: info.titulo.clone(),
                        fragmento,
                        coincidencias: cuentas.iter().sum(),
                    },
                    t: -en_titulo,
                    m: info.modificado,
                });
            }
        }
        resultados.sort_by(|a, b| {
            a.t.cmp(&b.t)
                .then(b.r.coincidencias.cmp(&a.r.coincidencias))
                .then(b.m.total_cmp(&a.m))
                .then(a.r.ruta.cmp(&b.r.ruta))
        });
        Ok(resultados.into_iter().map(|c| c.r).collect())
    }

    // ---------------------------------------------------------------- enlaces y grafo

    /// Los `[[enlaces]]` que salen de la nota.
    pub fn enlaces(&self, ruta: &str) -> R<Vec<Enlace>> {
        let rel = self.rel(ruta)?;
        let info = self.info(&rel).ok_or_else(|| ErrorVault::NoExiste(rel.clone()))?;
        let todas = self.todas();
        let mapa = self.mapa(&todas);
        let (mut salida, mut vistos) = (Vec::new(), HashSet::new());
        for (nombre, seccion, texto, _) in &info.enlaces {
            if !vistos.insert((nombre.to_lowercase(), seccion.to_lowercase())) {
                continue;
            }
            let destino = if nombre.is_empty() {
                Some(rel.clone())
            } else {
                self.resolver_en(&mapa, nombre, Some(&rel))
            };
            salida.push(Enlace {
                nombre: nombre.clone(),
                existe: destino.is_some(),
                ruta: destino,
                seccion: seccion.clone(),
                texto: texto.clone(),
            });
        }
        Ok(salida)
    }

    /// Las notas que enlazan a esta (con la línea donde está el enlace).
    pub fn retroenlaces(&self, ruta: &str) -> R<Vec<Retroenlace>> {
        let rel = self.rel(ruta)?;
        let todas = self.todas();
        let mapa = self.mapa(&todas);
        let mut salida = Vec::new();
        for info in &todas {
            if info.ruta == rel {
                continue;
            }
            let lleva = |nombre: &str| {
                !nombre.is_empty()
                    && self.resolver_en(&mapa, nombre, Some(&info.ruta)).as_deref() == Some(rel.as_str())
            };
            if !info.enlaces.iter().any(|e| lleva(&e.0)) {
                continue;
            }
            let texto = self.leer(&info.ruta)?;
            let mut fragmento = String::new();
            for e in enlaces_de(&texto) {
                if lleva(&e.nombre) {
                    let inicio = texto[..e.inicio].rfind('\n').map_or(0, |p| p + 1);
                    let fin = texto[e.fin..].find('\n').map_or(texto.len(), |p| e.fin + p);
                    let linea = texto[inicio..fin].trim();
                    fragmento = if linea.chars().count() <= 220 {
                        linea.to_string()
                    } else {
                        let ci = texto[..e.inicio].chars().count();
                        let cf = texto[..e.fin].chars().count();
                        let ini_l = texto[..inicio].chars().count();
                        let lin: Vec<char> = linea.chars().collect();
                        recorte(&lin, ci.saturating_sub(ini_l), cf.saturating_sub(ini_l))
                    };
                    break;
                }
            }
            salida.push(Retroenlace {
                ruta: info.ruta.clone(),
                titulo: info.titulo.clone(),
                fragmento,
            });
        }
        Ok(salida)
    }

    /// Los nodos y las aristas. Los enlaces a notas que todavía no existen salen como nodos con
    /// `existe = false` e id «?nombre». Los incrustados de imágenes u otros archivos no cuentan.
    pub fn grafo(&self) -> Grafo {
        static EXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\.[a-z0-9]{2,5}$").unwrap());
        let todas = self.todas();
        let mapa = self.mapa(&todas);
        let mut nodos: Vec<NodoGrafo> = todas
            .iter()
            .map(|i| NodoGrafo {
                id: i.ruta.clone(),
                titulo: i.titulo.clone(),
                etiquetas: i.etiquetas.clone(),
                grado: 0,
                existe: true,
            })
            .collect();
        let mut indice: HashMap<String, usize> =
            nodos.iter().enumerate().map(|(i, n)| (n.id.clone(), i)).collect();
        let mut aristas: BTreeSet<(String, String)> = BTreeSet::new();
        for info in &todas {
            for (nombre, ..) in &info.enlaces {
                if nombre.is_empty() {
                    continue;
                }
                if let Some(ext) = Path::new(nombre).extension() {
                    let sufijo = format!(".{}", ext.to_string_lossy().to_lowercase());
                    if sufijo != ".md" && EXT.is_match(&sufijo) {
                        continue; // foto.png, documento.pdf…
                    }
                }
                let destino = match self.resolver_en(&mapa, nombre, Some(&info.ruta)) {
                    Some(d) => d,
                    None => {
                        let id = format!("?{nombre}");
                        if !indice.contains_key(&id) {
                            indice.insert(id.clone(), nodos.len());
                            nodos.push(NodoGrafo {
                                id: id.clone(),
                                titulo: nombre.clone(),
                                etiquetas: vec![],
                                grado: 0,
                                existe: false,
                            });
                        }
                        id
                    }
                };
                if destino == info.ruta {
                    continue;
                }
                aristas.insert((info.ruta.clone(), destino));
            }
        }
        for (o, d) in &aristas {
            nodos[indice[o]].grado += 1;
            nodos[indice[d]].grado += 1;
        }
        Grafo {
            nodos,
            aristas: aristas.into_iter().collect(),
        }
    }

    /// `{"etiqueta": cuántas notas la tienen}`, de la más usada a la menos.
    pub fn etiquetas(&self) -> Vec<(String, usize)> {
        let mut cuenta: HashMap<String, usize> = HashMap::new();
        for i in self.todas() {
            for t in &i.etiquetas {
                *cuenta.entry(t.clone()).or_default() += 1;
            }
        }
        let mut v: Vec<(String, usize)> = cuenta.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        v
    }
}
