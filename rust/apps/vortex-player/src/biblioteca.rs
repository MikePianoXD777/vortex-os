//! Biblioteca musical en SQLite, escaneo de carpetas, playlists y `ajustes.json`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use thiserror::Error;
use uuid::Uuid;

use crate::{calidad, letras, nube, rutas};

/// Identificador fijo en la nube de la playlist «Tus me gusta» (compartido con Android).
pub const ME_GUSTA: &str = "me-gusta";
pub const NOMBRE_ME_GUSTA: &str = "Tus me gusta";

const ESQUEMA: &str = r#"
CREATE TABLE IF NOT EXISTS canciones (
    ruta TEXT PRIMARY KEY, mtime REAL, titulo TEXT, artista TEXT, album TEXT, pista INTEGER,
    duracion REAL, codec TEXT, bits INTEGER, hz INTEGER, kbps INTEGER, portada INTEGER,
    origen TEXT DEFAULT '', agregada REAL
);
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL, url TEXT DEFAULT '', creada REAL
);
CREATE TABLE IF NOT EXISTS playlist_canciones (
    playlist INTEGER REFERENCES playlists(id) ON DELETE CASCADE, pos INTEGER, ruta TEXT,
    PRIMARY KEY (playlist, pos)
);
CREATE TABLE IF NOT EXISTS borradas (tipo TEXT, clave TEXT, cuando REAL, PRIMARY KEY (tipo, clave));
CREATE TABLE IF NOT EXISTS escuchas (cuando REAL, ruta TEXT, titulo TEXT, artista TEXT, segundos REAL);
CREATE INDEX IF NOT EXISTS escuchas_cuando ON escuchas (cuando);
"#;

#[derive(Debug, Error)]
pub enum ErrorBiblioteca {
    #[error("Error de SQLite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Error de entrada/salida: {0}")]
    Io(#[from] std::io::Error),
    #[error("Error de JSON en ajustes: {0}")]
    Json(#[from] serde_json::Error),
}

/// `ajustes.json`, como el `dict` de la de Python: las claves que conoce (con su valor de fábrica) y todas
/// las demás que guarde la app (`discord`, `despertador`, `aparato`…), que se conservan al guardar.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Ajustes(pub Map<String, Value>);

/// Los de fábrica (`AJUSTES_BASE` de la de Python).
fn ajustes_base() -> Map<String, Value> {
    let Value::Object(m) = json!({
        "carpetas": [],            // además de Música
        "navegador_premium": "",   // «firefox», «chrome»…: baja con tu sesión de YouTube Music Premium
        "volumen": 80,
        "ldac_hq": true,           // pedir LDAC a 990 kbps cuando hay audífonos que lo soportan
        "fundido": 5,              // segundos de crossfade entre canciones (0 = sin)
        "radio": true,             // al acabarse la cola siguen canciones parecidas (radio sin fin)
    }) else {
        unreachable!()
    };
    m
}

impl Ajustes {
    pub fn valor(&self, clave: &str) -> Option<&Value> {
        self.0.get(clave)
    }
    pub fn texto(&self, clave: &str) -> String {
        self.0
            .get(clave)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
    pub fn numero(&self, clave: &str, defecto: f64) -> f64 {
        self.0.get(clave).and_then(Value::as_f64).unwrap_or(defecto)
    }
    /// Como el `bool()` de Python: `null`, `false`, 0 y «» son falso.
    pub fn booleano(&self, clave: &str, defecto: bool) -> bool {
        match self.0.get(clave) {
            None => defecto,
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(o)) => !o.is_empty(),
            Some(Value::Null) => false,
        }
    }
    pub fn volumen(&self) -> i64 {
        self.numero("volumen", 80.0).round() as i64
    }
    pub fn carpetas(&self) -> Vec<String> {
        self.0
            .get("carpetas")
            .and_then(Value::as_array)
            .map(|v| v.iter().filter_map(|c| c.as_str().map(str::to_string)).collect())
            .unwrap_or_default()
    }
}

/// Orden disponible al consultar canciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orden {
    Agregada,
    Titulo,
    Artista,
    Calidad,
}

impl Orden {
    pub fn desde_str(s: &str) -> Self {
        match s {
            "titulo" => Self::Titulo,
            "artista" => Self::Artista,
            "calidad" => Self::Calidad,
            _ => Self::Agregada,
        }
    }
}

/// Canción almacenada en la biblioteca junto con su etiqueta y nivel calculados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cancion {
    pub ruta: String,
    pub mtime: f64,
    pub titulo: String,
    pub artista: String,
    pub album: String,
    pub pista: i64,
    pub duracion: f64,
    pub codec: String,
    pub bits: i64,
    pub hz: i64,
    pub kbps: i64,
    pub portada: bool,
    pub origen: String,
    pub agregada: f64,
    pub etiqueta: String,
    pub nivel: String,
}

/// Resumen de una playlist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: i64,
    pub nombre: String,
    pub url: String,
    pub nube: String,
    pub canciones: i64,
    #[serde(rename = "meGusta")]
    pub me_gusta: bool,
}

/// Resultado de `Biblioteca::escanear`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ResultadoEscaneo {
    pub nuevas: usize,
    pub cambiadas: usize,
    pub borradas: usize,
    pub total: usize,
}

fn ahora_epoch() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn mtime_de(ruta: &Path) -> Option<f64> {
    let meta = fs::metadata(ruta).ok()?;
    let mod_time = meta.modified().ok()?;
    mod_time.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs_f64())
}

