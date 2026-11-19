//! Limpieza de títulos/artistas y lectura de letras sincronizadas (LRC).

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::red::{self, ErrorRed, Peticion};

pub const API: &str = "https://lrclib.net/api";
pub const AGENTE: &str = "Vortex Player (https://github.com/MikePianoXD777/vortex-os)";
/// Si no había letra, se vuelve a buscar en una semana.
pub const REINTENTAR: f64 = 7.0 * 24.0 * 3600.0;
/// Una sin tiempos: se busca la sincronizada cada 3 días.
pub const MEJORAR: f64 = 3.0 * 24.0 * 3600.0;

static PARENTESIS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[(\[【「][^)\]】」]*[)\]】」]").unwrap());

static BASURA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*[(\[【「]([^)\]】」]*?(official|oficial|video|vídeo|audio|lyric|letra|visuali[sz]er|mv\b|m/v|hd|4k|hq|remaster|en vivo|live|explicit|clean|color coded|sub|español|english)[^)\]】」]*)[)\]】」]",
    )
    .unwrap()
});

static COLA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*[-|/~]?\s*(official\s+(music\s+)?video|official\s+(audio|mv|lyric\s+video)|music\s+video|lyric\s+video|lyrics?|m/?v|audio|visuali[sz]er|video\s+oficial|v[ií]deo\s+oficial|letra|hd|4k)\s*$",
    )
    .unwrap()
});

static COMILLAS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^(.*?)\s*[「『【"“]([^」』】"”]+)[」』】"”]"#).unwrap());

static FEAT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s+(featuring\b|feat\b\.?|ft\b\.?).*$").unwrap());

static SUFIJO_ARTISTA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\s*-\s*Topic$|VEVO$|\s+Official$").unwrap());

static SUFIJO_ARTISTA_COMPLETO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*-\s*Topic$|VEVO$|\s+Official$|\s*(official\s+)?(channel|ch\.?)$").unwrap()
});

static SEP_ARTISTAS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r",| & | x | feat| ft\.").unwrap());

static MARCA_LRC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?]").unwrap());

/// Línea de letra con tiempo en milisegundos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineaLrc {
    pub ms: i64,
    pub texto: String,
}

/// Limpia el nombre completo de artistas (`"YOASOBI - Topic"` -> `"YOASOBI"`, `"AdoVEVO"` -> `"Ado"`).
pub fn artista_limpio(artista: &str) -> String {
    SUFIJO_ARTISTA_COMPLETO
        .replace(artista.trim(), "")
        .trim()
        .to_string()
}

/// Toma solo el artista principal sin sufijos de YouTube.
pub fn limpiar_artista(artista: &str) -> String {
    let primero = SEP_ARTISTAS.split(artista).next().unwrap_or("");
    SUFIJO_ARTISTA.replace(primero, "").trim().to_string()
}

/// Quita etiquetas de video oficial, prefijo `Artista - ` y `feat. ...`.
pub fn limpiar(titulo: &str, artista: &str) -> String {
    let mut t = BASURA.replace_all(titulo, "").trim().to_string();
    if let Some(guion) = t.find(" - ") {
        let izq = t[..guion].trim().to_lowercase();
        let a = limpiar_artista(artista).to_lowercase();
        if a.is_empty() || izq == a || a.contains(&izq) || izq.contains(&a) {
            t = t[guion + 3..].trim().to_string();
        }
    }
    t = FEAT.replace(&t, "").to_string();
    let limpio = t.trim_matches([' ', '-', '|', '"']);
    if limpio.is_empty() {
        titulo.trim().to_string()
    } else {
        limpio.to_string()
    }
}

/// Limpieza profunda de títulos de YouTube (`YOASOBI「アイドル」Official Music Video` -> `アイドル`).
pub fn titulo_limpio(titulo: &str, artista: &str) -> String {
    let mut t = titulo.to_string();
    if let Some(cap) = COMILLAS.captures(&t) {
        let prefijo = cap.get(1).map(|m| m.as_str().trim()).unwrap_or("");
        let dentro = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let a = limpiar_artista(artista).to_lowercase();
        let p_low = prefijo.to_lowercase();
        if !prefijo.is_empty() && (a.is_empty() || a.contains(&p_low) || p_low.contains(&a)) {
            t = dentro.to_string();
        }
    }
    t = limpiar(&t, artista);
    for _ in 0..3 {
        let siguiente = COLA
            .replace(&t, "")
            .trim_matches([' ', '-', '|', '~'])
            .to_string();
        if siguiente == t || siguiente.is_empty() {
            break;
        }
        t = siguiente;
    }
    let final_t = t.trim();
    if final_t.is_empty() {
        titulo.trim().to_string()
    } else {
        final_t.to_string()
    }
}

