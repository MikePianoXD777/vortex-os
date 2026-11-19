//! Bajar canciones y playlists de YouTube, YouTube Music y Spotify (como `descargas.py`).
//!
//! - YouTube y YouTube Music: yt-dlp baja el mejor audio que haya y lo guarda tal cual (Opus o AAC), sin
//!   recomprimirlo. Deja la portada en un `.jpg` junto a la canción.
//! - Spotify no deja bajar su audio: de su página pública («embed», sin cuenta) se leen los nombres y cada
//!   canción se busca y se baja de YouTube Music.
//! - Con YouTube Music Premium (ajuste `navegador_premium`) yt-dlp usa la sesión de tu navegador.
//!
//! Los resultados y los eventos son JSON, como los `dict` de la de Python (así llegan tal cual a los QML):
//! `descargar` llama a `emitir` con `{"tipo": "inicio" | "cancion" | "progreso" | "archivo" | "aviso" | "fin", …}`.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Value, json};

use crate::biblioteca::redondear;
use crate::letras;
use crate::red::{self, ErrorRed, Peticion};

pub const AGENTE: &str = "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0";
pub const MARCA: &str = "VORTEX";
/// YouTube no toma más de 50 en una playlist sin cuenta.
pub const POR_ENLACE: usize = 50;
pub const YTM_SOLO_CANCIONES: &str = "EgWKAQIIAWoQEAMQBBAJEAoQBRAREBUQFQ%3D%3D"; // el filtro «Canciones»
const YTM_ARTISTAS: &str = "EgWKAQIgAWoKEAkQBRAKEAMQBA%3D%3D";

fn re(p: &str) -> Regex {
    Regex::new(p).expect("regex fija")
}

static RE_SPOTIFY_URI: LazyLock<Regex> =
    LazyLock::new(|| re(r"^spotify:(track|album|playlist):([A-Za-z0-9]+)$"));
static RE_SPOTIFY_URL: LazyLock<Regex> = LazyLock::new(|| re(r"/(track|album|playlist)/([A-Za-z0-9]+)"));
static RE_BROWSE: LazyLock<Regex> = LazyLock::new(|| re(r"^/(?:browse|channel)/([\w-]+)"));
static RE_SHORTS: LazyLock<Regex> = LazyLock::new(|| re(r"^/shorts/([\w-]+)"));
static RE_NEXT_DATA: LazyLock<Regex> =
    LazyLock::new(|| re(r#"(?s)<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#));
static RE_TAMANO: LazyLock<Regex> = LazyLock::new(|| re(r"=w\d+-h\d+"));
static RE_DURACION: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{1,2}(:\d{2}){1,2}$"));
static RE_ID: LazyLock<Regex> = LazyLock::new(|| re(r"^[\w-]{11}$"));
static RE_ANIO4: LazyLock<Regex> = LazyLock::new(|| re(r"^\d{4}$"));
static RE_ANIO: LazyLock<Regex> = LazyLock::new(|| re(r"\b(19|20)\d\d\b"));
static RE_SINGLE: LazyLock<Regex> = LazyLock::new(|| re(r"Single|Sencillo|EP"));
static RE_OLAK: LazyLock<Regex> = LazyLock::new(|| re(r#""playlistId":\s*"(OLAK[\w-]+)""#));
static RE_CLAVE_T: LazyLock<Regex> = LazyLock::new(|| re(r"[\(\[].*?[\)\]]|\s-\s.*$"));
static RE_NO_PALABRA: LazyLock<Regex> = LazyLock::new(|| re(r"\W+"));

// --- enlaces -----------------------------------------------------------------------------------------

/// `%XX` → bytes (y `+` → espacio si `mas`), como `unquote`/`parse_qs`.
pub(crate) fn desescapar(s: &str, mas: bool) -> String {
    let b = s.as_bytes();
    let mut v = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("zz"), 16) {
                    Ok(x) => {
                        v.push(x);
                        i += 3;
                        continue;
                    }
                    Err(_) => v.push(b'%'),
                }
            }
            b'+' if mas => v.push(b' '),
            c => v.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&v).into_owned()
}

/// (host en minúsculas, ruta, consulta) de una URL, como `urlparse`.
fn partes(url: &str) -> (String, String, String) {
    let sin_esquema = url.split_once("://").map_or(url, |(_, r)| r);
    let sin_frag = sin_esquema.split('#').next().unwrap_or("");
    let (antes, consulta) = sin_frag.split_once('?').unwrap_or((sin_frag, ""));
    let (host, ruta) = match antes.find('/') {
        Some(i) => (&antes[..i], &antes[i..]),
        None => (antes, ""),
    };
    let host = host.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']').next().unwrap_or("").trim_start_matches('[')
    } else {
        host.split(':').next().unwrap_or("")
    };
    (host.to_lowercase(), ruta.to_string(), consulta.to_string())
}