/// Extrae un ID de YouTube de 11 caracteres desde `[id].ext` o desde una URL `?v=id` / `&v=id`.
pub fn id_de(ruta: &str) -> String {
    if let Some(pos_corchete_abre) = ruta.rfind('[')
        && let Some(rel_cierre) = ruta[pos_corchete_abre + 1..].find(']')
    {
        let candidato = &ruta[pos_corchete_abre + 1..pos_corchete_abre + 1 + rel_cierre];
        let resto = &ruta[pos_corchete_abre + 1 + rel_cierre + 1..];
        if candidato.len() == 11
            && candidato
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            && resto.starts_with('.')
            && resto.len() > 1
            && resto[1..].bytes().all(|b| b.is_ascii_alphanumeric())
        {
            return candidato.to_string();
        }
    }
    if (ruta.starts_with("http://") || ruta.starts_with("https://"))
        && let Some(pos) = ruta.find("v=")
    {
        let prefix_ok = pos > 0 && matches!(ruta.as_bytes()[pos - 1], b'?' | b'&');
        if prefix_ok {
            let resto = &ruta[pos + 2..];
            if resto.len() >= 11 {
                let candidato = &resto[..11];
                if candidato
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return candidato.to_string();
                }
            }
        }
    }
    String::new()
}

/// Determina si la canción proviene de YouTube o es local.
pub fn origen_de(ruta: &str, extra: &HashMap<String, String>) -> &'static str {
    let en_extra = extra
        .values()
        .any(|v| v.contains("youtube.com") || v.contains("youtu.be"));
    if en_extra {
        return "youtube";
    }
    if let (Ok(r_abs), Ok(desc_abs)) = (fs::canonicalize(ruta), fs::canonicalize(rutas::descargas()))
        && r_abs.starts_with(&desc_abs)
    {
        return "youtube";
    }
    if Path::new(ruta).starts_with(rutas::descargas()) {
        return "youtube";
    }
    "local"
}

fn recoger_archivos_recursivo(dir: &Path, salida: &mut Vec<PathBuf>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };
    for entrada in entradas.flatten() {
        let nombre = entrada.file_name();
        let nombre_str = nombre.to_string_lossy();
        if nombre_str.starts_with('.') {
            continue;
        }
        let path = entrada.path();
        if path.is_dir() {
            recoger_archivos_recursivo(&path, salida);
        } else if path.is_file() && calidad::extension_soportada(&path) {
            salida.push(path);
        }
    }
}

/// Gestor de la biblioteca musical en SQLite.
pub struct Biblioteca {
    db: Connection,
    ffprobe: String,
}

impl Biblioteca {
    /// Abre (o crea) la base de datos en `archivo` (por defecto `rutas::datos()/biblioteca.db`).
    pub fn abrir(archivo: Option<&Path>, ffprobe: impl Into<String>) -> Result<Self, ErrorBiblioteca> {
        let ruta = match archivo {
            Some(p) => p.to_path_buf(),
            None => rutas::datos().join("biblioteca.db"),
        };
        if let Some(padre) = ruta.parent() {
            fs::create_dir_all(padre)?;
        }
        let db = Connection::open(&ruta)?;
        db.execute_batch("PRAGMA foreign_keys = ON;")?;
        db.execute_batch(ESQUEMA)?;

        // Migración compatible con 2.4 (`nube` y `modificada` en `playlists`)
        let mut columnas = HashSet::new();
        {
            let mut stmt = db.prepare("PRAGMA table_info(playlists)")?;
            let filas = stmt.query_map([], |row| row.get::<_, String>(1))?;
            for nombre in filas {
                columnas.insert(nombre?);
            }
        }
        if !columnas.contains("nube") {
            db.execute("ALTER TABLE playlists ADD COLUMN nube TEXT DEFAULT ''", [])?;
        }
        if !columnas.contains("modificada") {
            db.execute("ALTER TABLE playlists ADD COLUMN modificada REAL DEFAULT 0", [])?;
            db.execute("UPDATE playlists SET modificada = creada", [])?;
        }

        Ok(Self {
            db,
            ffprobe: ffprobe.into(),
        })
    }

    /// Lee `ajustes.json` (lo que falte, de fábrica).
    pub fn ajustes() -> Ajustes {
        Self::ajustes_en(&rutas::config())
    }

