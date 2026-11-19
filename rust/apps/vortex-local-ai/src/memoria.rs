//! «Preguntar a mi vault»: la IA contesta con lo que dicen tus notas (RAG), igual que `memoria.py`.
//!
//! Cada nota se parte en fragmentos de ~800 caracteres (por sección y párrafo) y un modelo de embeddings
//! (qwen3-embedding, nomic-embed-text…) los vuelve vectores. Se guardan en un SQLite en `cache/memoria/`
//! (el mismo archivo y las mismas tablas que usa la de Python); al preguntar se busca el fragmento más
//! parecido (similitud coseno) y se le pasa al modelo de chat pidiéndole que cite con `[[título]]`.
//!
//! El índice es incremental: solo vuelve a leer las notas que cambiaron (por fecha, y luego por hash) y saca
//! las que se borraron. Si no hay modelo de embeddings, busca por palabras con `Vault::buscar` y lo avisa.

use crate::catalogo;
use crate::http::Cancelar;
use crate::ollama::{ErrorOllama, Ollama};
use crate::sha1;
use crate::texto::normal;
use crate::vault::{Vault, separar_frontmatter};
use regex::Regex;
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

pub const TAMANO: usize = 800; // caracteres por fragmento, más o menos
pub const LOTE: usize = 16; // fragmentos por llamada a /api/embed
/// `al_progreso(hechas, total, ruta)`.
pub type AlProgreso<'a> = &'a mut dyn FnMut(usize, usize, &str);

pub const POR_NOTA: usize = 2; // como mucho, fragmentos de la misma nota en un contexto

static TITULO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.*\S)\s*$").unwrap());
static PALABRA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\w#-]+").unwrap());

const VACIAS: &str = "
a al algo algun alguna algunas alguno algunos ante antes aqui asi aun bien cada como con contra cual cuales cuando
cuanto de del desde donde dos el ella ellas ellos en entre era eran es esa esas ese eso esos esta estaba estan estar
estas este esto estos fue fueron ha han hasta hay la las le les lo los mas me mi mis mucho muy nada ni no nos nosotros
nuestra nuestro o os otra otro para pero poco por porque que quien quienes se sea segun ser si sin sobre solo son su
sus tambien tan tanto te tengo ti tiene tienen todo todos tu tus un una unas uno unos usted ya yo dime dice decir hago
hace hacer puedo puede sabes saber cosa cosas mis notas nota vault
the of and to in is it for on that this what how
";

fn vacia(p: &str) -> bool {
    VACIAS.split_whitespace().any(|v| v == p)
}

/// `(?<=[.!?…])\s+` como separador.
fn partir_oraciones(p: &str) -> Vec<String> {
    let c: Vec<char> = p.chars().collect();
    let (mut salida, mut desde, mut i) = (Vec::new(), 0, 1);
    while i < c.len() {
        if c[i].is_whitespace() && matches!(c[i - 1], '.' | '!' | '?' | '…') {
            let mut j = i;
            while j < c.len() && c[j].is_whitespace() {
                j += 1;
            }
            salida.push(c[desde..i].iter().collect());
            desde = j;
            i = j;
        } else {
            i += 1;
        }
    }
    salida.push(c[desde..].iter().collect());
    salida
}

fn largo(t: &str) -> usize {
    t.chars().count()
}