/// El primer valor de `clave` en la consulta (`parse_qs(q)[clave][0]`; los vacíos no cuentan).
fn valor_consulta(consulta: &str, clave: &str) -> Option<String> {
    consulta
        .split(['&', ';'])
        .filter_map(|par| par.split_once('='))
        .find(|(k, v)| desescapar(k, true) == clave && !v.is_empty())
        .map(|(_, v)| desescapar(v, true))
}

/// («spotify» | «youtube», clase, id), o `None` si no es un enlace que se pueda bajar.
pub fn tipo_de(url: &str) -> Option<(String, String, String)> {
    let url = url.trim();
    let t = |a: &str, b: &str, c: &str| Some((a.to_string(), b.to_string(), c.to_string()));
    if let Some(m) = RE_SPOTIFY_URI.captures(url) {
        return t("spotify", &m[1], &m[2]);
    }
    let completa = if url.contains("://") {
        url.to_string()
    } else {
        format!("https://{url}")
    };
    let (host, ruta, q) = partes(&completa);
    if host == "open.spotify.com" {
        let m = RE_SPOTIFY_URL.captures(&ruta)?;
        return t("spotify", &m[1], &m[2]);
    }
    if host == "youtu.be" {
        return t("youtube", "cancion", ruta.trim_matches('/'));
    }
    if host.ends_with("youtube.com") {
        if ruta == "/watch"
            && let Some(v) = valor_consulta(&q, "v")
        {
            return t("youtube", "cancion", &v);
        }
        if ruta == "/playlist"
            && let Some(v) = valor_consulta(&q, "list")
        {
            return t("youtube", "playlist", &v);
        }
        if ruta == "/watch_videos"
            && let Some(v) = valor_consulta(&q, "video_ids")
        {
            return t("youtube", "playlist", &v);
        }
        if let Some(m) = RE_BROWSE.captures(&ruta) {
            return t("youtube", "playlist", &m[1]);
        }
        if let Some(m) = RE_SHORTS.captures(&ruta) {
            return t("youtube", "cancion", &m[1]);
        }
    }
    None
}

/// `quote(texto, safe=seguros)`: letras, números, `_.-~` y los `seguros` quedan; lo demás va como `%XX`.
fn citar(texto: &str, seguros: &[u8]) -> String {
    let mut r = String::with_capacity(texto.len());
    for b in texto.bytes() {
        if b.is_ascii_alphanumeric() || b"_.-~".contains(&b) || seguros.contains(&b) {
            r.push(b as char);
        } else {
            r.push_str(&format!("%{b:02X}"));
        }
    }
    r
}

/// La playlist como enlace(s) de YouTube que abre cualquiera, sin cuenta (de 50 en 50).
pub fn enlaces_playlist(nombre: &str, ids: &[String]) -> Vec<String> {
    let mut vistos = HashSet::new();
    let ids: Vec<&str> = ids
        .iter()
        .map(String::as_str)
        .filter(|i| !i.is_empty() && vistos.insert(*i))
        .collect();
    let partes: Vec<&[&str]> = ids.chunks(POR_ENLACE).collect();
    let total = partes.len();
    partes
        .iter()
        .enumerate()
        .map(|(n, parte)| {
            let titulo = if total == 1 {
                nombre.to_string()
            } else {
                format!("{nombre} ({}/{total})", n + 1)
            };
            format!(
                "https://www.youtube.com/watch_videos?video_ids={}&title={}",
                citar(&parte.join(","), b","),
                citar(&titulo, b",")
            )
        })
        .collect()
}

// --- Spotify -------------------------------------------------------------------------------------------

/// `html.unescape` de lo poco que trae la página de Spotify.
fn desescapar_html(s: &str) -> String {
    let mut r = String::with_capacity(s.len());
    let mut resto = s;
    while let Some(i) = resto.find('&') {
        r.push_str(&resto[..i]);
        resto = &resto[i..];
        let Some(fin) = resto[..resto.len().min(12)].find(';') else {
            r.push('&');
            resto = &resto[1..];
            continue;
        };
        let ent = &resto[1..fin];
        let c = match ent {
            "quot" => Some('"'),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
            }
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match c {
            Some(c) => {
                r.push(c);
                resto = &resto[fin + 1..];
            }
            None => {
                r.push('&');
                resto = &resto[1..];
            }
        }
    }
    r.push_str(resto);
    r
}

