//! Calidad real de cada canción (códec, bits, frecuencia) leída mediante `ffprobe`.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Códecs sin pérdida reconocidos directamente.
pub const SIN_PERDIDA: &[&str] = &["flac", "alac", "wavpack", "ape", "tta", "mlp", "truehd", "dsd"];

/// Extensiones de audio soportadas al escanear carpetas.
pub const EXTENSIONES: &[&str] = &[
    ".flac", ".wav", ".wave", ".aif", ".aiff", ".aifc", ".m4a", ".alac", ".mp3", ".ogg", ".oga", ".opus",
    ".wv", ".ape", ".aac", ".webm", ".mka", ".dsf", ".dff", ".tta",
];

/// Nivel visual de calidad de una canción.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NivelCalidad {
    Hires,
    Cd,
    Comprimida,
}

impl NivelCalidad {
    /// Cadena compatible con Python y QML (`"hires"`, `"cd"`, `"comprimida"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hires => "hires",
            Self::Cd => "cd",
            Self::Comprimida => "comprimida",
        }
    }
}

/// Metadatos técnicos y de etiquetas de un archivo de audio.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Info {
    pub ruta: String,
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
    pub extra: HashMap<String, String>,
}

impl Info {
    /// Etiqueta legible de calidad (`"Hi-Res 24/96"`, `"CD 16/44,1"`, `"Opus 160 kbps"`, etc.).
    pub fn etiqueta(&self) -> String {
        etiqueta(&self.codec, self.bits, self.hz, self.kbps)
    }

    /// Nivel de calidad (`"hires"`, `"cd"` o `"comprimida"`).
    pub fn nivel(&self) -> &'static str {
        nivel(&self.codec, self.bits, self.hz)
    }
}

/// Indica si la extensión (con o sin punto inicial) corresponde a un archivo de audio soportado.
pub fn extension_soportada(ruta: &Path) -> bool {
    let Some(ext) = ruta.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    let con_punto = format!(".{}", ext.to_lowercase());
    EXTENSIONES.contains(&con_punto.as_str())
}

/// Devuelve `true` si el códec es sin pérdida (FLAC, ALAC, PCM, DSD, etc.).
pub fn es_sin_perdida(codec: &str) -> bool {
    SIN_PERDIDA.contains(&codec) || codec.starts_with("pcm_") || codec.starts_with("dsd_")
}

/// Familia normalizada de un códec (`"pcm_s16le"` -> `"pcm"`, `"dsd_lsbf"` -> `"dsd"`).
pub fn familia(codec: &str) -> &str {
    if codec.starts_with("pcm_") {
        "pcm"
    } else if codec.starts_with("dsd_") {
        "dsd"
    } else {
        codec
    }
}

fn nombre_codec(codec: &str) -> String {
    match familia(codec) {
        "opus" => "Opus".into(),
        "aac" => "AAC".into(),
        "mp3" => "MP3".into(),
        "vorbis" => "Vorbis".into(),
        "flac" => "FLAC".into(),
        "alac" => "ALAC".into(),
        "wavpack" => "WavPack".into(),
        "ape" => "APE".into(),
        "pcm" => "WAV".into(),
        "dsd" => "DSD".into(),
        "tta" => "TTA".into(),
        "" => "?".into(),
        other => other.to_uppercase(),
    }
}

fn khz(hz: i64) -> String {
    let k = hz as f64 / 1000.0;
    if (k - k.trunc()).abs() < f64::EPSILON {
        format!("{}", k as i64)
    } else {
        format!("{k}").replace('.', ",")
    }
}

/// Genera la etiqueta de calidad equivalente a `calidad.etiqueta` en Python.
pub fn etiqueta(codec: &str, bits: i64, hz: i64, kbps: i64) -> String {
    if es_sin_perdida(codec) {
        if bits > 16 || hz > 48_000 {
            if bits > 0 {
                return format!("Hi-Res {bits}/{}", khz(hz));
            }
            return format!("Hi-Res {} kHz", khz(hz));
        }
        let b = if bits > 0 { bits } else { 16 };
        return format!("CD {b}/{}", khz(hz));
    }
    let nombre = nombre_codec(codec);
    if kbps > 0 {
        format!("{nombre} {kbps} kbps")
    } else {
        nombre
    }
}