    /// Lee `ajustes.json` desde una carpeta concreta.
    pub fn ajustes_en(dir: &Path) -> Ajustes {
        let mut a = ajustes_base();
        if let Some(Value::Object(g)) = fs::read_to_string(dir.join("ajustes.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        {
            a.extend(g);
        }
        Ajustes(a)
    }

    /// Cambia una clave y guarda todo el archivo de una vez (con un temporal, para no dejarlo a medias).
    pub fn guardar_ajuste(clave: &str, valor: Value) -> Result<(), ErrorBiblioteca> {
        Self::guardar_ajuste_en(&rutas::config(), clave, valor)
    }

    pub fn guardar_ajuste_en(dir: &Path, clave: &str, valor: Value) -> Result<(), ErrorBiblioteca> {
        let mut a = Self::ajustes_en(dir);
        a.0.insert(clave.to_string(), valor);
        fs::create_dir_all(dir)?;
        let tmp = dir.join("ajustes.json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(&Value::Object(a.0))?)?;
        fs::rename(tmp, dir.join("ajustes.json"))?;
        Ok(())
    }

    /// Carpetas configuradas para escanear (`Música` + adicionales sin duplicados).
    pub fn carpetas(&self) -> Vec<PathBuf> {
        let mut vistas = Vec::new();
        let mut candidatos = vec![rutas::musica()];
        for c in Self::ajustes().carpetas() {
            candidatos.push(PathBuf::from(c));
        }
        for c in candidatos {
            if !vistas.contains(&c) {
                vistas.push(c);
            }
        }
        vistas
    }

    /// Lista todos los archivos de audio soportados dentro de `carpetas` ignorando ocultos.
    pub fn archivos_de(carpetas: &[PathBuf]) -> Vec<PathBuf> {
        let mut salida = Vec::new();
        for carpeta in carpetas {
            if carpeta.is_file() {
                if calidad::extension_soportada(carpeta) {
                    salida.push(carpeta.clone());
                }
            } else if carpeta.is_dir() {
                recoger_archivos_recursivo(carpeta, &mut salida);
            }
        }
        salida
    }

    /// Lee un archivo con `ffprobe` y lo inserta o actualiza en la tabla `canciones`.
    pub fn agregar_archivo(
        &self,
        ruta: &Path,
        origen: &str,
    ) -> Result<Option<calidad::Info>, ErrorBiblioteca> {
        let Some(mtime) = mtime_de(ruta) else {
            return Ok(None);
        };
        let Some(info) = calidad::leer(ruta, &self.ffprobe) else {
            return Ok(None);
        };
        let ruta_str = ruta.to_string_lossy().to_string();

        let previa: Option<(f64, String)> = self
            .db
            .query_row(
                "SELECT agregada, origen FROM canciones WHERE ruta = ?1",
                params![ruta_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if previa.is_none() {
            let vid = id_de(&ruta_str);
            if !vid.is_empty() {
                self.db.execute(
                    "DELETE FROM borradas WHERE tipo = 'cancion' AND clave = ?1",
                    params![vid],
                )?;
            }
        }

        let origen_final = if !origen.is_empty() {
            origen.to_string()
        } else if let Some((_, ref orig_prev)) = previa
            && !orig_prev.is_empty()
        {
            orig_prev.clone()
        } else {
            origen_de(&ruta_str, &info.extra).to_string()
        };

        let agregada = previa.map(|(a, _)| a).unwrap_or_else(ahora_epoch);

        self.db.execute(
            "INSERT OR REPLACE INTO canciones VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![
                ruta_str,
                mtime,
                info.titulo,
                info.artista,
                info.album,
                info.pista,
                info.duracion,
                info.codec,
                info.bits,
                info.hz,
                info.kbps,
                i64::from(info.portada),
                origen_final,
                agregada,
            ],
        )?;

        Ok(Some(info))
    }

    /// Escanea las carpetas configuradas y sincroniza nuevas, cambiadas y borradas.
    pub fn escanear(&self) -> Result<ResultadoEscaneo, ErrorBiblioteca> {
        self.escanear_carpetas(&self.carpetas())
    }

    /// Escanea una lista explícita de carpetas y sincroniza nuevas, cambiadas y borradas.
    pub fn escanear_carpetas(&self, carpetas: &[PathBuf]) -> Result<ResultadoEscaneo, ErrorBiblioteca> {
        let mut conocidas: HashMap<String, f64> = HashMap::new();
        {
            let mut stmt = self.db.prepare("SELECT ruta, mtime FROM canciones")?;
            let filas = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?)))?;
            for fila in filas {
                let (r, m) = fila?;
                conocidas.insert(r, m);
            }
        }

        let encontradas = Self::archivos_de(carpetas);
        let mut nuevas = 0usize;
        let mut cambiadas = 0usize;
        let mut vistas = HashSet::with_capacity(encontradas.len());

        for ruta in &encontradas {
            let ruta_str = ruta.to_string_lossy().to_string();
            vistas.insert(ruta_str.clone());
            let Some(mtime) = mtime_de(ruta) else {
                continue;
            };
            let antes = conocidas.get(&ruta_str).copied();
            let necesita_leer = match antes {
                None => true,
                Some(prev) => (prev - mtime).abs() > 0.5,
            };
            if necesita_leer && self.agregar_archivo(ruta, "")?.is_some() {
                if antes.is_none() {
                    nuevas += 1;
                } else {
                    cambiadas += 1;
                }
            }
        }

        let borradas: Vec<String> = conocidas
            .keys()
            .filter(|r| !vistas.contains(*r) && !Path::new(r).exists())
            .cloned()
            .collect();

        for r in &borradas {
            self.db
                .execute("DELETE FROM canciones WHERE ruta = ?1", params![r])?;
        }

        Ok(ResultadoEscaneo {
            nuevas,
            cambiadas,
            borradas: borradas.len(),
            total: vistas.len(),
        })
    }