fn texto_de(v: &Value, clave: &str) -> String {
    v.get(clave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// (nombre, [{titulo, artistas, duracion}]) desde la página «embed» de Spotify.
pub fn leer_spotify(pagina: &str) -> Result<(String, Vec<Value>), String> {
    let m = RE_NEXT_DATA
        .captures(pagina)
        .ok_or("Spotify cambió su página: no encontré la lista de canciones")?;
    let bruto = &m[1];
    let texto = if bruto.trim_start().starts_with('&') {
        desescapar_html(bruto)
    } else {
        bruto.to_string()
    };
    let datos: Value = serde_json::from_str(&texto).map_err(|e| e.to_string())?;
    let entidad = datos
        .pointer("/props/pageProps/state/data/entity")
        .filter(|e| e.is_object())
        .ok_or("'entity'")?;
    let nombre = [texto_de(entidad, "name"), texto_de(entidad, "title")]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_else(|| "Spotify".into());
    let segundos = |v: &Value| redondear(v.get("duration").and_then(Value::as_f64).unwrap_or(0.0) / 1000.0);
    let mut pistas: Vec<Value> = entidad
        .get("trackList")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|t| {
            json!({"titulo": texto_de(t, "title"), "artistas": texto_de(t, "subtitle").replace('\u{a0}', " "),
                   "duracion": segundos(t)})
        })
        .collect();
    if pistas.is_empty() && entidad.get("type").and_then(Value::as_str) == Some("track") {
        let artistas: Vec<String> = entidad
            .get("artists")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|a| texto_de(a, "name"))
            .collect();
        pistas
            .push(json!({"titulo": nombre, "artistas": artistas.join(", "), "duracion": segundos(entidad)}));
    }
    pistas.retain(|p| !p["titulo"].as_str().unwrap_or_default().is_empty());
    Ok((nombre, pistas))
}

/// Baja una página como texto (la de Spotify).
pub fn bajar_texto(url: &str) -> Result<String, String> {
    red::pedir(
        &Peticion::get(url)
            .cabecera(format!("User-Agent: {AGENTE}"))
            .cabecera("Accept-Language: es-MX,es;q=0.9")
            .espera(20),
    )
    .map(|d| String::from_utf8_lossy(&d).into_owned())
    .map_err(|e| e.to_string())
}

/// La búsqueda de YouTube Music (sección «Canciones») que encuentra esa canción.
pub fn busqueda_ytmusic(pista: &Value) -> String {
    let artistas = texto_de(pista, "artistas");
    let principal = artistas.split(',').next().unwrap_or("").trim();
    let q = format!("{principal} {}", texto_de(pista, "titulo"));
    format!(
        "https://music.youtube.com/search?q={}#songs",
        citar(q.trim(), b"/")
    )
}

// --- buscar, radio, novedades (la API web de YouTube Music, sin llave ni cuenta) ----------------------------

/// Por qué falló una consulta a YouTube Music: sin conexión (se le dice a la persona) o una respuesta que no
/// se entiende (se cae a otra forma, como el `ValueError` de la de Python).
#[derive(Debug, Clone, PartialEq)]
pub enum Fallo {
    Red(String),
    Rara,
}

/// Le pide algo a YouTube Music (url, cuerpo JSON). Las pruebas le pasan uno falso.
pub type Pedir<'a> = &'a dyn Fn(&str, &Value) -> Result<Value, Fallo>;

pub fn cliente() -> Value {
    json!({"clientName": "WEB_REMIX", "clientVersion": "1.20251020.01.00", "hl": "es", "gl": "MX"})
}

/// El `pedir` de verdad.
pub fn pedir_json(url: &str, cuerpo: &Value) -> Result<Value, Fallo> {
    let p = Peticion::post(url, cuerpo.to_string().into_bytes())
        .cabecera("Content-Type: application/json")
        .cabecera(format!("User-Agent: {AGENTE}"))
        .cabecera("Origin: https://music.youtube.com")
        .cabecera("Referer: https://music.youtube.com/")
        .espera(20);
    match red::json(&p) {
        Ok(v) => Ok(v),
        Err(ErrorRed::Rara(_)) => Err(Fallo::Rara),
        Err(ErrorRed::SinConexion(m)) => Err(Fallo::Red(m)),
        Err(ErrorRed::Http(c, _)) => Err(Fallo::Red(format!("YouTube contestó {c}"))),
    }
}

/// «3:34» o «1:02:03» → segundos (0 si no es una duración).
pub fn segundos(texto: &str) -> i64 {
    let t = texto.trim();
    if !RE_DURACION.is_match(t) {
        return 0;
    }
    t.split(':')
        .fold(0, |s, p| s * 60 + p.parse::<i64>().unwrap_or(0))
}

