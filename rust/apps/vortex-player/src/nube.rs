//! Biblioteca compartida en la nube (Google Drive), compatible con `Nube.kt` (Android) y `nube.py`.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMATO: i64 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CancionNube {
    pub titulo: String,
    pub artista: String,
    pub album: String,
    pub duracion: i64,
    pub agregada: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaylistNube {
    pub nombre: String,
    pub url: String,
    pub canciones: Vec<String>,
    pub modificada: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocNube {
    pub formato: i64,
    pub canciones: BTreeMap<String, CancionNube>,
    pub quitadas: BTreeMap<String, i64>,
    pub playlists: BTreeMap<String, PlaylistNube>,
    pub borradas: BTreeMap<String, i64>,
}

impl Default for DocNube {
    fn default() -> Self {
        vacio()
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Local {
    pub canciones: BTreeMap<String, CancionNube>,
    pub quitadas: BTreeMap<String, i64>,
    pub playlists: BTreeMap<String, PlaylistNube>,
    pub borradas: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub bajar: Vec<(String, CancionNube)>,
    pub borrar: Vec<String>,
    pub playlists: Vec<(String, PlaylistNube)>,
    pub quitar_playlists: Vec<String>,
}

pub fn vacio() -> DocNube {
    DocNube {
        formato: FORMATO,
        canciones: BTreeMap::new(),
        quitadas: BTreeMap::new(),
        playlists: BTreeMap::new(),
        borradas: BTreeMap::new(),
    }
}

pub fn leer(texto: &str) -> DocNube {
    let Ok(val) = serde_json::from_str::<Value>(texto) else {
        return vacio();
    };
    if !val.is_object() {
        return vacio();
    }
    let mut doc = vacio();
    if let Some(canciones) = val.get("canciones").and_then(Value::as_object) {
        for (k, v) in canciones {
            if let Ok(c) = serde_json::from_value::<CancionNube>(v.clone()) {
                doc.canciones.insert(k.clone(), c);
            }
        }
    }
    if let Some(quitadas) = val.get("quitadas").and_then(Value::as_object) {
        for (k, v) in quitadas {
            if let Some(t) = v.as_i64() {
                doc.quitadas.insert(k.clone(), t);
            }
        }
    }
    if let Some(playlists) = val.get("playlists").and_then(Value::as_object) {
        for (k, v) in playlists {
            if let Ok(p) = serde_json::from_value::<PlaylistNube>(v.clone()) {
                doc.playlists.insert(k.clone(), p);
            }
        }
    }
    if let Some(borradas) = val.get("borradas").and_then(Value::as_object) {
        for (k, v) in borradas {
            if let Some(t) = v.as_i64() {
                doc.borradas.insert(k.clone(), t);
            }
        }
    }
    doc
}

pub fn texto(doc: &DocNube) -> String {
    serde_json::to_string(doc).unwrap_or_else(|_| "{}".to_string())
}

pub fn desde(local: &Local) -> DocNube {
    DocNube {
        formato: FORMATO,
        canciones: local.canciones.clone(),
        quitadas: local.quitadas.clone(),
        playlists: local.playlists.clone(),
        borradas: local.borradas.clone(),
    }
}

/// Mezcla dos copias (la de la nube y la local) sin perder cambios en ningún lado.
pub fn mezclar(a: &DocNube, b: &DocNube) -> DocNube {
    let mut salida = vacio();

    for doc in [a, b] {
        for (vid, c) in &doc.canciones {
            let reemplaza = salida
                .canciones
                .get(vid)
                .is_none_or(|prev| c.agregada > prev.agregada);
            if reemplaza {
                salida.canciones.insert(vid.clone(), c.clone());
            }
        }
        for (vid, &t) in &doc.quitadas {
            let prev = salida.quitadas.get(vid).copied().unwrap_or(0);
            salida.quitadas.insert(vid.clone(), prev.max(t));
        }
        for (u, p) in &doc.playlists {
            let reemplaza = salida
                .playlists
                .get(u)
                .is_none_or(|prev| p.modificada > prev.modificada);
            if reemplaza {
                salida.playlists.insert(u.clone(), p.clone());
            }
        }
        for (u, &t) in &doc.borradas {
            let prev = salida.borradas.get(u).copied().unwrap_or(0);
            salida.borradas.insert(u.clone(), prev.max(t));
        }
    }

    let vids_quitadas: Vec<String> = salida.quitadas.keys().cloned().collect();
    for vid in vids_quitadas {
        if let Some(c) = salida.canciones.get(&vid) {
            if c.agregada > salida.quitadas[&vid] {
                salida.quitadas.remove(&vid);
            } else {
                salida.canciones.remove(&vid);
            }
        }
    }

    let uuids_borrados: Vec<String> = salida.borradas.keys().cloned().collect();
    for u in uuids_borrados {
        if let Some(p) = salida.playlists.get(&u) {
            if p.modificada > salida.borradas[&u] {
                salida.borradas.remove(&u);
            } else {
                salida.playlists.remove(&u);
            }
        }
    }

    salida
}

pub fn plan(doc: &DocNube, local: &Local) -> Plan {
    let mut bajar: Vec<(String, CancionNube)> = doc
        .canciones
        .iter()
        .filter(|(vid, _)| !local.canciones.contains_key(*vid))
        .map(|(vid, c)| (vid.clone(), c.clone()))
        .collect();
    bajar.sort_by_key(|(_, c)| c.agregada);

    let borrar: Vec<String> = doc
        .quitadas
        .iter()
        .filter_map(|(vid, &t)| {
            let c = local.canciones.get(vid)?;
            if c.agregada <= t { Some(vid.clone()) } else { None }
        })
        .collect();

    let playlists: Vec<(String, PlaylistNube)> = doc
        .playlists
        .iter()
        .filter(|(u, p)| match local.playlists.get(*u) {
            None => true,
            Some(loc) => loc.modificada <= p.modificada && loc != *p,
        })
        .map(|(u, p)| (u.clone(), p.clone()))
        .collect();

    let quitar_playlists: Vec<String> = doc
        .borradas
        .iter()
        .filter_map(|(u, &t)| {
            let p = local.playlists.get(u)?;
            if p.modificada <= t { Some(u.clone()) } else { None }
        })
        .collect();

    Plan {
        bajar,
        borrar,
        playlists,
        quitar_playlists,
    }
}

pub fn iguales(a: &DocNube, b: &DocNube) -> bool {
    a == b
}

/// Convierte un mapa genérico de opciones extra si hiciera falta interoperar con JSON.
pub type MapaExtra = HashMap<String, String>;