    fn fila_a_cancion(row: &rusqlite::Row<'_>) -> rusqlite::Result<Cancion> {
        let ruta: String = row.get("ruta")?;
        let mtime: f64 = row.get("mtime")?;
        let mut titulo: String = row.get::<_, Option<String>>("titulo")?.unwrap_or_default();
        let mut artista: String = row.get::<_, Option<String>>("artista")?.unwrap_or_default();
        let album: String = row.get::<_, Option<String>>("album")?.unwrap_or_default();
        let pista: i64 = row.get::<_, Option<i64>>("pista")?.unwrap_or(0);
        let duracion: f64 = row.get::<_, Option<f64>>("duracion")?.unwrap_or(0.0);
        let codec: String = row.get::<_, Option<String>>("codec")?.unwrap_or_default();
        let bits: i64 = row.get::<_, Option<i64>>("bits")?.unwrap_or(0);
        let hz: i64 = row.get::<_, Option<i64>>("hz")?.unwrap_or(0);
        let kbps: i64 = row.get::<_, Option<i64>>("kbps")?.unwrap_or(0);
        let portada: bool = row.get::<_, Option<i64>>("portada")?.unwrap_or(0) != 0;
        let origen: String = row.get::<_, Option<String>>("origen")?.unwrap_or_default();
        let agregada: f64 = row.get::<_, Option<f64>>("agregada")?.unwrap_or(0.0);

        let etiqueta = calidad::etiqueta(&codec, bits, hz, kbps);
        let nivel = calidad::nivel(&codec, bits, hz).to_string();
        if !id_de(&ruta).is_empty() {
            titulo = letras::titulo_limpio(&titulo, &artista);
            artista = letras::artista_limpio(&artista);
        }

        Ok(Cancion {
            ruta,
            mtime,
            titulo,
            artista,
            album,
            pista,
            duracion,
            codec,
            bits,
            hz,
            kbps,
            portada,
            origen,
            agregada,
            etiqueta,
            nivel,
        })
    }

    /// Lista las canciones de la biblioteca filtrando opcionalmente por `buscar`.
    pub fn canciones(&self, buscar: &str, orden: Orden) -> Result<Vec<Cancion>, ErrorBiblioteca> {
        let orden_sql = match orden {
            Orden::Agregada => "agregada DESC",
            Orden::Titulo => "titulo COLLATE NOCASE",
            Orden::Artista => "artista COLLATE NOCASE, album COLLATE NOCASE, pista",
            Orden::Calidad => {
                "(codec IN ('flac','alac','wavpack','ape') OR codec LIKE 'pcm_%') DESC, bits DESC, hz DESC, kbps DESC"
            }
        };

        let q = buscar.trim();
        let mut salida = Vec::new();
        if q.is_empty() {
            let sql = format!("SELECT * FROM canciones ORDER BY {orden_sql}");
            let mut stmt = self.db.prepare(&sql)?;
            let filas = stmt.query_map([], Self::fila_a_cancion)?;
            for c in filas {
                salida.push(c?);
            }
        } else {
            let patron = format!("%{q}%");
            let sql = format!(
                "SELECT * FROM canciones WHERE titulo LIKE ?1 OR artista LIKE ?1 OR album LIKE ?1 ORDER BY {orden_sql}"
            );
            let mut stmt = self.db.prepare(&sql)?;
            let filas = stmt.query_map(params![patron], Self::fila_a_cancion)?;
            for c in filas {
                salida.push(c?);
            }
        }
        Ok(salida)
    }

    /// Devuelve una canción concreta por su ruta.
    pub fn cancion(&self, ruta: &str) -> Result<Option<Cancion>, ErrorBiblioteca> {
        let res = self
            .db
            .query_row(
                "SELECT * FROM canciones WHERE ruta = ?1",
                params![ruta],
                Self::fila_a_cancion,
            )
            .optional()?;
        Ok(res)
    }

    /// Quita una canción de la biblioteca y de todas las playlists (sin borrar el archivo).
    pub fn quitar(&self, ruta: &str) -> Result<(), ErrorBiblioteca> {
        self.db
            .execute("DELETE FROM canciones WHERE ruta = ?1", params![ruta])?;
        self.db
            .execute("DELETE FROM playlist_canciones WHERE ruta = ?1", params![ruta])?;
        Ok(())
    }

    /// Lista todas las playlists, con «Tus me gusta» primero.
    pub fn playlists(&self) -> Result<Vec<Playlist>, ErrorBiblioteca> {
        let mut stmt = self.db.prepare(
            "SELECT p.id, p.nombre, p.url, p.nube, COUNT(c.ruta) AS canciones
             FROM playlists p
             LEFT JOIN playlist_canciones c ON c.playlist = p.id
             GROUP BY p.id
             ORDER BY p.nube = ?1 DESC, p.creada DESC",
        )?;
        let filas = stmt.query_map(params![ME_GUSTA], |row| {
            let nube: String = row.get::<_, Option<String>>(3)?.unwrap_or_default();
            Ok(Playlist {
                id: row.get(0)?,
                nombre: row.get(1)?,
                url: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                me_gusta: nube == ME_GUSTA,
                nube,
                canciones: row.get(4)?,
            })
        })?;
        let mut salida = Vec::new();
        for p in filas {
            salida.push(p?);
        }
        Ok(salida)
    }