/// Todos los valores con esa clave, a cualquier profundidad y en el orden de la de Python.
pub fn todos<'a>(o: &'a Value, clave: &str) -> Vec<&'a Value> {
    fn ir<'a>(o: &'a Value, clave: &str, v: &mut Vec<&'a Value>) {
        match o {
            Value::Object(m) => {
                for (k, x) in m {
                    if k == clave {
                        v.push(x);
                    }
                    ir(x, clave, v);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| ir(x, clave, v)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    ir(o, clave, &mut v);
    v
}

/// El texto de un `{"runs": [{"text": …}]}`.
fn runs(x: Option<&Value>) -> String {
    x.and_then(|x| x.get("runs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|r| texto_de(r, "text"))
        .collect()
}

fn ultima_foto(fotos: Option<&Value>, tamano: &str) -> String {
    fotos
        .and_then(Value::as_array)
        .and_then(|f| f.last())
        .map(|f| RE_TAMANO.replace(&texto_de(f, "url"), tamano).into_owned())
        .unwrap_or_default()
}

/// Las canciones de una respuesta de búsqueda de YouTube Music.
pub fn leer_ytmusic(respuesta: &Value) -> Vec<Value> {
    let mut canciones = Vec::new();
    for it in todos(respuesta, "musicResponsiveListItemRenderer") {
        let vid = it
            .pointer("/playlistItemData/videoId")
            .and_then(Value::as_str)
            .unwrap_or("");
        let columnas: Vec<Vec<String>> = it
            .get("flexColumns")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .map(|c| {
                c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .map(|r| texto_de(r, "text"))
                    .collect()
            })
            .collect();
        if vid.is_empty() || columnas.is_empty() || columnas[0].is_empty() {
            continue;
        }
        // La segunda columna: «Artista • Álbum • 3:34» (con «Canción •» delante si no va filtrada)
        let segunda = columnas.get(1).map(|c| c.concat()).unwrap_or_default();
        let mut datos: Vec<String> = segunda
            .split('•')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty() && x != "Canción" && x != "Song")
            .collect();
        let duracion = datos.last().map_or(0, |d| segundos(d));
        if duracion != 0 {
            datos.pop();
        }
        let portada = ultima_foto(
            it.pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails"),
            "=w226-h226",
        );
        canciones.push(json!({
            "id": vid, "url": format!("https://music.youtube.com/watch?v={vid}"), "titulo": columnas[0].concat(),
            "artistas": datos.first().cloned().unwrap_or_default(), "album": datos.get(1).cloned().unwrap_or_default(),
            "duracion": duracion, "portada": portada,
        }));
    }
    canciones
}

/// Los resultados de `yt-dlp --flat-playlist -J` de una búsqueda.
pub fn leer_ytdlp(salida: &str, musica: bool) -> Vec<Value> {
    let texto = if salida.is_empty() { "{}" } else { salida };
    let Ok(v) = serde_json::from_str::<Value>(texto) else {
        return Vec::new();
    };
    v.get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let vid = texto_de(e, "id");
            if !RE_ID.is_match(&vid) {
                return None; // canales y playlists que se cuelan en la búsqueda
            }
            let artistas = [texto_de(e, "channel"), texto_de(e, "uploader")]
                .into_iter()
                .find(|s| !s.is_empty())
                .unwrap_or_default();
            Some(json!({
                "id": vid,
                "url": if musica { format!("https://music.youtube.com/watch?v={vid}") } else { format!("https://www.youtube.com/watch?v={vid}") },
                "titulo": texto_de(e, "title"), "artistas": artistas, "album": "",
                "duracion": e.get("duration").and_then(Value::as_f64).unwrap_or(0.0) as i64,
                "portada": format!("https://i.ytimg.com/vi/{vid}/mqdefault.jpg"),
            }))
        })
        .collect()
}

/// Busca canciones en YouTube Music («ytmusic») o videos en YouTube («youtube»). `Err` sin conexión.
pub fn buscar(
    texto: &str,
    fuente: &str,
    n: usize,
    yt_dlp: &str,
    pedir: Pedir<'_>,
) -> Result<Vec<Value>, String> {
    let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if texto.is_empty() {
        return Ok(Vec::new());
    }
    let objetivo = if fuente == "ytmusic" {
        match pedir(
            "https://music.youtube.com/youtubei/v1/search?prettyPrint=false",
            &json!({"context": {"client": cliente()}, "query": texto, "params": desescapar(YTM_SOLO_CANCIONES, false)}),
        ) {
            Ok(r) => {
                let c = leer_ytmusic(&r);
                if !c.is_empty() {
                    return Ok(c.into_iter().take(n).collect());
                }
            }
            Err(Fallo::Rara) => {}
            Err(Fallo::Red(m)) => return Err(m),
        }
        format!("https://music.youtube.com/search?q={}#songs", citar(&texto, b"/"))
    } else {
        format!("ytsearch{n}:{texto}")
    };
    let salida = match Command::new(yt_dlp)
        .args([
            "--flat-playlist",
            "-J",
            "--no-warnings",
            "--playlist-items",
            &format!("1:{n}"),
            "--",
            &objetivo,
        ])
        .stdin(Stdio::null())
        .output()
    {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("Falta yt-dlp: sudo pacman -S yt-dlp".into());
        }
        Err(e) => return Err(e.to_string()),
    };
    let out = String::from_utf8_lossy(&salida.stdout);
    if !salida.status.success() && out.is_empty() {
        let err = String::from_utf8_lossy(&salida.stderr);
        let ultima = err
            .trim()
            .lines()
            .last()
            .unwrap_or("Sin conexión con YouTube")
            .trim_start_matches("ERROR:")
            .trim()
            .to_string();
        return Err(error_claro(&ultima));
    }
    Ok(leer_ytdlp(&out, fuente == "ytmusic"))
}

