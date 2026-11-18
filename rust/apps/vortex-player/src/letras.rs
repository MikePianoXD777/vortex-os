//! Limpieza de títulos/artistas y lectura de letras sincronizadas (LRC).

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

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
        let texto = MARCA_LRC.replace_all(fila, "").trim().to_string();
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