    /// Crea una playlist (o devuelve la existente si coincide `url`).
    pub fn crear_playlist(&self, nombre: &str, url: &str) -> Result<i64, ErrorBiblioteca> {
        if !url.is_empty()
            && let Some(id) = self
                .db
                .query_row("SELECT id FROM playlists WHERE url = ?1", params![url], |row| {
                    row.get::<_, i64>(0)
                })
                .optional()?
        {
            return Ok(id);
        }
        let ahora = ahora_epoch();
        let limpio = nombre.trim();
        let nombre_final = if limpio.is_empty() { "Playlist" } else { limpio };
        self.db.execute(
            "INSERT INTO playlists (nombre, url, creada, modificada) VALUES (?1,?2,?3,?4)",
            params![nombre_final, url, ahora, ahora],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    fn marcar_cambio(&self, pid: i64) -> Result<(), ErrorBiblioteca> {
        self.db.execute(
            "UPDATE playlists SET modificada = ?1 WHERE id = ?2",
            params![ahora_epoch(), pid],
        )?;
        Ok(())
    }

    /// Añade una canción al final de la playlist si no estaba ya presente.
    pub fn agregar_a_playlist(&self, pid: i64, ruta: &str) -> Result<(), ErrorBiblioteca> {
        let existe: Option<i64> = self
            .db
            .query_row(
                "SELECT 1 FROM playlist_canciones WHERE playlist = ?1 AND ruta = ?2",
                params![pid, ruta],
                |row| row.get(0),
            )
            .optional()?;
        if existe.is_some() {
            return Ok(());
        }
        let pos: i64 = self.db.query_row(
            "SELECT COALESCE(MAX(pos), -1) + 1 FROM playlist_canciones WHERE playlist = ?1",
            params![pid],
            |row| row.get(0),
        )?;
        self.db.execute(
            "INSERT INTO playlist_canciones VALUES (?1,?2,?3)",
            params![pid, pos, ruta],
        )?;
        self.marcar_cambio(pid)?;
        Ok(())
    }

    /// Quita una canción de la playlist.
    pub fn quitar_de_playlist(&self, pid: i64, ruta: &str) -> Result<(), ErrorBiblioteca> {
        let filas = self.db.execute(
            "DELETE FROM playlist_canciones WHERE playlist = ?1 AND ruta = ?2",
            params![pid, ruta],
        )?;
        if filas > 0 {
            self.marcar_cambio(pid)?;
        }
        Ok(())
    }

    /// Devuelve las canciones de una playlist en su orden.
    pub fn canciones_de_playlist(&self, pid: i64) -> Result<Vec<Cancion>, ErrorBiblioteca> {
        let mut stmt = self.db.prepare(
            "SELECT c.* FROM playlist_canciones p
             JOIN canciones c ON c.ruta = p.ruta
             WHERE p.playlist = ?1
             ORDER BY p.pos",
        )?;
        let filas = stmt.query_map(params![pid], Self::fila_a_cancion)?;
        let mut salida = Vec::new();
        for c in filas {
            salida.push(c?);
        }
        Ok(salida)
    }

    /// Borra una playlist (y opcionalmente anota su borrado para la nube).
    pub fn borrar_playlist(&self, pid: i64, anotar: bool) -> Result<(), ErrorBiblioteca> {
        let nube: Option<String> = self
            .db
            .query_row("SELECT nube FROM playlists WHERE id = ?1", params![pid], |row| {
                row.get(0)
            })
            .optional()?;
        if anotar
            && let Some(ref u) = nube
            && !u.is_empty()
        {
            self.db.execute(
                "INSERT OR REPLACE INTO borradas VALUES ('playlist', ?1, ?2)",
                params![u, ahora_epoch()],
            )?;
        }
        self.db
            .execute("DELETE FROM playlists WHERE id = ?1", params![pid])?;
        Ok(())
    }

    /// Registra una escucha para «Tu resumen».
    pub fn anotar_escucha(
        &self,
        cancion: &Cancion,
        segundos: f64,
        cuando: Option<f64>,
    ) -> Result<(), ErrorBiblioteca> {
        let titulo = if cancion.titulo.is_empty() {
            Path::new(&cancion.ruta)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            cancion.titulo.clone()
        };
        self.db.execute(
            "INSERT INTO escuchas VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                cuando.unwrap_or_else(ahora_epoch),
                cancion.ruta,
                titulo,
                cancion.artista,
                segundos,
            ],
        )?;
        Ok(())
    }

    /// Devuelve el ID de «Tus me gusta» (o `-1` si aún no existe y `crear == false`).
    pub fn id_me_gusta(&self, crear: bool) -> Result<i64, ErrorBiblioteca> {
        if let Some(id) = self
            .db
            .query_row(
                "SELECT id FROM playlists WHERE nube = ?1",
                params![ME_GUSTA],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        if !crear {
            return Ok(-1);
        }
        let ahora = ahora_epoch();
        self.db.execute(
            "INSERT INTO playlists (nombre, url, creada, modificada, nube) VALUES (?1,?2,?3,?4,?5)",
            params![NOMBRE_ME_GUSTA, "", ahora, ahora, ME_GUSTA],
        )?;
        Ok(self.db.last_insert_rowid())
    }

    /// Lista las rutas de «Tus me gusta» en su orden (más recientes primero).
    pub fn me_gusta(&self) -> Result<Vec<String>, ErrorBiblioteca> {
        let pid = self.id_me_gusta(false)?;
        if pid < 0 {
            return Ok(Vec::new());
        }
        let mut stmt = self
            .db
            .prepare("SELECT ruta FROM playlist_canciones WHERE playlist = ?1 ORDER BY pos")?;
        let filas = stmt.query_map(params![pid], |row| row.get::<_, String>(0))?;
        let mut salida = Vec::new();
        for r in filas {
            salida.push(r?);
        }
        Ok(salida)
    }

    /// Alterna una canción en «Tus me gusta». Devuelve `true` si ahora está marcada.
    pub fn alternar_me_gusta(&self, ruta: &str) -> Result<bool, ErrorBiblioteca> {
        let pid = self.id_me_gusta(true)?;
        let borradas = self.db.execute(
            "DELETE FROM playlist_canciones WHERE playlist = ?1 AND ruta = ?2",
            params![pid, ruta],
        )?;
        let gusta = if borradas > 0 {
            false
        } else {
            let pos: i64 = self.db.query_row(
                "SELECT COALESCE(MIN(pos), 1) - 1 FROM playlist_canciones WHERE playlist = ?1",
                params![pid],
                |row| row.get(0),
            )?;
            self.db.execute(
                "INSERT INTO playlist_canciones VALUES (?1,?2,?3)",
                params![pid, pos, ruta],
            )?;
            true
        };
        self.marcar_cambio(pid)?;
        Ok(gusta)
    }

    /// Anota una canción de YouTube borrada para propagar el borrado a la nube.
    pub fn anotar_cancion_borrada(&self, vid: &str) -> Result<(), ErrorBiblioteca> {
        if !vid.is_empty() {
            self.db.execute(
                "INSERT OR REPLACE INTO borradas VALUES ('cancion', ?1, ?2)",
                params![vid, ahora_epoch()],
            )?;
        }
        Ok(())
    }

    /// Mapa `video_id -> ruta` de todas las canciones de YouTube en la biblioteca.
    pub fn rutas_por_id(&self) -> Result<HashMap<String, String>, ErrorBiblioteca> {
        let mut stmt = self.db.prepare("SELECT ruta FROM canciones ORDER BY agregada")?;
        let filas = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut mapa = HashMap::new();
        for r in filas {
            let ruta = r?;
            let vid = id_de(&ruta);
            if !vid.is_empty() {
                mapa.insert(vid, ruta);
            }
        }
        Ok(mapa)
    }

    fn borradas_de(&self, tipo: &str) -> Result<std::collections::BTreeMap<String, i64>, ErrorBiblioteca> {
        let mut stmt = self
            .db
            .prepare("SELECT clave, cuando FROM borradas WHERE tipo = ?1")?;
        let filas = stmt.query_map(params![tipo], |row| {
            Ok((row.get::<_, String>(0)?, (row.get::<_, f64>(1)? * 1000.0) as i64))
        })?;
        let mut mapa = std::collections::BTreeMap::new();
        for f in filas {
            let (k, v) = f?;
            mapa.insert(k, v);
        }
        Ok(mapa)
    }

    /// Exporta el estado local en el formato esperado por `nube.rs` (asignando UUID si falta).
    pub fn estado_nube(&self) -> Result<nube::Local, ErrorBiblioteca> {
        let mut local = nube::Local {
            quitadas: self.borradas_de("cancion")?,
            borradas: self.borradas_de("playlist")?,
            ..Default::default()
        };

        {
            let mut stmt = self
                .db
                .prepare("SELECT ruta, titulo, artista, album, duracion, agregada FROM canciones")?;
            let filas = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                    row.get::<_, Option<f64>>(5)?.unwrap_or(0.0),
                ))
            })?;
            for f in filas {
                let (ruta, titulo, artista, album, duracion, agregada) = f?;
                let vid = id_de(&ruta);
                if !vid.is_empty() {
                    local.canciones.insert(
                        vid,
                        nube::CancionNube {
                            titulo,
                            artista,
                            album,
                            duracion: duracion as i64,
                            agregada: (agregada * 1000.0) as i64,
                        },
                    );
                }
            }
        }

        let playlists_raw: Vec<(i64, String, String, String, f64)> = {
            let mut stmt = self
                .db
                .prepare("SELECT id, nombre, url, nube, modificada FROM playlists")?;
            let filas = stmt.query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    row.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    row.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                ))
            })?;
            let mut out = Vec::new();
            for f in filas {
                out.push(f?);
            }
            out
        };

        for (pid, nombre, url, nube_actual, modificada) in playlists_raw {
            let u = if nube_actual.is_empty() {
                let nuevo = Uuid::new_v4().to_string();
                self.db.execute(
                    "UPDATE playlists SET nube = ?1 WHERE id = ?2",
                    params![nuevo, pid],
                )?;
                nuevo
            } else {
                nube_actual
            };
            let mut ids = Vec::new();
            let mut stmt = self
                .db
                .prepare("SELECT ruta FROM playlist_canciones WHERE playlist = ?1 ORDER BY pos")?;
            let filas = stmt.query_map(params![pid], |row| row.get::<_, String>(0))?;
            for r in filas {
                let vid = id_de(&r?);
                if !vid.is_empty() && !ids.contains(&vid) {
                    ids.push(vid);
                }
            }
            local.playlists.insert(
                u,
                nube::PlaylistNube {
                    nombre,
                    url,
                    canciones: ids,
                    modificada: (modificada * 1000.0) as i64,
                },
            );
        }

        Ok(local)
    }

    /// Aplica una playlist recibida de la nube conservando canciones locales al final.
    pub fn poner_playlist(
        &self,
        u: &str,
        p: &nube::PlaylistNube,
        ruta_de_id: &HashMap<String, String>,
    ) -> Result<(), ErrorBiblioteca> {
        let pid = if let Some(id) = self
            .db
            .query_row("SELECT id FROM playlists WHERE nube = ?1", params![u], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?
        {
            id
        } else {
            let nombre = if p.nombre.is_empty() {
                "Playlist"
            } else {
                &p.nombre
            };
            self.db.execute(
                "INSERT INTO playlists (nombre, url, creada, nube) VALUES (?1,?2,?3,?4)",
                params![nombre, p.url, ahora_epoch(), u],
            )?;
            self.db.last_insert_rowid()
        };

        let mut locales = Vec::new();
        {
            let mut stmt = self
                .db
                .prepare("SELECT ruta FROM playlist_canciones WHERE playlist = ?1 ORDER BY pos")?;
            let filas = stmt.query_map(params![pid], |row| row.get::<_, String>(0))?;
            for r in filas {
                let ruta = r?;
                if id_de(&ruta).is_empty() {
                    locales.push(ruta);
                }
            }
        }

        let mut rutas_nuevas = Vec::new();
        for vid in &p.canciones {
            if let Some(r) = ruta_de_id.get(vid)
                && !rutas_nuevas.contains(r)
            {
                rutas_nuevas.push(r.clone());
            }
        }
        for r in locales {
            if !rutas_nuevas.contains(&r) {
                rutas_nuevas.push(r);
            }
        }

        let nombre = if p.nombre.is_empty() {
            "Playlist"
        } else {
            &p.nombre
        };
        self.db.execute(
            "UPDATE playlists SET nombre = ?1, url = ?2, modificada = ?3 WHERE id = ?4",
            params![nombre, p.url, p.modificada as f64 / 1000.0, pid],
        )?;
        self.db
            .execute("DELETE FROM playlist_canciones WHERE playlist = ?1", params![pid])?;
        for (i, r) in rutas_nuevas.into_iter().enumerate() {
            self.db.execute(
                "INSERT INTO playlist_canciones VALUES (?1,?2,?3)",
                params![pid, i as i64, r],
            )?;
        }
        Ok(())
    }

    pub fn quitar_playlist_nube(&self, u: &str) -> Result<(), ErrorBiblioteca> {
        if let Some(pid) = self
            .db
            .query_row("SELECT id FROM playlists WHERE nube = ?1", params![u], |row| {
                row.get::<_, i64>(0)
            })
            .optional()?
        {
            self.borrar_playlist(pid, false)?;
        }
        Ok(())
    }
}