/// La «radio» de YouTube Music de una canción (lo que su página pone en «A continuación»).
pub fn leer_radio(respuesta: &Value) -> Vec<Value> {
    let mut canciones = Vec::new();
    for it in todos(respuesta, "playlistPanelVideoRenderer") {
        let vid = texto_de(it, "videoId");
        let titulo = runs(it.get("title"));
        if vid.is_empty() || titulo.is_empty() {
            continue;
        }
        // «Artista • Álbum • 2023»
        let datos: Vec<String> = runs(it.get("longBylineText"))
            .split('•')
            .map(|x| x.trim().to_string())
            .filter(|x| !x.is_empty() && !RE_ANIO4.is_match(x))
            .collect();
        canciones.push(json!({
            "id": vid, "url": format!("https://music.youtube.com/watch?v={vid}"), "titulo": titulo,
            "artistas": datos.first().cloned().unwrap_or_default(), "album": datos.get(1).cloned().unwrap_or_default(),
            "duracion": segundos(&runs(it.get("lengthText"))),
            "portada": ultima_foto(it.pointer("/thumbnail/thumbnails"), "=w226-h226"),
        }));
    }
    canciones
}

/// Las canciones parecidas a esta (sin ella misma). `Err` sin conexión.
pub fn radio(video_id: &str, pedir: Pedir<'_>) -> Result<Vec<Value>, String> {
    match pedir(
        "https://music.youtube.com/youtubei/v1/next?prettyPrint=false",
        &json!({"context": {"client": cliente()}, "videoId": video_id, "playlistId": format!("RDAMVM{video_id}"),
                "isAudioOnly": true}),
    ) {
        Ok(r) => Ok(leer_radio(&r)
            .into_iter()
            .filter(|c| c["id"] != video_id)
            .collect()),
        Err(Fallo::Rara) => Ok(Vec::new()),
        Err(Fallo::Red(m)) => Err(m),
    }
}

/// El id del canal del artista en YouTube Music («UC…»), o «» si no lo encuentra.
pub fn artista_id(nombre: &str, pedir: Pedir<'_>) -> Result<String, String> {
    let r = match pedir(
        "https://music.youtube.com/youtubei/v1/search?prettyPrint=false",
        &json!({"context": {"client": cliente()}, "query": nombre, "params": YTM_ARTISTAS}),
    ) {
        Ok(r) => r,
        Err(Fallo::Rara) => return Ok(String::new()),
        Err(Fallo::Red(m)) => return Err(m),
    };
    Ok(todos(&r, "musicResponsiveListItemRenderer")
        .into_iter()
        .map(|it| {
            it.pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        })
        .find(|b| b.starts_with("UC"))
        .unwrap_or_default())
}

/// Los álbumes, singles y EP de un artista, los más nuevos primero: [{id, titulo, artista, tipo, anio, portada}].
pub fn lanzamientos(canal: &str, artista: &str, pedir: Pedir<'_>) -> Result<Vec<Value>, String> {
    let r = match pedir(
        "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false",
        &json!({"context": {"client": cliente()}, "browseId": canal}),
    ) {
        Ok(r) => r,
        Err(Fallo::Rara) => return Ok(Vec::new()),
        Err(Fallo::Red(m)) => return Err(m),
    };
    let mut salida = Vec::new();
    for estante in todos(&r, "musicCarouselShelfRenderer") {
        for x in estante
            .get("contents")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(it) = x.get("musicTwoRowItemRenderer") else {
                continue;
            };
            let bid = it
                .pointer("/navigationEndpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
                .unwrap_or("");
            if !bid.starts_with("MPREb") {
                continue;
            }
            let sub = runs(it.get("subtitle"));
            let anio: i64 = RE_ANIO
                .find(&sub)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0);
            let tipo = if !RE_SINGLE.is_match(&sub) {
                "Álbum"
            } else if sub.contains("EP") {
                "EP"
            } else {
                "Single"
            };
            salida.push(json!({
                "id": bid, "titulo": runs(it.get("title")), "artista": artista, "tipo": tipo, "anio": anio,
                "portada": ultima_foto(it.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"), "=w320-h320"),
            }));
        }
    }
    // estable: dentro del año, como los ordena YouTube Music
    salida.sort_by_key(|c| std::cmp::Reverse(c["anio"].as_i64().unwrap_or(0)));
    Ok(salida)
}

/// (id de su playlist «OLAK…» para bajarlo entero, [{id, titulo}]) de un álbum o single («MPREb…»).
pub fn canciones_de_lanzamiento(album: &str, pedir: Pedir<'_>) -> Result<(String, Vec<Value>), String> {
    let r = match pedir(
        "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false",
        &json!({"context": {"client": cliente()}, "browseId": album}),
    ) {
        Ok(r) => r,
        Err(Fallo::Rara) => return Ok((String::new(), Vec::new())),
        Err(Fallo::Red(m)) => return Err(m),
    };
    let lista = RE_OLAK
        .captures(&r.to_string())
        .map(|m| m[1].to_string())
        .unwrap_or_default();
    let canciones = todos(&r, "musicResponsiveListItemRenderer")
        .into_iter()
        .filter_map(|it| {
            let vid = it.pointer("/playlistItemData/videoId").and_then(Value::as_str)?;
            let col = it.get("flexColumns").and_then(Value::as_array)?.first()?;
            Some(json!({"id": vid, "titulo": runs(col.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))}))
        })
        .collect();
    Ok((lista, canciones))
}