/// Parsea texto LRC (`[mm:ss.xx] texto`) devolviendo las líneas ordenadas por tiempo.
pub fn leer_lrc(lrc: &str) -> Vec<LineaLrc> {
    let mut salida = Vec::new();
    for fila in lrc.lines() {
        let marcas: Vec<_> = MARCA_LRC.captures_iter(fila).collect();
        if marcas.is_empty() {
            continue;
        }
        // Lo que va después de la última marca (como la de Python)
        let texto = fila[marcas[marcas.len() - 1].get(0).map_or(0, |m| m.end())..]
            .trim()
            .to_string();
        for m in marcas {
            let min: i64 = m[1].parse().unwrap_or(0);
            let seg: i64 = m[2].parse().unwrap_or(0);
            let frac_str = m.get(3).map(|x| x.as_str()).unwrap_or("0");
            let frac_padded = format!("{frac_str:0<3}");
            let frac: i64 = frac_padded[..3].parse().unwrap_or(0);
            let ms = (min * 60 + seg) * 1000 + frac;
            salida.push(LineaLrc {
                ms,
                texto: texto.clone(),
            });
        }
    }
    salida.sort_by_key(|l| l.ms);
    salida
}

/// Índice de la línea actual para `ms` (`-1` antes de la primera marca, con 300 ms de adelanto visual).
pub fn actual(lineas: &[LineaLrc], ms: i64) -> isize {
    let mut idx = -1isize;
    for (i, l) in lineas.iter().enumerate() {
        if l.ms <= ms + 300 {
            idx = i as isize;
        } else {
            break;
        }
    }
    idx
}

/// Una letra: sus líneas (`ms` = -1 si no tiene tiempos) y si va sincronizada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Letra {
    pub lineas: Vec<LineaLrc>,
    pub sincronizada: bool,
}

pub fn texto_plano(plano: &str) -> Vec<LineaLrc> {
    plano
        .lines()
        .map(|l| LineaLrc {
            ms: -1,
            texto: l.trim().to_string(),
        })
        .collect()
}

fn tiene(o: &Value, clave: &str) -> bool {
    o.get(clave)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty())
}

/// De los resultados de LRCLIB, el que dura lo mismo (±`holgura` s), mejor si viene sincronizado.
pub fn elegir(resultados: &[Value], segundos: i64, holgura: f64) -> Option<&Value> {
    let cerca: Vec<&Value> = resultados
        .iter()
        .filter(|o| o.is_object() && (tiene(o, "syncedLyrics") || tiene(o, "plainLyrics")))
        .filter(|o| {
            segundos <= 0
                || (o.get("duration").and_then(Value::as_f64).unwrap_or(0.0) - segundos as f64).abs()
                    <= holgura
        })
        .collect();
    cerca
        .iter()
        .find(|o| tiene(o, "syncedLyrics"))
        .or(cerca.first())
        .copied()
}

pub fn de_json(o: &Value) -> Option<Letra> {
    if tiene(o, "syncedLyrics") {
        let lineas = leer_lrc(o["syncedLyrics"].as_str().unwrap_or_default());
        if !lineas.is_empty() {
            return Some(Letra {
                lineas,
                sincronizada: true,
            });
        }
    }
    tiene(o, "plainLyrics").then(|| Letra {
        lineas: texto_plano(o["plainLyrics"].as_str().unwrap_or_default()),
        sincronizada: false,
    })
}

/// Pide a LRCLIB: `Ok(None)` si contesta 404.
pub fn pedir_lrclib(url: &str) -> Result<Option<Value>, String> {
    match red::json(&Peticion::get(url).cabecera(format!("User-Agent: {AGENTE}"))) {
        Ok(v) => Ok(Some(v)),
        Err(ErrorRed::Http(404, _)) => Ok(None),
        Err(ErrorRed::Http(c, _)) => Err(format!("LRCLIB contestó {c}")),
        Err(e) => Err(e.to_string()),
    }
}

/// Busca en internet: primero exacto (título, artista y duración) y luego más suelto. `pedir` es
/// `pedir_lrclib` (las pruebas le pasan uno falso).
pub fn buscar_con(
    pedir: &dyn Fn(&str) -> Result<Option<Value>, String>,
    titulo: &str,
    artista: &str,
    segundos: i64,
) -> Result<Option<Letra>, String> {
    let (t, a) = (limpiar(titulo, artista), limpiar_artista(artista));
    let c = red::escapar_ruta;
    if !a.is_empty() && segundos > 0 {
        let r = pedir(&format!(
            "{API}/get?track_name={}&artist_name={}&duration={segundos}",
            c(&t),
            c(&a)
        ))?;
        if let Some(l) = r.as_ref().filter(|r| r.is_object()).and_then(de_json) {
            return Ok(Some(l));
        }
    }
    let corto = PARENTESIS.replace_all(&t, "").trim().to_string();
    let corto = if corto.is_empty() { t.clone() } else { corto };
    let mut intentos = Vec::new();
    if !a.is_empty() {
        intentos.push(format!(
            "{API}/search?track_name={}&artist_name={}",
            c(&corto),
            c(&a)
        ));
    }
    intentos.push(format!("{API}/search?q={}", c(format!("{a} {corto}").trim())));
    if corto != t {
        intentos.push(format!("{API}/search?q={}", c(format!("{a} {t}").trim())));
    }
    for url in intentos {
        if let Some(Value::Array(r)) = pedir(&url)?
            && let Some(l) = elegir(&r, segundos, 4.0).and_then(de_json)
        {
            return Ok(Some(l));
        }
    }
    Ok(None)
}