/// Lo que falta de la 1.7.3 de Python: «Tu resumen», lo más escuchado, «Agregar canciones» de una playlist
/// (1.7.2), renombrar y el mosaico de la portada.
impl Biblioteca {
    /// Tus canciones (las que siguen en tu biblioteca) que más sonaron desde `desde`.
    pub fn mas_escuchadas(&self, desde: f64, n: usize) -> Result<Vec<Cancion>, ErrorBiblioteca> {
        let rutas_: Vec<String> = {
            let mut st = self.db.prepare(
                "SELECT e.ruta, COUNT(*) AS veces FROM escuchas e JOIN canciones c ON c.ruta = e.ruta \
                 WHERE e.cuando >= ?1 GROUP BY e.ruta ORDER BY veces DESC, MAX(e.cuando) DESC LIMIT ?2",
            )?;
            st.query_map(params![desde, n as i64], |r| r.get(0))?
                .collect::<Result<_, _>>()?
        };
        let mut v = Vec::new();
        for r in rutas_ {
            if let Some(c) = self.cancion(&r)? {
                v.push(c);
            }
        }
        Ok(v)
    }

    /// Tus artistas de los últimos días; sin escuchas, los que más tienes en tu biblioteca.
    pub fn artistas_mas_escuchados(&self, desde: f64, n: usize) -> Result<Vec<String>, ErrorBiblioteca> {
        let leer = |sql: &str, con_desde: bool| -> Result<Vec<String>, ErrorBiblioteca> {
            let mut st = self.db.prepare(sql)?;
            let f = |r: &rusqlite::Row<'_>| r.get::<_, Option<String>>(0);
            let v: Vec<Option<String>> = if con_desde {
                st.query_map(params![desde], f)?.collect::<Result<_, _>>()?
            } else {
                st.query_map([], f)?.collect::<Result<_, _>>()?
            };
            Ok(v.into_iter().map(Option::unwrap_or_default).collect())
        };
        let mut filas = leer("SELECT artista FROM escuchas WHERE cuando >= ?1", true)?;
        if filas.is_empty() {
            filas = leer("SELECT artista FROM canciones", false)?;
        }
        // (clave, nombre, veces) en orden de aparición: el sort de Python es estable
        let mut cuenta: Vec<(String, String, usize)> = Vec::new();
        for f in filas {
            let a = letras::limpiar_artista(&f);
            if a.is_empty() {
                continue;
            }
            let k = a.to_lowercase();
            match cuenta.iter_mut().find(|(c, _, _)| *c == k) {
                Some(x) => x.2 += 1,
                None => cuenta.push((k, a, 1)),
            }
        }
        cuenta.sort_by_key(|c| std::cmp::Reverse(c.2));
        Ok(cuenta.into_iter().take(n).map(|(_, a, _)| a).collect())
    }