/// La letra que muestra YouTube Music (pestaña «Letra», de Musixmatch), sin tiempos. `Ok(None)` si no tiene.
pub fn letra_youtube(video_id: &str, pedir: Pedir<'_>) -> Result<Option<String>, String> {
    let ctx = json!({"client": cliente()});
    let n = match pedir(
        "https://music.youtube.com/youtubei/v1/next?prettyPrint=false",
        &json!({"context": ctx, "videoId": video_id, "isAudioOnly": true}),
    ) {
        Ok(n) => n,
        Err(Fallo::Rara) => return Ok(None),
        Err(Fallo::Red(m)) => return Err(m),
    };
    let Some(bid) = todos(&n, "tabRenderer")
        .into_iter()
        .filter_map(|t| {
            t.pointer("/endpoint/browseEndpoint/browseId")
                .and_then(Value::as_str)
        })
        .find(|i| i.starts_with("MPLY"))
    else {
        return Ok(None);
    };
    let b = match pedir(
        "https://music.youtube.com/youtubei/v1/browse?prettyPrint=false",
        &json!({"context": ctx, "browseId": bid}),
    ) {
        Ok(b) => b,
        Err(Fallo::Rara) => return Ok(None),
        Err(Fallo::Red(m)) => return Err(m),
    };
    Ok(todos(&b, "musicDescriptionShelfRenderer")
        .into_iter()
        .map(|d| runs(d.get("description")))
        .find(|t| !t.trim().is_empty()))
}

/// Para reconocer la misma canción aunque venga de otro lado: «idol (from oshi no ko)» ≈ «idol».
pub fn clave(titulo: &str, artista: &str) -> String {
    let t = RE_CLAVE_T
        .replace_all(&letras::titulo_limpio(titulo, artista).to_lowercase(), "")
        .into_owned();
    let a = letras::limpiar_artista(artista).to_lowercase();
    format!(
        "{}|{}",
        RE_NO_PALABRA.replace_all(&t, ""),
        RE_NO_PALABRA.replace_all(&a, "")
    )
}

/// Recomendaciones para unas canciones de semilla ({id?, titulo, artista}): las radios de cada una,
/// intercaladas, sin repetir y sin lo que ya tienes. Las semillas sin id se buscan primero en YouTube Music.
pub fn recomendar(
    semillas: &[Value],
    tienes: &[Value],
    n: usize,
    pedir: Pedir<'_>,
) -> Result<Vec<Value>, String> {
    let mut ids: Vec<String> = Vec::new();
    for s in semillas {
        let mut vid = texto_de(s, "id");
        if vid.is_empty() {
            let q = format!("{} {}", texto_de(s, "artista"), texto_de(s, "titulo"));
            vid = buscar(&q, "ytmusic", 1, "yt-dlp", pedir)
                .ok()
                .and_then(|e| e.first().map(|c| texto_de(c, "id")))
                .unwrap_or_default();
        }
        if !vid.is_empty() && !ids.contains(&vid) {
            ids.push(vid);
        }
    }
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut radios = Vec::new();
    let mut errores = Vec::new();
    for vid in &ids {
        match radio(vid, pedir) {
            Ok(r) => radios.push(r),
            Err(e) => errores.push(e),
        }
    }
    if radios.is_empty() {
        return Err(errores
            .into_iter()
            .next()
            .unwrap_or_else(|| "Sin conexión con YouTube".into()));
    }
    let ya: HashSet<String> = tienes
        .iter()
        .map(|t| texto_de(t, "id"))
        .filter(|i| !i.is_empty())
        .chain(ids.iter().cloned())
        .collect();
    let ya_claves: HashSet<String> = tienes
        .iter()
        .map(|t| clave(&texto_de(t, "titulo"), &texto_de(t, "artista")))
        .collect();
    let mut salida = Vec::new();
    let mut vistas: HashSet<String> = HashSet::new();
    let filas = radios.iter().map(Vec::len).max().unwrap_or(0);
    for fila in 0..filas {
        for r in &radios {
            let Some(c) = r.get(fila) else { continue };
            let id = texto_de(c, "id");
            let k = clave(&texto_de(c, "titulo"), &texto_de(c, "artistas"));
            if ya.contains(&id) || ya_claves.contains(&k) || vistas.contains(&id) || vistas.contains(&k) {
                continue;
            }
            vistas.insert(id);
            vistas.insert(k);
            salida.push(c.clone());
            if salida.len() >= n {
                return Ok(salida);
            }
        }
    }
    Ok(salida)
}

// --- yt-dlp ---------------------------------------------------------------------------------------------

