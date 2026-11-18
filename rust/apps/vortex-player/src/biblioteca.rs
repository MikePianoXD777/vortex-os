//! Biblioteca musical en SQLite, escaneo de carpetas, playlists y `ajustes.json`.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
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

/// Ajustes persistidos en `ajustes.json` (compatibles con la versión Python).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    pub carpetas: Vec<String>,
    pub navegador_premium: String,
    pub volumen: i64,
    pub ldac_hq: bool,
    pub fundido: f64,
    pub radio: bool,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            carpetas: Vec::new(),
            navegador_premium: String::new(),
            volumen: 80,
            ldac_hq: true,
            fundido: 5.0,
            radio: true,
        }
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

    /// Lee `ajustes.json` o devuelve los valores por defecto.
    pub fn ajustes() -> Ajustes {
        Self::ajustes_en(&rutas::config())
    }

    /// Lee `ajustes.json` desde una carpeta concreta.
    pub fn ajustes_en(dir: &Path) -> Ajustes {
        let ruta = dir.join("ajustes.json");
        let Ok(contenido) = fs::read_to_string(ruta) else {
            return Ajustes::default();
        };
        serde_json::from_str(&contenido).unwrap_or_default()
    }

    /// Guarda los ajustes completos de forma atómica en `ajustes.json`.
    pub fn guardar_ajustes(ajustes: &Ajustes) -> Result<(), ErrorBiblioteca> {
        Self::guardar_ajustes_en(&rutas::config(), ajustes)
    }

    /// Guarda los ajustes de forma atómica en una carpeta concreta.
    pub fn guardar_ajustes_en(dir: &Path, ajustes: &Ajustes) -> Result<(), ErrorBiblioteca> {
        fs::create_dir_all(dir)?;
        let tmp = dir.join("ajustes.json.tmp");
        let destino = dir.join("ajustes.json");
        let texto = serde_json::to_string_pretty(ajustes)?;
        fs::write(&tmp, texto)?;
        fs::rename(tmp, destino)?;
        Ok(())
    }

    /// Carpetas configuradas para escanear (`Música` + adicionales sin duplicados).
    pub fn carpetas(&self) -> Vec<PathBuf> {
        let mut vistas = Vec::new();
        let mut candidatos = vec![rutas::musica()];
        for c in Self::ajustes().carpetas {
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