    /// Lo que escuchaste entre `desde` y `hasta`: minutos, cuántas, cuántos días y lo más escuchado.
    pub fn resumen(&self, desde: f64, hasta: f64, n: usize) -> Result<Value, ErrorBiblioteca> {
        type Fila = (f64, String, String, String, f64);
        let filas: Vec<Fila> = {
            let mut st = self.db.prepare(
                "SELECT cuando, ruta, titulo, artista, segundos FROM escuchas WHERE cuando >= ?1 AND cuando < ?2",
            )?;
            st.query_map(params![desde, hasta], |r| {
                Ok((
                    r.get::<_, Option<f64>>(0)?.unwrap_or(0.0),
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                    r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                    r.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                ))
            })?
            .collect::<Result<_, _>>()?
        };
        struct Cuenta {
            clave: String,
            datos: Value,
            veces: i64,
            minutos: f64,
        }
        fn sumar(lista: &mut Vec<Cuenta>, clave: String, datos: impl FnOnce() -> Value, s: f64) {
            match lista.iter_mut().find(|c| c.clave == clave) {
                Some(c) => {
                    c.veces += 1;
                    c.minutos += s / 60.0;
                }
                None => lista.push(Cuenta {
                    clave,
                    datos: datos(),
                    veces: 1,
                    minutos: s / 60.0,
                }),
            }
        }
        let mut canciones: Vec<Cuenta> = Vec::new();
        let mut artistas: Vec<Cuenta> = Vec::new();
        let mut dias = HashSet::new();
        let mut total = 0.0;
        for (cuando, ruta, titulo, artista, seg) in &filas {
            dias.insert(crate::fecha::dia_texto(*cuando));
            total += seg;
            let nombre = letras::limpiar_artista(artista); // «YOASOBI - Topic» es «YOASOBI»
            let mostrar = if nombre.is_empty() {
                artista.clone()
            } else {
                nombre.clone()
            };
            sumar(
                &mut canciones,
                format!("{}\u{0}{}", titulo.to_lowercase(), nombre.to_lowercase()),
                || json!({"titulo": titulo, "artista": mostrar, "ruta": ruta}),
                *seg,
            );
            if !nombre.is_empty() {
                sumar(
                    &mut artistas,
                    nombre.to_lowercase(),
                    || json!({"artista": nombre, "ruta": ruta}),
                    *seg,
                );
            }
        }
        let top = |mut l: Vec<Cuenta>| -> Vec<Value> {
            // (-veces, -minutos), estable como el sorted de Python
            l.sort_by(|a, b| b.veces.cmp(&a.veces).then(b.minutos.total_cmp(&a.minutos)));
            l.into_iter()
                .take(n)
                .map(|c| {
                    let mut d = c.datos;
                    d["veces"] = json!(c.veces);
                    d["minutos"] = json!(redondear(c.minutos));
                    d
                })
                .collect()
        };
        let distintas = canciones.len();
        Ok(json!({
            "desde": desde, "hasta": hasta, "minutos": redondear(total / 60.0), "escuchas": filas.len(),
            "distintas": distintas, "dias": dias.len(), "canciones": top(canciones), "artistas": top(artistas),
        }))
    }

