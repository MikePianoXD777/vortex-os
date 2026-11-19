//! Los chats guardados: un JSON por conversación en `datos/chats/<id>.json` (igual que `conversaciones.py`).
//!
//! Una conversación:
//! ```text
//! {"id", "titulo", "modelo", "creado", "modificado",
//!  "mensajes": [{"role": "user"|"assistant"|"system", "content", …opcionales: "pensamiento", "citas",
//!                "modelo", "fecha"}],
//!  "vault": bool}   // opcional: true si fue «Preguntar a mi vault»
//! ```
//! Es un `serde_json::Value`, como el diccionario de Python: lo que la app no conoce se conserva.

use crate::tiempo;
use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

fn id_valido(id: &str) -> bool {
    (1..=80).contains(&id.chars().count())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn archivo(carpeta: &Path, id: &str) -> Result<PathBuf, String> {
    if !id_valido(id) {
        return Err(format!("«{id}» no es un id de chat válido."));
    }
    let _ = std::fs::create_dir_all(carpeta);
    Ok(carpeta.join(format!("{id}.json")))
}

fn nuevo_id() -> String {
    let b: [u8; 3] = rand::random();
    format!(
        "{}-{:02x}{:02x}{:02x}",
        tiempo::sello(tiempo::ahora()),
        b[0],
        b[1],
        b[2]
    )
}

/// Una conversación vacía (todavía sin guardar).
pub fn nueva(modelo: &str, titulo: &str) -> Value {
    let ahora = tiempo::ahora();
    json!({"id": nuevo_id(), "titulo": titulo, "modelo": modelo, "creado": ahora, "modificado": ahora,
           "mensajes": []})
}

/// Los chats guardados, el más reciente primero, sin los mensajes (con «n_mensajes»).
pub fn lista(carpeta: &Path) -> Vec<Value> {
    let _ = std::fs::create_dir_all(carpeta);
    let mut salida = Vec::new();
    let Ok(entradas) = std::fs::read_dir(carpeta) else {
        return salida;
    };
    for e in entradas.flatten() {
        let ruta = e.path();
        if ruta.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let Some(Value::Object(conv)) = std::fs::read_to_string(&ruta)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            continue;
        };
        let n = conv
            .get("mensajes")
            .and_then(Value::as_array)
            .map(|m| {
                m.iter()
                    .filter(|m| m.is_object() && m.get("role").and_then(Value::as_str) != Some("system"))
                    .count()
            })
            .unwrap_or(0);
        let mut resumen = conv.clone();
        resumen.remove("mensajes");
        resumen.insert(
            "id".into(),
            Value::String(ruta.file_stem().unwrap_or_default().to_string_lossy().to_string()),
        );
        resumen.entry("titulo").or_insert_with(|| json!("Chat"));
        resumen.entry("modificado").or_insert_with(|| {
            let t = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0.0, |d| d.as_secs_f64());
            json!(t)
        });
        resumen.insert("n_mensajes".into(), json!(n));
        salida.push(Value::Object(resumen));
    }
    let mod_de = |c: &Value| c.get("modificado").and_then(Value::as_f64).unwrap_or(0.0);
    salida.sort_by(|a, b| mod_de(b).total_cmp(&mod_de(a)));
    salida
}

/// La conversación completa, o `None` si no existe (o el archivo está roto).
pub fn abrir(carpeta: &Path, id: &str) -> Option<Value> {
    let ruta = archivo(carpeta, id).ok()?;
    let Value::Object(mut conv) = serde_json::from_str(&std::fs::read_to_string(ruta).ok()?).ok()? else {
        return None;
    };
    conv.insert("id".into(), json!(id));
    conv.entry("mensajes").or_insert_with(|| json!([]));
    Some(Value::Object(conv))
}

/// Como `json.dump(…, ensure_ascii=False, indent=1)`.
fn a_texto(conv: &Value) -> String {
    let mut sal = Vec::new();
    let formato = serde_json::ser::PrettyFormatter::with_indent(b" ");
    let mut ser = serde_json::Serializer::with_formatter(&mut sal, formato);
    if conv.serialize(&mut ser).is_err() {
        return conv.to_string();
    }
    String::from_utf8(sal).unwrap_or_default()
}

/// Guarda la conversación (escritura atómica) y regresa su id. Le pone id, fechas y título si faltan.
pub fn guardar(carpeta: &Path, conv: &mut Value) -> std::io::Result<String> {
    let obj = conv
        .as_object_mut()
        .ok_or_else(|| std::io::Error::other("la conversación no es un objeto"))?;
    obj.entry("mensajes").or_insert_with(|| json!([]));
    if obj.get("id").and_then(Value::as_str).is_none_or(str::is_empty) {
        obj.insert("id".into(), json!(nuevo_id()));
    }
    let ahora = tiempo::ahora();
    obj.entry("creado").or_insert(json!(ahora));
    obj.insert("modificado".into(), json!(ahora));
    if obj
        .get("titulo")
        .and_then(Value::as_str)
        .is_none_or(str::is_empty)
    {
        let primero = obj
            .get("mensajes")
            .and_then(Value::as_array)
            .and_then(|m| {
                m.iter()
                    .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
            })
            .and_then(|m| m.get("content").and_then(Value::as_str))
            .unwrap_or("")
            .to_string();
        obj.insert("titulo".into(), json!(titulo_automatico(&primero)));
    }
    let id = obj["id"].as_str().unwrap_or("").to_string();
    let destino = archivo(carpeta, &id).map_err(std::io::Error::other)?;
    let mut temporal = tempfile::Builder::new()
        .prefix(".")
        .suffix(".tmp")
        .tempfile_in(destino.parent().unwrap_or(carpeta))?;
    temporal.write_all(a_texto(conv).as_bytes())?;
    temporal.persist(&destino).map_err(|e| e.error)?;
    Ok(id)
}