/// Clasifica la calidad en `"hires"`, `"cd"` o `"comprimida"`.
pub fn nivel(codec: &str, bits: i64, hz: i64) -> &'static str {
    if !es_sin_perdida(codec) {
        return NivelCalidad::Comprimida.as_str();
    }
    if bits > 16 || hz > 48_000 {
        NivelCalidad::Hires.as_str()
    } else {
        NivelCalidad::Cd.as_str()
    }
}

fn valor_entero(v: Option<&Value>) -> i64 {
    match v {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(0),
        Some(Value::String(s)) => s.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn valor_float(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn bits_de_stream(stream: &Value) -> i64 {
    for clave in ["bits_per_raw_sample", "bits_per_sample"] {
        let v = valor_entero(stream.get(clave));
        if v > 0 {
            return v;
        }
    }
    match stream.get("sample_fmt").and_then(Value::as_str).unwrap_or("") {
        "s16" | "s16p" => 16,
        "s32" | "s32p" | "flt" | "fltp" => 32,
        "dbl" => 64,
        "u8" => 8,
        _ => 0,
    }
}

fn recoger_etiquetas(fmt: &Value, audio: &Value) -> HashMap<String, String> {
    let mut mapa = HashMap::new();
    for origen in [fmt, audio] {
        if let Some(tags) = origen.get("tags").and_then(Value::as_object) {
            for (k, v) in tags {
                let clave = k.to_lowercase();
                mapa.entry(clave).or_insert_with(|| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                });
            }
        }
    }
    mapa
}

/// Extrae `Info` desde el JSON producido por `ffprobe`. Devuelve `None` si no hay pista de audio.
pub fn de_ffprobe(ruta: &str, json: &Value) -> Option<Info> {
    let streams = json.get("streams")?.as_array()?;
    let audio = streams
        .iter()
        .find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"))?;
    let fmt = json.get("format").unwrap_or(&Value::Null);
    let tags = recoger_etiquetas(fmt, audio);

    let codec = audio
        .get("codec_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut bits = if es_sin_perdida(&codec) {
        bits_de_stream(audio)
    } else {
        0
    };
    if codec == "alac" && bits == 0 {
        bits = 16;
    }

    let hz = valor_entero(audio.get("sample_rate"));
    let br = valor_entero(audio.get("bit_rate").or_else(|| fmt.get("bit_rate")));
    let kbps = if br > 0 {
        ((br as f64) / 1000.0).round() as i64
    } else {
        0
    };
    let duracion = valor_float(fmt.get("duration").or_else(|| audio.get("duration")));

    let pista = tags
        .get("track")
        .and_then(|t| t.split('/').next())
        .and_then(|n| n.trim().parse::<i64>().ok())
        .unwrap_or(0);

    let titulo = tags
        .get("title")
        .filter(|s| !s.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            Path::new(ruta)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        });

    let artista = tags
        .get("artist")
        .filter(|s| !s.is_empty())
        .or_else(|| tags.get("album_artist").filter(|s| !s.is_empty()))
        .cloned()
        .unwrap_or_default();

    let album = tags.get("album").cloned().unwrap_or_default();
    let portada = streams
        .iter()
        .any(|s| s.get("codec_type").and_then(Value::as_str) == Some("video"));

    let mut extra = HashMap::new();
    for k in ["purl", "comment", "description"] {
        if let Some(v) = tags.get(k) {
            extra.insert(k.to_string(), v.clone());
        }
    }

    Some(Info {
        ruta: ruta.to_string(),
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
        extra,
    })
}

/// Ejecuta `ffprobe` sobre `ruta` y devuelve su `Info`, o `None` si falla o no contiene audio.
pub fn leer(ruta: &Path, ffprobe: &str) -> Option<Info> {
    let out = Command::new(ffprobe)
        .stdin(std::process::Stdio::null())
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "--",
        ])
        .arg(ruta)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let json: Value = serde_json::from_slice(&out.stdout).ok()?;
    de_ffprobe(&ruta.to_string_lossy(), &json)
}
