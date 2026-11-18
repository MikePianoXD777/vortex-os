//! Análisis de URLs (YouTube / YouTube Music / Spotify), enlaces de playlist,
//! parser de Spotify embed, búsqueda de YouTube Music y armado de comandos `yt-dlp`.

use std::collections::HashSet;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

static RE_SPOTIFY_URI: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^spotify:(track|album|playlist):([A-Za-z0-9]+)$").unwrap());
static RE_SPOTIFY_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"/(track|album|playlist)/([A-Za-z0-9]+)").unwrap());
static RE_YT_BROWSE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/(?:browse|channel)/([\w-]+)").unwrap());
static RE_YT_SHORTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^/shorts/([\w-]+)").unwrap());
static RE_NEXT_DATA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?s)<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap());
static RE_PORTADA_W_H: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"=w\d+-h\d+").unwrap());

pub const POR_ENLACE: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TipoEnlace {
    Spotify { clase: String, id: String },
    Youtube { clase: String, id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PistaSpotify {
    pub titulo: String,
    pub artistas: String,
    pub duracion: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultadoYtm {
    pub id: String,
    pub url: String,
    pub titulo: String,
    pub artistas: String,
    pub album: String,
    pub duracion: i64,
    pub portada: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoLinea {
    Archivo { ruta: String },
    Progreso { porcentaje: f64, titulo: String },
    Aviso { texto: String },
}

fn extraer_query(query: &str, clave: &str) -> Option<String> {
    for par in query.split('&') {
        if let Some((k, v)) = par.split_once('=')
            && k == clave
            && !v.is_empty()
        {
            return Some(v.to_string());
        }
    }
    None
}

/// Clasifica una URL o URI (`spotify:...`, `open.spotify.com`, `youtu.be`, `youtube.com`).
pub fn tipo_de(url_raw: &str) -> Option<TipoEnlace> {
    let url = url_raw.trim();
    if let Some(cap) = RE_SPOTIFY_URI.captures(url) {
        return Some(TipoEnlace::Spotify {
            clase: cap[1].to_string(),
            id: cap[2].to_string(),
        });
    }

    let con_esquema = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    let (_, resto) = con_esquema.split_once("://")?;
    let (autoridad, path_query) = match resto.find('/') {
        Some(idx) => (&resto[..idx], &resto[idx..]),
        None => (resto, "/"),
    };
    let host = autoridad
        .split('@')
        .next_back()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .to_lowercase();
    let (path, query) = match path_query.split_once('?') {
        Some((p, q)) => (p, q.split('#').next().unwrap_or("")),
        None => (path_query.split('#').next().unwrap_or(""), ""),
    };

    if host == "open.spotify.com" {
        let cap = RE_SPOTIFY_URL.captures(path)?;
        return Some(TipoEnlace::Spotify {
            clase: cap[1].to_string(),
            id: cap[2].to_string(),
        });
    }
    if host == "youtu.be" {
        let id = path.trim_matches('/');
        if id.is_empty() {
            return None;
        }
        return Some(TipoEnlace::Youtube {
            clase: "cancion".into(),
            id: id.to_string(),
        });
    }
    if host == "youtube.com" || host.ends_with(".youtube.com") {
        if path == "/watch"
            && let Some(v) = extraer_query(query, "v")
        {
            return Some(TipoEnlace::Youtube {
                clase: "cancion".into(),
                id: v,
            });
        }
        if path == "/playlist"
            && let Some(list) = extraer_query(query, "list")
        {
            return Some(TipoEnlace::Youtube {
                clase: "playlist".into(),
                id: list,
            });
        }
        if path == "/watch_videos"
            && let Some(ids) = extraer_query(query, "video_ids")
        {
            return Some(TipoEnlace::Youtube {
                clase: "playlist".into(),
                id: ids,
            });
        }
        if let Some(cap) = RE_YT_BROWSE.captures(path) {
            return Some(TipoEnlace::Youtube {
                clase: "playlist".into(),
                id: cap[1].to_string(),
            });
        }
        if let Some(cap) = RE_YT_SHORTS.captures(path) {
            return Some(TipoEnlace::Youtube {
                clase: "cancion".into(),
                id: cap[1].to_string(),
            });
        }
    }
    None
}

fn pct_encode(s: &str, safe: &[u8]) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) || safe.contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Genera enlaces compartibles de YouTube (`watch_videos`) agrupados de 50 en 50 sin duplicados.
pub fn enlaces_playlist(nombre: &str, ids: &[&str]) -> Vec<String> {
    let mut vistos = HashSet::new();
    let limpios: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| !id.is_empty() && vistos.insert(*id))
        .collect();
    if limpios.is_empty() {
        return Vec::new();
    }
    let partes: Vec<&[&str]> = limpios.chunks(POR_ENLACE).collect();
    let total = partes.len();
    partes
        .into_iter()
        .enumerate()
        .map(|(i, parte)| {
            let titulo = if total == 1 {
                nombre.to_string()
            } else {
                format!("{nombre} ({}/{total})", i + 1)
            };
            let ids_param = parte.join(",");
            format!(
                "https://www.youtube.com/watch_videos?video_ids={}&title={}",
                pct_encode(&ids_param, b","),
                pct_encode(&titulo, b"")
            )
        })
        .collect()
}

/// Extrae `(nombre, pistas)` desde el HTML de `open.spotify.com/embed/...`.
pub fn leer_spotify(pagina: &str) -> Result<(String, Vec<PistaSpotify>), String> {
    let cap = RE_NEXT_DATA
        .captures(pagina)
        .ok_or_else(|| "Spotify cambió su página: no encontré la lista de canciones".to_string())?;
    let bruto = cap[1].trim();
    let datos: Value = serde_json::from_str(bruto).map_err(|e| e.to_string())?;
    let entidad = &datos["props"]["pageProps"]["state"]["data"]["entity"];
    let nombre = entidad
        .get("name")
        .or_else(|| entidad.get("title"))
        .and_then(Value::as_str)
        .unwrap_or("Spotify")
        .to_string();

    let mut pistas = Vec::new();
    if let Some(lista) = entidad.get("trackList").and_then(Value::as_array) {
        for t in lista {
            let titulo = t.get("title").and_then(Value::as_str).unwrap_or("").to_string();
            if titulo.is_empty() {
                continue;
            }
            let artistas = t
                .get("subtitle")
                .and_then(Value::as_str)
                .unwrap_or("")
                .replace('\u{00a0}', " ");
            let duracion_ms = t.get("duration").and_then(Value::as_i64).unwrap_or(0);
            pistas.push(PistaSpotify {
                titulo,
                artistas,
                duracion: (duracion_ms as f64 / 1000.0).round() as i64,
            });
        }
    }

    if pistas.is_empty() && entidad.get("type").and_then(Value::as_str) == Some("track") {
        let artistas = entidad
            .get("artists")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("name").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let duracion_ms = entidad.get("duration").and_then(Value::as_i64).unwrap_or(0);
        pistas.push(PistaSpotify {
            titulo: nombre.clone(),
            artistas,
            duracion: (duracion_ms as f64 / 1000.0).round() as i64,
        });
    }

    Ok((nombre, pistas))
}

pub fn busqueda_ytmusic(pista: &PistaSpotify) -> String {
    let principal = pista.artistas.split(',').next().unwrap_or("").trim();
    let q = format!("{principal} {}", pista.titulo).trim().to_string();
    format!("https://music.youtube.com/search?q={}#songs", pct_encode(&q, b""))
}

fn segundos(texto: &str) -> i64 {
    let partes: Vec<&str> = texto.trim().split(':').collect();
    if !(2..=3).contains(&partes.len())
        || !partes
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
    {
        return 0;
    }
    partes
        .iter()
        .fold(0i64, |acc, p| acc * 60 + p.parse::<i64>().unwrap_or(0))
}

fn buscar_objetos<'a>(val: &'a Value, clave: &str, out: &mut Vec<&'a Value>) {
    match val {
        Value::Object(map) => {
            for (k, v) in map {
                if k == clave {
                    out.push(v);
                }
                buscar_objetos(v, clave, out);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                buscar_objetos(v, clave, out);
            }
        }
        _ => {}
    }
}

/// Extrae canciones desde una respuesta JSON de búsqueda de YouTube Music.
pub fn leer_ytmusic(respuesta: &Value) -> Vec<ResultadoYtm> {
    let mut items = Vec::new();
    buscar_objetos(respuesta, "musicResponsiveListItemRenderer", &mut items);
    let mut canciones = Vec::new();

    for it in items {
        let Some(vid) = it
            .get("playlistItemData")
            .and_then(|d| d.get("videoId"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let cols: Vec<Vec<&str>> = it
            .get("flexColumns")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .map(|c| {
                        c.get("musicResponsiveListItemFlexColumnRenderer")
                            .and_then(|r| r.get("text"))
                            .and_then(|t| t.get("runs"))
                            .and_then(Value::as_array)
                            .map(|runs| {
                                runs.iter()
                                    .filter_map(|r| r.get("text").and_then(Value::as_str))
                                    .collect()
                            })
                            .unwrap_or_default()
                    })
                    .collect()
            })
            .unwrap_or_default();

        if cols.is_empty() || cols[0].is_empty() {
            continue;
        }
        let titulo = cols[0].concat();
        let segunda = if cols.len() > 1 {
            cols[1].concat()
        } else {
            String::new()
        };
        let mut datos: Vec<&str> = segunda
            .split('•')
            .map(str::trim)
            .filter(|x| !x.is_empty() && *x != "Canción" && *x != "Song")
            .collect();

        let duracion = datos.last().map(|s| segundos(s)).unwrap_or(0);
        if duracion > 0 {
            datos.pop();
        }
        let artistas = datos.first().copied().unwrap_or("").to_string();
        let album = if datos.len() >= 2 {
            datos[1].to_string()
        } else {
            String::new()
        };

        let portada_raw = it
            .get("thumbnail")
            .and_then(|t| t.get("musicThumbnailRenderer"))
            .and_then(|r| r.get("thumbnail"))
            .and_then(|t| t.get("thumbnails"))
            .and_then(Value::as_array)
            .and_then(|arr| arr.last())
            .and_then(|t| t.get("url"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let portada = RE_PORTADA_W_H.replace(portada_raw, "=w226-h226").to_string();

        canciones.push(ResultadoYtm {
            id: vid.to_string(),
            url: format!("https://music.youtube.com/watch?v={vid}"),
            titulo,
            artistas,
            album,
            duracion,
            portada,
        });
    }
    canciones
}

/// Arma la lista de argumentos de `yt-dlp` para bajar audio sin recomprimir.
pub fn comando(url: &str, destino: &Path, navegador: &str, una: bool, yt_dlp: &str) -> Vec<String> {
    let plantilla = destino
        .join("%(artist,uploader,channel)s/%(title)s [%(id)s].%(ext)s")
        .to_string_lossy()
        .to_string();
    let mut args = vec![
        yt_dlp.to_string(),
        "-x".into(),
        "-f".into(),
        "bestaudio/best".into(),
        "--embed-thumbnail".into(),
        "--add-metadata".into(),
        "--no-warnings".into(),
        "--newline".into(),
        "--progress-template".into(),
        "download:VORTEXP\t%(progress._percent_str)s\t%(info.title)s".into(),
        "--print".into(),
        "after_move:VORTEXA\t%(filepath)s".into(),
        "-o".into(),
        plantilla,
    ];
    if !navegador.is_empty() {
        args.push("--cookies-from-browser".into());
        args.push(navegador.to_string());
    }
    if url.contains("music.youtube.com/search") {
        args.push("--playlist-items".into());
        args.push("1".into());
    } else if una {
        args.push("--no-playlist".into());
    } else {
        args.push("--yes-playlist".into());
    }
    args.push("--".into());
    args.push(url.to_string());
    args
}

/// Interpreta una línea de salida de `yt-dlp`.
pub fn leer_linea(linea: &str) -> Option<EventoLinea> {
    let l = linea.trim_end_matches(['\r', '\n']);
    if let Some(ruta) = l.strip_prefix("VORTEXA\t") {
        return Some(EventoLinea::Archivo {
            ruta: ruta.trim().to_string(),
        });
    }
    if let Some(resto) = l.strip_prefix("VORTEXP\t") {
        let partes: Vec<&str> = resto.splitn(2, '\t').collect();
        let pct_str = partes.first().copied().unwrap_or("").trim().trim_end_matches('%');
        let porcentaje = pct_str.parse::<f64>().unwrap_or(0.0);
        let titulo = partes.get(1).copied().unwrap_or("").trim().to_string();
        return Some(EventoLinea::Progreso { porcentaje, titulo });
    }
    if l.contains("ERROR:") {
        let texto = if l.contains("Private video") {
            "Esa canción o lista es privada".to_string()
        } else {
            l.to_string()
        };
        return Some(EventoLinea::Aviso { texto });
    }
    None
}