/// La línea de yt-dlp que baja el audio de `objetivo` a `destino` sin recomprimirlo.
pub fn comando(objetivo: &str, destino: &Path, navegador: &str, una: bool, yt_dlp: &str) -> Vec<String> {
    let plantilla = destino
        .join("%(artist,creator,uploader|Desconocido)s")
        .join("%(track,title)s [%(id)s].%(ext)s")
        .to_string_lossy()
        .into_owned();
    let mut c: Vec<String> = [
        yt_dlp,
        "--newline",
        "--no-colors",
        "--ignore-errors",
        "--no-mtime",
        "-f",
        "bestaudio/best",
        "-x", // -x sin --audio-format: el mismo códec, solo cambia el envase
        "--embed-metadata",
        "--write-thumbnail",
        "--convert-thumbnails",
        "jpg",
        "--parse-metadata",
        "%(track,title)s:%(meta_title)s",
        "--parse-metadata",
        "%(artist,creator,uploader)s:%(meta_artist)s",
        "--parse-metadata",
        "%(album|)s:%(meta_album)s",
        "--parse-metadata",
        "%(webpage_url)s:%(meta_purl)s",
        "-o",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    c.push(plantilla.clone());
    c.push("-o".into());
    c.push(format!("thumbnail:{plantilla}"));
    c.push("--print".into());
    c.push(format!("after_move:{MARCA}A\t%(filepath)s"));
    c.push("--progress-template".into());
    c.push(format!(
        "download:{MARCA}P\t%(progress._percent_str)s\t%(info.track,info.title)s"
    ));
    if !navegador.is_empty() {
        c.push("--cookies-from-browser".into());
        c.push(navegador.into());
    }
    c.push(if una { "--no-playlist" } else { "--yes-playlist" }.into());
    if objetivo.starts_with("https://music.youtube.com/search") {
        c.push("--playlist-items".into());
        c.push("1".into());
    }
    c.push("--".into());
    c.push(objetivo.into());
    c
}

/// Un evento a partir de una línea de yt-dlp.
pub fn leer_linea(linea: &str) -> Option<Value> {
    let linea = linea.trim_end_matches('\n');
    if let Some(ruta) = linea.strip_prefix(&format!("{MARCA}A\t")) {
        return Some(json!({"tipo": "archivo", "ruta": ruta}));
    }
    if linea.starts_with(&format!("{MARCA}P\t")) {
        let partes: Vec<&str> = linea.split('\t').collect();
        let pct = partes
            .get(1)
            .and_then(|p| p.trim().trim_end_matches('%').parse::<f64>().ok())
            .unwrap_or(0.0);
        return Some(
            json!({"tipo": "progreso", "porcentaje": pct, "titulo": partes.get(2).copied().unwrap_or("")}),
        );
    }
    linea
        .strip_prefix("ERROR:")
        .map(|e| json!({"tipo": "aviso", "texto": error_claro(e.trim())}))
}

pub fn error_claro(e: &str) -> String {
    let m = e.to_lowercase();
    if e.contains("Sign in to confirm") || e.contains("confirm you") {
        return "YouTube pide confirmar que no eres un bot: inicia sesión en tu navegador y elige ese navegador en Ajustes".into();
    }
    if e.contains("Private video") || m.contains("private") {
        return "Es privada: solo la puede ver su dueño".into();
    }
    if m.contains("unavailable") {
        return "Ya no está disponible en YouTube".into();
    }
    if m.contains("cookies") {
        return "No pude leer la sesión del navegador elegido (ciérralo y vuelve a intentar)".into();
    }
    if e.contains("Unable to download") || e.contains("urlopen") || e.contains("timed out") {
        return "Sin conexión con YouTube".into();
    }
    e.chars().take(200).collect()
}

/// Nombre y número de canciones de una playlist de YouTube (sin bajar nada).
pub fn titulo_playlist(url: &str, yt_dlp: &str, navegador: &str) -> (String, usize) {
    let mut c = Command::new(yt_dlp);
    c.args(["--flat-playlist", "-J", "--no-warnings"]);
    if !navegador.is_empty() {
        c.args(["--cookies-from-browser", navegador]);
    }
    let Ok(s) = c
        .args(["--", url])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return (String::new(), 0);
    };
    let texto = String::from_utf8_lossy(&s.stdout);
    let Ok(j) = serde_json::from_str::<Value>(if texto.is_empty() { "{}" } else { &texto }) else {
        return (String::new(), 0);
    };
    let nombre = texto_de(&j, "title");
    let nombre = nombre
        .strip_prefix("Album - ")
        .unwrap_or(&nombre)
        .trim()
        .to_string();
    (
        nombre,
        j.get("entries").and_then(Value::as_array).map_or(0, Vec::len),
    )
}