/// `[(sección, texto)]` de ~`TAMANO` caracteres, sin el frontmatter. La sección es el último título (#).
pub fn fragmentar(texto: &str) -> Vec<(String, String)> {
    let (_, cuerpo) = separar_frontmatter(texto);
    let mut secciones: Vec<(String, Vec<String>)> = vec![(String::new(), vec![])];
    let mut parrafo: Vec<String> = Vec::new();
    let mut en_codigo = false;
    fn cerrar(secciones: &mut [(String, Vec<String>)], parrafo: &mut Vec<String>) {
        if !parrafo.is_empty() {
            secciones
                .last_mut()
                .unwrap()
                .1
                .push(parrafo.join("\n").trim().to_string());
            parrafo.clear();
        }
    }
    for linea in cuerpo.split('\n') {
        let sin = linea.trim_start();
        if sin.starts_with("```") || sin.starts_with("~~~") {
            en_codigo = !en_codigo;
        }
        let m = if en_codigo { None } else { TITULO.captures(linea) };
        if let Some(m) = m {
            cerrar(&mut secciones, &mut parrafo);
            secciones.push((m[2].trim().to_string(), vec![]));
        } else if linea.trim().is_empty() && !en_codigo {
            cerrar(&mut secciones, &mut parrafo);
        } else {
            parrafo.push(linea.trim_end().to_string());
        }
    }
    cerrar(&mut secciones, &mut parrafo);

    let mut salida: Vec<(String, String)> = Vec::new();
    for (seccion, parrafos) in secciones {
        let mut piezas: Vec<String> = Vec::new();
        for p in parrafos {
            if p.is_empty() {
                continue;
            }
            if largo(&p) <= TAMANO {
                piezas.push(p);
                continue;
            }
            let mut actual = String::new();
            for oracion in partir_oraciones(&p) {
                let mut o: Vec<char> = oracion.chars().collect();
                while o.len() > TAMANO {
                    if !actual.is_empty() {
                        piezas.push(std::mem::take(&mut actual));
                    }
                    let corte = o[..TAMANO].iter().rposition(|c| *c == ' ');
                    let corte = match corte {
                        Some(c) if c > TAMANO / 2 => c,
                        _ => TAMANO,
                    };
                    piezas.push(o[..corte].iter().collect::<String>().trim().to_string());
                    o = o[corte..].iter().collect::<String>().trim().chars().collect();
                }
                let o: String = o.into_iter().collect();
                if !actual.is_empty() && largo(&actual) + 1 + largo(&o) > TAMANO {
                    piezas.push(std::mem::take(&mut actual));
                }
                actual = format!("{actual} {o}").trim().to_string();
            }
            if !actual.is_empty() {
                piezas.push(actual);
            }
        }
        let mut actual = String::new();
        for pieza in &piezas {
            if !actual.is_empty() && largo(&actual) + 2 + largo(pieza) > TAMANO {
                salida.push((seccion.clone(), std::mem::take(&mut actual)));
            }
            actual = if actual.is_empty() {
                pieza.clone()
            } else {
                format!("{actual}\n\n{pieza}")
            };
        }
        if !actual.is_empty() {
            salida.push((seccion.clone(), actual));
        } else if !seccion.is_empty() && piezas.is_empty() {
            salida.push((seccion.clone(), seccion.clone())); // un título sin texto también dice algo
        }
    }
    salida.into_iter().filter(|(_, t)| !t.trim().is_empty()).collect()
}

fn palabras(texto: &str) -> Vec<String> {
    let mut vistas: Vec<String> = Vec::new();
    for m in PALABRA.find_iter(&normal(texto)) {
        let p = m.as_str().trim_matches('-').to_string();
        if largo(&p) >= 3 && !vacia(&p) && !vistas.contains(&p) {
            vistas.push(p);
        }
    }
    vistas
}

fn vector_a_bytes(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_ne_bytes()).collect()
}

fn bytes_a_vector(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_ne_bytes(*c))
        .collect()
}

fn redondear4(x: f64) -> f64 {
    (x * 1e4).round() / 1e4
}

fn mal(e: impl std::fmt::Display) -> ErrorOllama {
    ErrorOllama::otro(format!("Algo salió mal: {e}"))
}

