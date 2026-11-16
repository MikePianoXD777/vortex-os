//! Ecualizador de Vortex Player: perfiles compartidos con Android (`Ecualizador.kt`),
//! integración con EasyEffects en Vortexine's OS y filtros `lavfi` dentro de `mpv` (Windows / pruebas).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rutas;

pub const FRECUENCIAS: [i64; 10] = [31, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000];
pub const PRESETS: &[&str] = &[
    "Flat", "Bass", "Treble", "Vocal", "Pop", "Rock", "Jazz", "Classic",
];
pub const MIN_BANDAS: usize = 2;
pub const MAX_BANDAS: usize = 16;

/// Los 19 perfiles compartidos con el cliente Android (`Ecualizador.kt`).
pub const PERFILES: &[(&str, [f64; 10])] = &[
    ("Plano", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    ("Graves", [6.0, 6.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]),
    (
        "Graves al máximo",
        [9.0, 8.0, 6.0, 3.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0],
    ),
    (
        "Menos graves",
        [-6.0, -5.0, -4.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ),
    ("Agudos", [0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 3.0, 5.0, 6.0, 7.0]),
    ("Voz", [-2.0, -2.0, -1.0, 1.0, 3.0, 4.0, 4.0, 3.0, 1.0, 0.0]),
    ("Pop", [-1.0, 1.0, 3.0, 4.0, 3.0, 0.0, -1.0, -1.0, 1.0, 2.0]),
    ("Rock", [5.0, 4.0, 3.0, 1.0, -1.0, -1.0, 1.0, 3.0, 4.0, 5.0]),
    ("Metal", [4.0, 4.0, 2.0, 0.0, -2.0, 0.0, 2.0, 4.0, 5.0, 4.0]),
    ("Electrónica", [6.0, 5.0, 2.0, 0.0, -2.0, 1.0, 0.0, 2.0, 4.0, 5.0]),
    ("Hip hop", [6.0, 5.0, 3.0, 2.0, -1.0, -1.0, 1.0, 0.0, 2.0, 3.0]),
    ("Reggaetón", [7.0, 6.0, 3.0, 0.0, -1.0, 0.0, 1.0, 2.0, 2.0, 2.0]),
    (
        "J-pop y anime",
        [2.0, 2.0, 1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 3.0],
    ),
    ("Clásica", [4.0, 3.0, 2.0, 1.0, -1.0, -1.0, 0.0, 2.0, 3.0, 4.0]),
    ("Jazz", [3.0, 2.0, 1.0, 2.0, -2.0, -2.0, 0.0, 1.0, 3.0, 4.0]),
    ("Piano", [2.0, 1.0, 0.0, 2.0, 3.0, 1.0, 3.0, 4.0, 3.0, 2.0]),
    ("Acústica", [4.0, 3.0, 2.0, 1.0, 1.0, 1.0, 2.0, 3.0, 3.0, 2.0]),
    ("Lo-fi", [3.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0, -3.0, -4.0, -5.0]),
    (
        "Volumen bajo",
        [6.0, 5.0, 3.0, 0.0, -1.0, 0.0, 0.0, 2.0, 4.0, 5.0],
    ),
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Banda {
    pub f: f64,
    pub g: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EstadoEq {
    pub bandas: Vec<Banda>,
    pub preset: String,
    pub pending: bool,
}

/// Ruta al script `quickshell/media/equalizer.sh` de Serpantinum si existe.
pub fn script() -> Option<PathBuf> {
    let env_dir = env::var("SERPANTINUM_DIR").unwrap_or_default();
    for base in [env_dir.as_str(), "/usr/share/serpantinum/src"] {
        if base.is_empty() {
            continue;
        }
        let candidato = Path::new(base).join("quickshell/media/equalizer.sh");
        if candidato.is_file() {
            return Some(candidato);
        }
    }
    None
}

/// Indica si el ecualizador se aplica dentro de `mpv` (Windows o `VORTEX_PLAYER_EQ_MPV=1`).
pub fn en_mpv() -> bool {
    env::var("VORTEX_PLAYER_EQ_MPV").as_deref() == Ok("1") || (script().is_none() && cfg!(windows))
}

pub fn disponible() -> bool {
    en_mpv() || script().is_some()
}

pub fn archivo_estado() -> PathBuf {
    if let Ok(p) = env::var("VORTEX_PLAYER_EQ")
        && !p.is_empty()
    {
        return PathBuf::from(p);
    }
    if en_mpv() {
        return rutas::config().join("ecualizador.json");
    }
    vortex_core::rutas::home().join(".local/state/serpantinum/music/eq_state.json")
}

fn redondear_1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn formatear_num(v: f64) -> String {
    if (v - v.trunc()).abs() < 1e-9 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

pub fn guardar_en(ruta: &Path, bandas: &[Banda], preset: &str) -> bool {
    if let Some(padre) = ruta.parent()
        && fs::create_dir_all(padre).is_err()
    {
        return false;
    }
    let tmp = ruta.with_extension("tmp");
    let cuerpo = serde_json::json!({
        "bandas": bandas,
        "preset": preset,
    });
    let Ok(texto) = serde_json::to_string(&cuerpo) else {
        return false;
    };
    if fs::write(&tmp, texto).is_err() {
        return false;
    }
    fs::rename(tmp, ruta).is_ok()
}

/// Construye el filtro `af` de `mpv` para `bandas` (vacío si todas están en 0).
pub fn filtro_mpv(bandas: &[Banda]) -> String {
    let activas: Vec<&Banda> = bandas.iter().filter(|b| b.g.abs() >= 0.05).collect();
    if activas.is_empty() {
        return String::new();
    }
    let mut partes: Vec<String> = activas
        .iter()
        .map(|b| {
            format!(
                "equalizer=f={}:t=o:w=1:g={}",
                b.f.round() as i64,
                formatear_num(redondear_1(b.g))
            )
        })
        .collect();
    let pico = activas.iter().map(|b| b.g).fold(0.0_f64, f64::max);
    if pico > 0.0 {
        partes.insert(
            0,
            format!("volume=volume={}dB", formatear_num(-redondear_1(pico))),
        );
    }
    format!("lavfi=[{}]", partes.join(","))
}

fn valor_f64(v: Option<&Value>) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Extrae bandas `{f, g}` desde formato nuevo o formato antiguo (`b1`..`b10`).
pub fn bandas_de(datos: &Value) -> Vec<Banda> {
    if let Some(lista) = datos.get("bandas").and_then(Value::as_array)
        && lista.len() >= MIN_BANDAS
    {
        return lista
            .iter()
            .map(|x| Banda {
                f: valor_f64(x.get("f")),
                g: valor_f64(x.get("g")),
            })
            .collect();
    }
    FRECUENCIAS
        .iter()
        .enumerate()
        .map(|(i, &f)| {
            let clave = format!("b{}", i + 1);
            Banda {
                f: f as f64,
                g: valor_f64(datos.get(&clave)),
            }
        })
        .collect()
}

pub fn estado() -> EstadoEq {
    estado_en(&archivo_estado())
}

pub fn estado_en(ruta: &Path) -> EstadoEq {
    let contenido = fs::read_to_string(ruta).unwrap_or_default();
    let datos: Value = serde_json::from_str(&contenido).unwrap_or(Value::Null);
    let bandas = bandas_de(&datos);
    let preset = datos
        .get("preset")
        .and_then(Value::as_str)
        .unwrap_or("Flat")
        .to_string();
    let pending = datos.get("pending").and_then(Value::as_bool).unwrap_or(false);
    EstadoEq {
        bandas,
        preset,
        pending,
    }
}

/// Formatea bandas como `"31:5,63:7,..."` con ganancia acotada a `[-12, 12]`.
pub fn texto_bandas(bandas: &[Banda]) -> String {
    bandas
        .iter()
        .map(|b| {
            let f = b.f.round() as i64;
            let g = redondear_1(b.g.clamp(-12.0, 12.0));
            format!("{f}:{}", formatear_num(g))
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn correr(args: &[&str]) -> bool {
    let Some(s) = script() else {
        return false;
    };
    Command::new("bash")
        .arg(s)
        .args(args)
        .output()
        .is_ok_and(|o| o.status.success())
}

pub fn poner_bandas(bandas: &[Banda]) -> bool {
    if !(MIN_BANDAS..=MAX_BANDAS).contains(&bandas.len()) {
        return false;
    }
    if en_mpv() {
        return poner_bandas_mpv_en(&archivo_estado(), bandas);
    }
    let texto = texto_bandas(bandas);
    correr(&["set_bands", &texto]) && correr(&["apply"])
}

pub fn poner_bandas_mpv_en(ruta: &Path, bandas: &[Banda]) -> bool {
    if !(MIN_BANDAS..=MAX_BANDAS).contains(&bandas.len()) {
        return false;
    }
    let normalizadas: Vec<Banda> = bandas
        .iter()
        .map(|b| Banda {
            f: b.f,
            g: redondear_1(b.g.clamp(-12.0, 12.0)),
        })
        .collect();
    guardar_en(ruta, &normalizadas, "Custom")
}

fn perfil_de_preset(nombre: &str) -> Option<&'static str> {
    match nombre {
        "Flat" => Some("Plano"),
        "Bass" => Some("Graves"),
        "Treble" => Some("Agudos"),
        "Vocal" => Some("Voz"),
        "Pop" => Some("Pop"),
        "Rock" => Some("Rock"),
        "Jazz" => Some("Jazz"),
        "Classic" => Some("Clásica"),
        _ => None,
    }
}

pub fn poner_preset(nombre: &str) -> bool {
    if !PRESETS.contains(&nombre) {
        return false;
    }
    if en_mpv() {
        return poner_preset_mpv_en(&archivo_estado(), nombre);
    }
    correr(&["preset", nombre])
}

pub fn poner_preset_mpv_en(ruta: &Path, nombre: &str) -> bool {
    let Some(perfil) = perfil_de_preset(nombre) else {
        return false;
    };
    let bandas = bandas_de_perfil(perfil, &estado_en(ruta).bandas);
    guardar_en(ruta, &bandas, nombre)
}

/// Interpolación logarítmica en frecuencia de un perfil de 10 bandas.
pub fn db_en(perfil: &[f64; 10], hz: f64) -> f64 {
    let x = hz.max(1.0).ln();
    let f: Vec<f64> = FRECUENCIAS.iter().map(|&v| (v as f64).ln()).collect();
    if x <= f[0] {
        return perfil[0];
    }
    if x >= f[f.len() - 1] {
        return perfil[perfil.len() - 1];
    }
    let i = f.iter().rposition(|&v| v <= x).unwrap_or(0);
    let t = (x - f[i]) / (f[i + 1] - f[i]);
    perfil[i] + (perfil[i + 1] - perfil[i]) * t
}

pub fn buscar_perfil(nombre: &str) -> Option<&'static [f64; 10]> {
    PERFILES.iter().find(|(n, _)| *n == nombre).map(|(_, vals)| vals)
}

pub fn bandas_de_perfil(nombre: &str, bandas: &[Banda]) -> Vec<Banda> {
    let Some(perfil) = buscar_perfil(nombre) else {
        return bandas.to_vec();
    };
    bandas
        .iter()
        .map(|b| Banda {
            f: b.f,
            g: redondear_1(db_en(perfil, b.f)),
        })
        .collect()
}

pub fn perfil_de(bandas: &[Banda]) -> &'static str {
    for &(nombre, _) in PERFILES {
        let esperadas = bandas_de_perfil(nombre, bandas);
        if bandas
            .iter()
            .zip(esperadas.iter())
            .all(|(a, b)| (a.g - b.g).abs() < 0.15)
        {
            return nombre;
        }
    }
    "Personalizado"
}

pub fn poner_perfil(nombre: &str, bandas: &[Banda]) -> bool {
    if buscar_perfil(nombre).is_none() {
        return false;
    }
    poner_bandas(&bandas_de_perfil(nombre, bandas))
}