    /// Canciones de tu biblioteca para agregar a una playlist, con `enPlaylist` si ya están: las que todavía
    /// no están, primero; entre ellas, las más nuevas.
    pub fn para_agregar(&self, pid: i64, buscar: &str, limite: usize) -> Result<Vec<Value>, ErrorBiblioteca> {
        let dentro: HashSet<String> = {
            let mut st = self
                .db
                .prepare("SELECT ruta FROM playlist_canciones WHERE playlist = ?1")?;
            st.query_map(params![pid], |r| r.get::<_, String>(0))?
                .collect::<Result<_, _>>()?
        };
        let mut v: Vec<Value> = self
            .canciones(buscar, Orden::Agregada)?
            .into_iter()
            .map(|c| {
                let en = dentro.contains(&c.ruta);
                let mut j = serde_json::to_value(c).unwrap_or_default();
                j["enPlaylist"] = json!(en);
                j
            })
            .collect();
        v.sort_by_key(|c| c["enPlaylist"].as_bool().unwrap_or(false)); // estable
        v.truncate(limite);
        Ok(v)
    }

    pub fn renombrar_playlist(&self, pid: i64, nombre: &str) -> Result<(), ErrorBiblioteca> {
        let nombre = if nombre.trim().is_empty() {
            "Playlist"
        } else {
            nombre.trim()
        };
        self.db.execute(
            "UPDATE playlists SET nombre = ?1, modificada = ?2 WHERE id = ?3",
            params![nombre, ahora_epoch(), pid],
        )?;
        Ok(())
    }

    /// Las rutas de las primeras canciones (para el mosaico de la portada).
    pub fn primeras_de_playlist(&self, pid: i64, n: usize) -> Result<Vec<String>, ErrorBiblioteca> {
        let mut st = self.db.prepare(
            "SELECT p.ruta FROM playlist_canciones p JOIN canciones c ON c.ruta = p.ruta WHERE p.playlist = ?1 \
             ORDER BY p.pos LIMIT ?2",
        )?;
        Ok(st
            .query_map(params![pid, n as i64], |r| r.get(0))?
            .collect::<Result<_, _>>()?)
    }

    /// Las rutas de todo lo que hay (para saber qué ids de YouTube ya tienes).
    pub fn todas_las_rutas(&self) -> Result<Vec<String>, ErrorBiblioteca> {
        let mut st = self.db.prepare("SELECT ruta FROM canciones")?;
        Ok(st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?)
    }
}

/// El `round()` de Python (al par más cercano en los empates).
pub fn redondear(x: f64) -> i64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && (r as i64) % 2 != 0 {
        (r - x.signum()) as i64
    } else {
        r as i64
    }
}