/// Borra un chat (si no existe, no pasa nada). Un id raro (`../../x`) es un error.
pub fn borrar(carpeta: &Path, id: &str) -> Result<(), String> {
    match std::fs::remove_file(archivo(carpeta, id)?) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn re(patron: &str) -> Regex {
    Regex::new(patron).unwrap()
}

/// Un título corto con las primeras palabras del primer mensaje («Chat nuevo» si no hay).
pub fn titulo_automatico(primer_mensaje: &str) -> String {
    static CODIGO: LazyLock<Regex> = LazyLock::new(|| re(r"(?s)```.*?(?:```|\z)"));
    static WIKI: LazyLock<Regex> = LazyLock::new(|| re(r"!?\[\[(?:[^\]|]*\|)?([^\]]*)\]\]"));
    static LIGA: LazyLock<Regex> = LazyLock::new(|| re(r"\[([^\]]*)\]\([^)]*\)"));
    static MARCAS: LazyLock<Regex> = LazyLock::new(|| re(r"[#*_`>~]+"));
    let texto = CODIGO.replace_all(primer_mensaje, " ");
    let texto = WIKI.replace_all(&texto, "${1}"); // [[nota|texto]] → texto
    let texto = LIGA.replace_all(&texto, "${1}"); // [texto](url) → texto
    let texto = MARCAS.replace_all(&texto, " ");
    let texto = texto.split_whitespace().collect::<Vec<_>>().join(" ");
    if texto.is_empty() {
        return "Chat nuevo".into();
    }
    let quitar = [' ', ',', '.', ';', ':', '¿', '?', '¡', '!', '-'];
    let palabras: Vec<&str> = texto.split(' ').collect();
    let mut titulo = palabras.iter().take(6).copied().collect::<Vec<_>>().join(" ");
    if titulo.chars().count() > 48 {
        let corte: String = titulo.chars().take(48).collect();
        titulo = match corte.rsplit_once(' ') {
            Some((a, _)) if !a.is_empty() => a.to_string(),
            _ => corte,
        };
    }
    let mut titulo = titulo.trim_end_matches(&quitar[..]).to_string();
    let texto_corto = texto.trim_end_matches(&quitar[..]);
    if palabras.len() > 6 || titulo.chars().count() < texto_corto.chars().count() {
        titulo.push('…');
    }
    let titulo = titulo.trim_start_matches(&['¿', '¡', ' '][..]);
    let mut c = titulo.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + c.as_str(),
        None => "Chat nuevo".into(),
    }
}

fn cita(texto: &str) -> String {
    texto
        .trim()
        .split('\n')
        .map(|l| {
            if l.is_empty() {
                ">".to_string()
            } else {
                format!("> {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// El chat en Markdown para guardarlo en el vault (con frontmatter y el pensamiento plegado).
pub fn a_nota(conv: &Value) -> String {
    let texto = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let titulo = Some(texto(conv, "titulo"))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Chat".into());
    let modelo = texto(conv, "modelo");
    let mut lineas: Vec<String> = vec!["---".into(), "tags: [chat, vortex-local-ai]".into()];
    if !modelo.is_empty() {
        lineas.push(format!("modelo: \"{modelo}\""));
    }
    if let Some(c) = conv.get("creado").and_then(Value::as_f64) {
        lineas.push(format!("fecha: {}", tiempo::fecha_hora(c)));
    }
    lineas.extend(["---".into(), String::new(), format!("# {titulo}"), String::new()]);
    for m in conv
        .get("mensajes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if !m.is_object() || m.get("role").and_then(Value::as_str) == Some("system") {
            continue;
        }
        if m.get("role").and_then(Value::as_str) == Some("user") {
            lineas.extend(["## Tú".into(), String::new()]);
        } else {
            let quien = Some(texto(m, "modelo"))
                .filter(|q| !q.is_empty())
                .unwrap_or(modelo.clone());
            lineas.push(if quien.is_empty() {
                "## IA".into()
            } else {
                format!("## IA ({quien})")
            });
            lineas.push(String::new());
        }
        let pens = texto(m, "pensamiento");
        if !pens.is_empty() {
            lineas.extend(["> [!note]- Pensamiento".into(), cita(&pens), String::new()]);
        }
        lineas.push(texto(m, "content").trim().to_string());
        lineas.push(String::new());
        let citas: Vec<String> = m
            .get("citas")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_str().filter(|c| !c.is_empty()))
            .map(|c| {
                let nombre = Path::new(c)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                format!("[[{nombre}]]")
            })
            .collect();
        if !citas.is_empty() {
            lineas.push(format!("*Fuentes: {}*", citas.join(", ")));
            lineas.push(String::new());
        }
    }
    lineas.join("\n").trim_end().to_string() + "\n"
}