pub fn buscar(titulo: &str, artista: &str, segundos: i64) -> Result<Option<Letra>, String> {
    if red::sin_red() {
        return Err("sin red (pruebas)".into());
    }
    buscar_con(&pedir_lrclib, titulo, artista, segundos)
}

/// El archivo de la caché: el mismo nombre que le da la de Python (así se aprovechan las que ya bajó).
pub fn archivo(carpeta: &Path, titulo: &str, artista: &str, segundos: i64) -> PathBuf {
    let h = crate::cripto::hex(&crate::cripto::sha1(
        format!("{titulo}|{artista}|{segundos}").as_bytes(),
    ));
    carpeta.join(format!("{}.json", &h[..20]))
}

/// Si ya se buscó (la haya o no): para bajar las letras de toda tu música sin repetir.
pub fn guardada(carpeta: &Path, titulo: &str, artista: &str, segundos: i64) -> bool {
    archivo(carpeta, titulo, artista, segundos).exists()
}

/// La letra de YouTube Music de un video (sin tiempos): `Ok(None)` si no tiene.
pub type DeYoutube<'a> = &'a dyn Fn(&str) -> Result<Option<String>, String>;
/// Buscar en LRCLIB (título, artista, segundos).
pub type Buscador<'a> = &'a dyn Fn(&str, &str, i64) -> Result<Option<Letra>, String>;

/// La letra guardada, o la busca (LRCLIB y luego YouTube Music) y la guarda. `Ok(None)` si no hay (y no se
/// vuelve a buscar en una semana). Sin internet da `Err` y no anota nada.
pub fn de(
    carpeta: &Path,
    titulo: &str,
    artista: &str,
    segundos: i64,
    video_id: &str,
    de_youtube: Option<DeYoutube<'_>>,
) -> Result<Option<Letra>, String> {
    de_con(
        &|t, a, s| buscar(t, a, s),
        carpeta,
        titulo,
        artista,
        segundos,
        video_id,
        de_youtube,
    )
}

/// `de` con la búsqueda inyectable (para las pruebas).
#[allow(clippy::too_many_arguments)]
pub fn de_con(
    buscar_: Buscador<'_>,
    carpeta: &Path,
    titulo: &str,
    artista: &str,
    segundos: i64,
    video_id: &str,
    de_youtube: Option<DeYoutube<'_>>,
) -> Result<Option<Letra>, String> {
    let f = archivo(carpeta, titulo, artista, segundos);
    let ahora = crate::fecha::ahora();
    let mut vieja = None;
    if let Some(o) = std::fs::read_to_string(&f)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    {
        let cuando = o.get("cuando").and_then(Value::as_f64).unwrap_or(0.0);
        if o.get("nada").and_then(Value::as_bool).unwrap_or(false) {
            if ahora - cuando < REINTENTAR {
                return Ok(None);
            }
        } else if let Ok(l) = serde_json::from_value::<Letra>(o.clone()) {
            if l.sincronizada || ahora - cuando < MEJORAR {
                return Ok(Some(l));
            }
            vieja = Some(l);
        }
    }
    let mut falla = None;
    let mut letra = buscar_(titulo, artista, segundos).unwrap_or_else(|e| {
        falla = Some(e);
        None
    });
    if letra.is_none()
        && !video_id.is_empty()
        && let Some(yt) = de_youtube
    {
        match yt(video_id) {
            Ok(Some(plano)) if !plano.is_empty() => {
                letra = Some(Letra {
                    lineas: texto_plano(&plano),
                    sincronizada: false,
                })
            }
            Ok(_) => {}
            Err(e) => falla = falla.or(Some(e)),
        }
    }
    if letra.is_none() {
        if vieja.is_some() {
            return Ok(vieja);
        }
        if let Some(e) = falla {
            return Err(e);
        }
    }
    let _ = std::fs::create_dir_all(carpeta);
    let guardar = match &letra {
        None => json!({"nada": true, "cuando": ahora}),
        Some(l) => json!({"lineas": l.lineas, "sincronizada": l.sincronizada, "cuando": ahora}),
    };
    let _ = vortex_core::config::escribir_atomico(&f, &guardar.to_string());
    Ok(letra)
}