/// Corre yt-dlp (con stderr junto a stdout) y emite un evento por cada línea que se entiende.
fn correr(c: &[String], parar: &dyn Fn() -> bool, emitir: &mut dyn FnMut(Value)) {
    let falta = || json!({"tipo": "aviso", "texto": "Falta yt-dlp: sudo pacman -S yt-dlp", "fatal": true});
    let Ok((lector, escritor)) = std::io::pipe() else {
        emitir(falta());
        return;
    };
    let hijo = {
        let mut cmd = Command::new(&c[0]);
        cmd.args(&c[1..]).stdin(Stdio::null());
        match escritor.try_clone() {
            Ok(e2) => {
                cmd.stdout(escritor).stderr(e2);
            }
            Err(_) => {
                emitir(falta());
                return;
            }
        }
        cmd.spawn() // al salir de aquí, `cmd` suelta sus copias del tubo: al terminar yt-dlp llega el fin
    };
    let Ok(mut hijo) = hijo else {
        emitir(falta());
        return;
    };
    for linea in BufReader::new(lector).lines() {
        let Ok(linea) = linea else { break };
        if parar() {
            let _ = hijo.kill();
            break;
        }
        if let Some(ev) = leer_linea(&linea) {
            emitir(ev);
        }
    }
    let _ = hijo.wait();
}

/// Baja lo que haya en la URL. Eventos: inicio (nombre, total, es_lista), cancion (i, titulo), progreso,
/// archivo (ruta, i), aviso (texto) y fin (bajadas, fallidas).
pub fn descargar(
    url: &str,
    destino: &Path,
    navegador: &str,
    yt_dlp: &str,
    bajar: &dyn Fn(&str) -> Result<String, String>,
    parar: &dyn Fn() -> bool,
    emitir: &mut dyn FnMut(Value),
) {
    let Some((servicio, clase, id)) = tipo_de(url) else {
        emitir(
            json!({"tipo": "aviso", "texto": "Ese enlace no es de YouTube, YouTube Music ni Spotify", "fatal": true}),
        );
        emitir(json!({"tipo": "fin", "bajadas": 0, "fallidas": 0}));
        return;
    };
    let _ = std::fs::create_dir_all(destino);
    let (mut bajadas, mut fallidas) = (0u64, 0u64);

    if servicio == "spotify" {
        let (nombre, pistas) = match bajar(&format!("https://open.spotify.com/embed/{clase}/{id}"))
            .and_then(|p| leer_spotify(&p))
        {
            Ok(x) => x,
            Err(e) => {
                emitir(
                    json!({"tipo": "aviso", "texto": format!("No pude leer la lista de Spotify: {e}"), "fatal": true}),
                );
                emitir(json!({"tipo": "fin", "bajadas": 0, "fallidas": 0}));
                return;
            }
        };
        emitir(
            json!({"tipo": "inicio", "nombre": nombre, "total": pistas.len(), "es_lista": clase != "track"}),
        );
        for (i, p) in pistas.iter().enumerate() {
            if parar() {
                break;
            }
            emitir(
                json!({"tipo": "cancion", "i": i, "titulo": format!("{} · {}", texto_de(p, "titulo"), texto_de(p, "artistas"))}),
            );
            let mut ruta = false;
            correr(
                &comando(&busqueda_ytmusic(p), destino, navegador, false, yt_dlp),
                parar,
                &mut |mut ev| {
                    if ev["tipo"] == "archivo" {
                        ruta = true;
                        ev["i"] = json!(i);
                    }
                    emitir(ev);
                },
            );
            if ruta {
                bajadas += 1;
            } else {
                fallidas += 1;
                emitir(
                    json!({"tipo": "aviso", "texto": format!("No encontré «{}» en YouTube Music", texto_de(p, "titulo"))}),
                );
            }
        }
        emitir(json!({"tipo": "fin", "bajadas": bajadas, "fallidas": fallidas}));
        return;
    }

    let es_lista = clase == "playlist";
    let (nombre, total) = if es_lista {
        titulo_playlist(url, yt_dlp, navegador)
    } else {
        (String::new(), 1)
    };
    emitir(json!({"tipo": "inicio", "nombre": nombre, "total": total, "es_lista": es_lista}));
    let mut i = 0u64;
    correr(
        &comando(url, destino, navegador, !es_lista, yt_dlp),
        parar,
        &mut |mut ev| {
            if ev["tipo"] == "archivo" {
                ev["i"] = json!(i);
                i += 1;
                bajadas += 1;
            }
            emitir(ev);
        },
    );
    if total > 0 && (bajadas as usize) < total && !parar() {
        fallidas = (total as u64) - bajadas;
    }
    emitir(json!({"tipo": "fin", "bajadas": bajadas, "fallidas": fallidas}));
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn partes_de_url() {
        assert_eq!(
            partes("https://Music.YouTube.com:443/watch?v=a%2Bb&x=1#f"),
            ("music.youtube.com".into(), "/watch".into(), "v=a%2Bb&x=1".into())
        );
        assert_eq!(valor_consulta("v=a%2Bb+c&x=1", "v").as_deref(), Some("a+b c"));
        assert_eq!(
            desescapar_html("&#123;&quot;a&quot;:&amp;x&#x41;&zz;"),
            "{\"a\":&xA&zz;"
        );
        assert_eq!(segundos("1:02:03"), 3723);
        assert_eq!(segundos("123"), 0);
    }
}