/// Un fragmento de una nota que sirve para contestar.
#[derive(Debug, Clone, PartialEq)]
pub struct Fragmento {
    pub ruta: String,
    pub titulo: String,
    pub fragmento: String,
    pub puntaje: f64,
    pub seccion: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Resumen {
    pub notas: usize,
    pub nuevas: usize,
    pub quitadas: usize,
    pub fragmentos: usize,
    pub cancelado: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Estado {
    pub notas: i64,
    pub fragmentos: i64,
    pub modelo: Option<String>,
    pub modo: String,
}

struct Fila {
    ruta: String,
    titulo: String,
    seccion: String,
    texto: String,
    vector: Vec<f32>,
    norma: f64,
}

/// El índice de un vault para preguntarle cosas. Úsala desde un hilo aparte: tarda.
pub struct Memoria {
    pub vault: Vault,
    ollama: Arc<Ollama>,
    modelo_fijo: Option<String>,
    modelo: Option<String>,
    pub ultimo_contexto: Vec<Fragmento>,
    cache_vectores: Option<(String, Arc<Vec<Fila>>)>,
    pub ruta_indice: PathBuf,
}

impl Memoria {
    /// `cache` es la carpeta de caché de la app (`Rutas::cache`): el índice va en `cache/memoria/`.
    pub fn new(
        vault: Vault,
        ollama: Arc<Ollama>,
        modelo_embeddings: Option<String>,
        cache: &Path,
    ) -> Memoria {
        let raiz = vault.raiz.to_string_lossy().to_string();
        let sucio = Regex::new(r"[^\w-]+")
            .unwrap()
            .replace_all(&vault.nombre, "-")
            .to_string();
        let mut nombre: String = sucio.trim_matches('-').chars().take(40).collect();
        if nombre.is_empty() {
            nombre = "vault".into();
        }
        let ruta_indice = cache
            .join("memoria")
            .join(format!("{nombre}-{}.sqlite", &sha1::hex(raiz.as_bytes())[..10]));
        let modelo_fijo = modelo_embeddings.filter(|m| !m.is_empty());
        Memoria {
            vault,
            ollama,
            modelo: modelo_fijo.clone(),
            modelo_fijo,
            ultimo_contexto: vec![],
            cache_vectores: None,
            ruta_indice,
        }
    }

    // ---------------------------------------------------------------- modelo y modo

    fn buscar_modelo(&self) -> Option<String> {
        if let Some(m) = &self.modelo_fijo {
            return Some(m.clone());
        }
        let Ok(instalados) = self.ollama.modelos() else {
            return self.modelo.clone(); // sin Ollama: el que ya se había encontrado (si hubo)
        };
        let tiene = |id: &str| -> Option<String> {
            instalados
                .iter()
                .map(|m| &m.nombre)
                .find(|n| {
                    **n == id
                        || **n == format!("{id}:latest")
                        || (!id.contains(':') && n.split(':').next() == Some(id))
                })
                .cloned()
        };
        let preferidos =
            std::iter::once(catalogo::EMBEDDINGS).chain(catalogo::de_tipo("vault").into_iter().map(|m| m.id));
        for p in preferidos {
            if let Some(e) = tiene(p) {
                return Some(e);
            }
        }
        instalados.iter().find(|m| m.embeddings).map(|m| m.nombre.clone())
    }

    pub fn modelo_embeddings(&self) -> Option<&str> {
        self.modelo.as_deref()
    }

    /// «embeddings» si hay modelo de embeddings, si no «palabras».
    pub fn modo(&self) -> &'static str {
        if self.modelo.is_some() {
            "embeddings"
        } else {
            "palabras"
        }
    }

    pub fn aviso(&self) -> String {
        if self.modelo.is_some() {
            return String::new();
        }
        format!(
            "Busco en tus notas por palabras porque no tienes un modelo para el vault. Baja \
             «{}» en Modelos para que entienda lo que quieres decir, no solo las palabras.",
            catalogo::EMBEDDINGS
        )
    }

    // ---------------------------------------------------------------- SQLite

    fn conectar(&self) -> Result<Connection, ErrorOllama> {
        if let Some(p) = self.ruta_indice.parent() {
            std::fs::create_dir_all(p).map_err(mal)?;
        }
        let db = Connection::open(&self.ruta_indice).map_err(mal)?;
        db.busy_timeout(Duration::from_secs(10)).map_err(mal)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS meta (clave TEXT PRIMARY KEY, valor TEXT);
             CREATE TABLE IF NOT EXISTS notas (ruta TEXT PRIMARY KEY, mtime REAL, hash TEXT);
             CREATE TABLE IF NOT EXISTS fragmentos (id INTEGER PRIMARY KEY, ruta TEXT, titulo TEXT, seccion TEXT,
                                                    texto TEXT, vector BLOB, norma REAL);
             CREATE INDEX IF NOT EXISTS fragmentos_ruta ON fragmentos(ruta);",
        )
        .map_err(mal)?;
        Ok(db)
    }

