//! Los Proton que hay en tu PC, y cuál usar (igual que `protones.py`).
//!
//! Busca en las carpetas de herramientas de Steam (la del sistema, donde pacman pone el de CachyOS, y las
//! de tu usuario) y en los Proton oficiales de Steam (`steamapps/common/Proton*`). Además ofrece
//! «GE-Proton», que umu-run baja solo la primera vez (unos 500 MB) y luego mantiene al día.
//!
//! El de serie es el Proton de CachyOS en su versión para el runtime de Steam (`proton-cachyos-slr`); si
//! no está, el nativo, y si tampoco, el más nuevo que encuentre.

use crate::Contexto;
use std::path::{Path, PathBuf};

pub const GE: &str = "GE-Proton";
const PREFERIDOS: [&str; 3] = ["proton-cachyos-slr", "proton-cachyos-native", "proton-cachyos"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proton {
    pub id: String,
    pub nombre: String,
    pub ruta: String,
    pub version: String,
}

fn nombre_de(id: &str) -> String {
    match id {
        "proton-cachyos-slr" | "proton-cachyos" => "Proton CachyOS".into(),
        "proton-cachyos-native" => "Proton CachyOS (nativo)".into(),
        otro => otro.into(),
    }
}

/// Dónde buscar (`VORTEX_RUNNER_PROTONES` en las pruebas).
pub fn carpetas(ctx: &Contexto) -> Vec<PathBuf> {
    if let Some(propias) = &ctx.carpetas_proton {
        return propias.clone();
    }
    let casa = vortex_core::rutas::home();
    let steam = [
        casa.join(".local/share/Steam"),
        casa.join(".steam/root"),
        casa.join(".steam/steam"),
        casa.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];
    let mut salida = vec![PathBuf::from("/usr/share/steam/compatibilitytools.d")];
    salida.extend(steam.iter().map(|s| s.join("compatibilitytools.d")));
    salida.extend(steam.iter().map(|s| s.join("steamapps/common")));
    salida
}

fn es_proton(carpeta: &Path) -> bool {
    carpeta.join("proton").is_file()
}

fn version(carpeta: &Path) -> String {
    std::fs::read(carpeta.join("version"))
        .ok()
        .and_then(|b| {
            String::from_utf8_lossy(&b)
                .split_whitespace()
                .last()
                .map(|v| v.chars().take(40).collect())
        })
        .unwrap_or_default()
}

/// Los números del nombre, para ordenar «GE-Proton10-3» después de «GE-Proton9-20».
fn orden_numerico(nombre: &str) -> Vec<u64> {
    let mut numeros = Vec::new();
    let mut actual = String::new();
    for c in nombre.chars().chain(std::iter::once(' ')) {
        if c.is_ascii_digit() {
            actual.push(c);
        } else if !actual.is_empty() {
            numeros.push(actual.parse().unwrap_or(u64::MAX));
            actual.clear();
        }
    }
    numeros.truncate(4);
    numeros
}

/// `[{id, nombre, ruta, version}]` sin repetir, primero los preferidos; GE-Proton al final.
pub fn lista(ctx: &Contexto) -> Vec<Proton> {
    let mut vistos: Vec<Proton> = Vec::new();
    for base in carpetas(ctx) {
        let Ok(rd) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut hijos: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
        hijos.sort();
        for c in hijos {
            let nombre = c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !c.is_dir() || !es_proton(&c) {
                continue;
            }
            if base.file_name().is_some_and(|n| n == "common") && !nombre.to_lowercase().starts_with("proton")
            {
                continue;
            }
            let real = crate::rutas::resolver(&c).to_string_lossy().into_owned();
            if vistos.iter().any(|v| v.ruta == real || v.id == nombre) {
                continue;
            }
            vistos.push(Proton {
                nombre: nombre_de(&nombre),
                version: version(&c),
                ruta: real,
                id: nombre,
            });
        }
    }
    let mut salida = Vec::new();
    for p in PREFERIDOS {
        if let Some(i) = vistos.iter().position(|v| v.id == p) {
            salida.push(vistos.remove(i));
        }
    }
    vistos.sort_by_key(|a| std::cmp::Reverse(orden_numerico(&a.id)));
    salida.extend(vistos);
    salida.push(Proton {
        id: GE.into(),
        nombre: "GE-Proton (se baja solo)".into(),
        ruta: GE.into(),
        version: String::new(),
    });
    salida
}

pub fn por_defecto(protones: &[Proton]) -> &Proton {
    &protones[0]
}

/// El Proton con ese id; si ya no está (lo quitaste), el de serie.
pub fn buscar<'a>(id: Option<&str>, protones: &'a [Proton]) -> &'a Proton {
    protones
        .iter()
        .find(|p| Some(p.id.as_str()) == id)
        .unwrap_or_else(|| por_defecto(protones))
}