    fn meta(db: &Connection, clave: &str) -> Option<String> {
        db.query_row("SELECT valor FROM meta WHERE clave=?", [clave], |f| f.get(0))
            .ok()
    }

    fn poner_meta(db: &Connection, clave: &str, valor: &str) -> rusqlite::Result<usize> {
        db.execute("INSERT OR REPLACE INTO meta VALUES (?, ?)", params![clave, valor])
    }

    pub fn estado(&self) -> Result<Estado, ErrorOllama> {
        let db = self.conectar()?;
        let n = |t: &str| {
            db.query_row(&format!("SELECT COUNT(*) FROM {t}"), [], |f| f.get::<_, i64>(0))
                .map_err(mal)
        };
        Ok(Estado {
            notas: n("notas")?,
            fragmentos: n("fragmentos")?,
            modelo: Self::meta(&db, "modelo").or_else(|| self.modelo.clone()),
            modo: self.modo().to_string(),
        })
    }

    pub fn borrar_indice(&mut self) {
        self.cache_vectores = None;
        for sufijo in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{sufijo}", self.ruta_indice.display()));
        }
    }

    // ---------------------------------------------------------------- prefijos de cada modelo

    fn texto_documento(&self, titulo: &str, seccion: &str, texto: &str) -> String {
        let cabeza = if !seccion.is_empty() && seccion != titulo {
            format!("{titulo} › {seccion}")
        } else {
            titulo.to_string()
        };
        let base = format!("{cabeza}\n{texto}");
        if self.modelo.as_deref().unwrap_or("").contains("nomic") {
            format!("search_document: {base}")
        } else {
            base
        }
    }

    fn texto_pregunta(&self, pregunta: &str) -> String {
        let modelo = self.modelo.as_deref().unwrap_or("");
        if modelo.contains("nomic") {
            format!("search_query: {pregunta}")
        } else if modelo.contains("qwen3-embedding") {
            format!("Instruct: Dada una pregunta, encuentra las notas que la contestan\nQuery: {pregunta}")
        } else {
            pregunta.to_string()
        }
    }

    // ---------------------------------------------------------------- índice

    /// Pone al día el índice. `al_progreso(hechas, total, ruta)`. Regresa lo que hizo.
    pub fn actualizar(
        &mut self,
        mut al_progreso: Option<AlProgreso>,
        cancelar: Option<&Cancelar>,
    ) -> Result<Resumen, ErrorOllama> {
        self.modelo = self.buscar_modelo();
        let notas = self.vault.notas();
        let mut resumen = Resumen {
            notas: notas.len(),
            ..Resumen::default()
        };
        let Some(modelo) = self.modelo.clone() else {
            return Ok(resumen);
        };
        let cancelado = || cancelar.is_some_and(Cancelar::is_set);
        let mut db = self.conectar()?;
        if Self::meta(&db, "modelo").as_deref() != Some(modelo.as_str()) {
            db.execute_batch("DELETE FROM fragmentos; DELETE FROM notas;")
                .map_err(mal)?;
            Self::poner_meta(&db, "modelo", &modelo).map_err(mal)?;
        }
        let vivas: HashMap<&str, &crate::vault::Nota> = notas.iter().map(|n| (n.ruta.as_str(), n)).collect();
        let mut guardadas: HashMap<String, (f64, String)> = HashMap::new();
        {
            let mut q = db.prepare("SELECT ruta, mtime, hash FROM notas").map_err(mal)?;
            let filas = q
                .query_map([], |f| {
                    Ok((
                        f.get::<_, String>(0)?,
                        f.get::<_, f64>(1)?,
                        f.get::<_, String>(2)?,
                    ))
                })
                .map_err(mal)?;
            for f in filas.flatten() {
                guardadas.insert(f.0, (f.1, f.2));
            }
        }
        for ruta in guardadas.keys().filter(|r| !vivas.contains_key(r.as_str())) {
            db.execute("DELETE FROM fragmentos WHERE ruta=?", [ruta])
                .map_err(mal)?;
            db.execute("DELETE FROM notas WHERE ruta=?", [ruta])
                .map_err(mal)?;
            resumen.quitadas += 1;
        }

        let mut pendientes: Vec<(&crate::vault::Nota, String, String)> = Vec::new();
        for nota in &notas {
            let anterior = guardadas.get(&nota.ruta);
            if anterior.is_some_and(|a| a.0 == nota.modificado) {
                continue;
            }
            let Ok(texto) = self.vault.leer(&nota.ruta) else {
                continue;
            };
            let huella = sha1::hex(texto.as_bytes());
            if anterior.is_some_and(|a| a.1 == huella) {
                db.execute(
                    "UPDATE notas SET mtime=? WHERE ruta=?",
                    params![nota.modificado, nota.ruta],
                )
                .map_err(mal)?;
                continue;
            }
            pendientes.push((nota, texto, huella));
        }

        let total = pendientes.len();
        for (i, (nota, texto, huella)) in pendientes.iter().enumerate() {
            if cancelado() {
                resumen.cancelado = true;
                break;
            }
            let trozos = fragmentar(texto);
            let mut vectores: Vec<Vec<f32>> = Vec::new();
            for lote in trozos.chunks(LOTE) {
                if cancelado() {
                    resumen.cancelado = true;
                    break;
                }
                let textos: Vec<String> = lote
                    .iter()
                    .map(|(s, t)| self.texto_documento(&nota.titulo, s, t))
                    .collect();
                vectores.extend(self.ollama.embeber(&modelo, &textos)?);
            }
            if resumen.cancelado {
                break;
            }
            let tx = db.transaction().map_err(mal)?;
            tx.execute("DELETE FROM fragmentos WHERE ruta=?", [&nota.ruta])
                .map_err(mal)?;
            for ((seccion, trozo), vector) in trozos.iter().zip(&vectores) {
                let norma = vector
                    .iter()
                    .map(|x| f64::from(*x) * f64::from(*x))
                    .sum::<f64>()
                    .sqrt();
                let norma = if norma == 0.0 { 1.0 } else { norma };
                tx.execute(
                    "INSERT INTO fragmentos (ruta, titulo, seccion, texto, vector, norma) VALUES (?, ?, ?, ?, ?, ?)",
                    params![nota.ruta, nota.titulo, seccion, trozo, vector_a_bytes(vector), norma],
                )
                .map_err(mal)?;
            }
            tx.execute(
                "INSERT OR REPLACE INTO notas VALUES (?, ?, ?)",
                params![nota.ruta, nota.modificado, huella],
            )
            .map_err(mal)?;
            tx.commit().map_err(mal)?;
            resumen.nuevas += 1;
            if let Some(f) = al_progreso.as_mut() {
                f(i + 1, total, &nota.ruta);
            }
        }
        resumen.fragmentos = db
            .query_row("SELECT COUNT(*) FROM fragmentos", [], |f| f.get::<_, i64>(0))
            .map_err(mal)? as usize;
        let version = Self::meta(&db, "version")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
            + 1;
        Self::poner_meta(&db, "version", &version.to_string()).map_err(mal)?;
        self.cache_vectores = None;
        Ok(resumen)
    }

    /// Todos los fragmentos con su vector (en memoria mientras el índice no cambie).
    fn vectores(&mut self) -> Result<Arc<Vec<Fila>>, ErrorOllama> {
        let db = self.conectar()?;
        let version = format!(
            "{}:{}",
            Self::meta(&db, "modelo").unwrap_or_else(|| "None".into()),
            Self::meta(&db, "version").unwrap_or_else(|| "None".into())
        );
        if let Some((v, filas)) = &self.cache_vectores
            && *v == version
        {
            return Ok(filas.clone());
        }
        let mut q = db
            .prepare("SELECT ruta, titulo, seccion, texto, vector, norma FROM fragmentos")
            .map_err(mal)?;
        let filas: Vec<Fila> = q
            .query_map([], |f| {
                let blob: Vec<u8> = f.get(4)?;
                Ok(Fila {
                    ruta: f.get(0)?,
                    titulo: f.get(1)?,
                    seccion: f.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    texto: f.get(3)?,
                    vector: bytes_a_vector(&blob),
                    norma: f.get::<_, f64>(5)?,
                })
            })
            .map_err(mal)?
            .flatten()
            .collect();
        let filas = Arc::new(filas);
        self.cache_vectores = Some((version, filas.clone()));
        Ok(filas)
    }

    // ---------------------------------------------------------------- buscar contexto

    /// Los `k` fragmentos que mejor contestan la pregunta.
    pub fn contexto(&mut self, pregunta: &str, k: usize) -> Result<Vec<Fragmento>, ErrorOllama> {
        if pregunta.trim().is_empty() {
            return Ok(vec![]);
        }
        if self.modelo.is_none() {
            self.modelo = self.buscar_modelo();
        }
        let filas = if self.modelo.is_some() {
            self.vectores()?
        } else {
            Arc::new(vec![])
        };
        let Some(modelo) = self.modelo.clone().filter(|_| !filas.is_empty()) else {
            return Ok(self.por_palabras(pregunta, k));
        };
        let consulta = self
            .ollama
            .embeber(&modelo, &[self.texto_pregunta(pregunta)])?
            .into_iter()
            .next()
            .unwrap_or_default();
        let norma_q = consulta
            .iter()
            .map(|x| f64::from(*x) * f64::from(*x))
            .sum::<f64>()
            .sqrt();
        let norma_q = if norma_q == 0.0 { 1.0 } else { norma_q };
        let puntajes: Vec<f64> = filas
            .iter()
            .map(|f| {
                if f.vector.len() == consulta.len() {
                    let punto: f64 = f
                        .vector
                        .iter()
                        .zip(&consulta)
                        .map(|(a, b)| f64::from(*a) * f64::from(*b))
                        .sum();
                    punto / (f.norma * norma_q)
                } else {
                    -1.0
                }
            })
            .collect();
        let mut orden: Vec<usize> = (0..filas.len()).collect();
        orden.sort_by(|a, b| puntajes[*b].total_cmp(&puntajes[*a])); // estable, como sorted()
        let (mut salida, mut por_nota) = (Vec::new(), HashMap::<&str, usize>::new());
        for i in orden {
            let f = &filas[i];
            if por_nota.get(f.ruta.as_str()).copied().unwrap_or(0) >= POR_NOTA || !self.vault.existe(&f.ruta)
            {
                continue;
            }
            *por_nota.entry(&f.ruta).or_default() += 1;
            salida.push(Fragmento {
                ruta: f.ruta.clone(),
                titulo: f.titulo.clone(),
                fragmento: f.texto.clone(),
                puntaje: redondear4(puntajes[i]),
                seccion: f.seccion.clone(),
            });
            if salida.len() >= k {
                break;
            }
        }
        Ok(salida)
    }

    /// Sin embeddings: las notas que más palabras de la pregunta tienen, con su mejor fragmento.
    fn por_palabras(&self, pregunta: &str, k: usize) -> Vec<Fragmento> {
        let pals = palabras(pregunta);
        if pals.is_empty() {
            return vec![];
        }
        struct N {
            titulo: String,
            palabras: usize,
            veces: usize,
        }
        let mut orden_de_llegada: Vec<String> = Vec::new();
        let mut notas: HashMap<String, N> = HashMap::new();
        for p in &pals {
            for r in self.vault.buscar(p).unwrap_or_default() {
                let n = notas.entry(r.ruta.clone()).or_insert_with(|| {
                    orden_de_llegada.push(r.ruta.clone());
                    N {
                        titulo: r.titulo.clone(),
                        palabras: 0,
                        veces: 0,
                    }
                });
                n.palabras += 1;
                n.veces += r.coincidencias;
            }
        }
        let mut orden: Vec<(String, N)> = notas.into_iter().collect();
        orden.sort_by(|a, b| {
            b.1.palabras
                .cmp(&a.1.palabras)
                .then(b.1.veces.cmp(&a.1.veces))
                .then(a.0.cmp(&b.0))
        });
        orden.truncate(k);
        let mut salida = Vec::new();
        for (ruta, n) in orden {
            let Ok(texto) = self.vault.leer(&ruta) else {
                continue;
            };
            let mut trozos = fragmentar(&texto);
            if trozos.is_empty() {
                trozos = vec![(String::new(), n.titulo.clone())];
            }
            let puntos = |st: &(String, String)| -> usize {
                let t = normal(&format!("{} {}", st.0, st.1));
                pals.iter().map(|p| t.matches(p.as_str()).count()).sum()
            };
            let mut mejor = &trozos[0];
            for t in &trozos[1..] {
                if puntos(t) > puntos(mejor) {
                    mejor = t;
                }
            }
            salida.push(Fragmento {
                ruta,
                titulo: n.titulo,
                fragmento: mejor.1.clone(),
                puntaje: redondear4(n.palabras as f64 / pals.len() as f64),
                seccion: mejor.0.clone(),
            });
        }
        salida
    }

    // ---------------------------------------------------------------- mensajes para el chat

    /// `[system, *historial, pregunta]` para `Ollama::chat`, con los fragmentos de tus notas en el system.
    pub fn mensajes_con_contexto(
        &mut self,
        pregunta: &str,
        historial: &[Value],
    ) -> Result<Vec<Value>, ErrorOllama> {
        let fragmentos = self.contexto(pregunta, 6)?;
        self.ultimo_contexto = fragmentos.clone();
        let mut partes: Vec<String> = vec![
            "Eres la IA de Vortexine's Local AI y contestas preguntas sobre las notas del vault de la persona.".into(),
            "Contesta siempre en español, claro y directo.".into(),
            "Usa la información de las notas de abajo. Si no alcanza para contestar, dilo con honestidad y no \
             inventes nada."
                .into(),
            "Cada vez que uses algo de una nota, cítala con su título entre dobles corchetes, exactamente como \
             aparece, por ejemplo [[Título de la nota]]."
                .into(),
        ];
        if fragmentos.is_empty() {
            partes.push(
                "\nNo encontré notas relacionadas con esta pregunta. Dilo, y si contestas algo general, \
                 aclara que no viene de sus notas."
                    .into(),
            );
        } else {
            if self.modo() == "palabras" || self.modelo.is_none() {
                partes.push("(Estas notas se encontraron buscando las palabras de la pregunta.)".into());
            }
            partes.push("\nFragmentos de las notas:".into());
            for f in &fragmentos {
                let cabeza = format!("[[{}]]", f.titulo)
                    + &if f.seccion.is_empty() {
                        String::new()
                    } else {
                        format!(" › {}", f.seccion)
                    };
                partes.push(format!("\n--- {cabeza} ({}) ---\n{}", f.ruta, f.fragmento.trim()));
            }
        }
        let mut mensajes = vec![json!({"role": "system", "content": partes.join("\n")})];
        for m in historial {
            let rol = m.get("role").and_then(Value::as_str).unwrap_or("");
            if (rol == "user" || rol == "assistant") && m.get("content").is_some_and(|c| !c.is_null()) {
                let mut limpio = json!({"role": rol, "content": m["content"]});
                if m.get("images")
                    .is_some_and(|i| i.as_array().is_some_and(|a| !a.is_empty()))
                {
                    limpio["images"] = m["images"].clone();
                }
                mensajes.push(limpio);
            }
        }
        mensajes.push(json!({"role": "user", "content": pregunta}));
        Ok(mensajes)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn oraciones() {
        assert_eq!(
            partir_oraciones("Hola. ¿Qué tal? Bien"),
            vec!["Hola.", "¿Qué tal?", "Bien"]
        );
        assert_eq!(partir_oraciones("a  b"), vec!["a  b"]);
    }
}
