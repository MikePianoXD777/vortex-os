//! Buscar en Flathub el mismo programa que un .deb, un .rpm o un .exe (`flathub.py`): la API abierta de
//! Flathub, la misma que usa Vortexine's Shop. `VORTEX_APP_INSTALLER_FLATHUB` cambia la dirección (pruebas).
//!
//! La petición la hace `curl` (como la de Python hace `urllib`): sin internet o sin `curl` da [`SinRed`].

use crate::util::a_ascii;
use crate::{AGENTE, Contexto, proc};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::Duration;

/// No hay conexión (o Flathub no respondió bien).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SinRed(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct App {
    pub id: String,
    pub nombre: String,
    pub resumen: String,
    pub icono: String,
}

impl App {
    pub fn nuevo(id: &str, nombre: &str) -> App {
        App {
            id: id.into(),
            nombre: nombre.into(),
            resumen: String::new(),
            icono: String::new(),
        }
    }

    pub fn a_json(&self) -> Value {
        json!({"id": self.id, "nombre": self.nombre, "resumen": self.resumen, "icono": self.icono})
    }
}

/// `[{id, nombre, resumen, icono}]` de Flathub para ese texto. [`SinRed`] si no hay conexión.
pub fn buscar(ctx: &Contexto, texto: &str, por_pagina: u32) -> Result<Vec<App>, SinRed> {
    let base = ctx.flathub.trim_end_matches('/');
    let cuerpo =
        json!({"query": texto.chars().take(100).collect::<String>(), "hits_per_page": por_pagina, "page": 1});
    let Some(curl) = ctx.which("curl") else {
        return Err(SinRed("curl no está instalado".into()));
    };
    let mut c = std::process::Command::new(curl);
    ctx.preparar(&mut c);
    c.args([
        "-sS",
        "-f",
        "--max-time",
        "12",
        "-X",
        "POST",
        "-H",
        "Content-Type: application/json",
        "-H",
        &format!("User-Agent: {AGENTE}"),
        "--data-binary",
        "@-",
        &format!("{base}/search?locale={}", ctx.idioma.codigo()),
    ]);
    let r = proc::ejecutar(
        c,
        Some(cuerpo.to_string().as_bytes()),
        Some(Duration::from_secs(15)),
    )
    .map_err(|e| SinRed(e.to_string()))?;
    if !r.ok() {
        return Err(SinRed(r.texto_err()));
    }
    let datos: Value = serde_json::from_slice(&r.stdout).map_err(|e| SinRed(e.to_string()))?;
    let mut salida = Vec::new();
    if let Some(hits) = datos.get("hits").and_then(Value::as_array) {
        for hit in hits {
            let Some(app_id) = hit.get("app_id").and_then(Value::as_str) else {
                continue;
            };
            if matches!(hit.get("type").and_then(Value::as_str), Some("runtime" | "addon")) {
                continue;
            }
            let t = |k: &str, defecto: &str| {
                hit.get(k)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or(defecto)
                    .to_string()
            };
            salida.push(App {
                id: app_id.into(),
                nombre: t("name", app_id),
                resumen: t("summary", ""),
                icono: t("icon", ""),
            });
        }
    }
    Ok(salida)
}

fn norma(t: &str) -> String {
    a_ascii(t)
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect()
}

const SOBRAS: [&str; 9] = [
    "stable", "beta", "nightly", "bin", "desktop", "linux", "amd64", "x8664", "app",
];

fn raiz(nombre: &str) -> String {
    let mut n = norma(nombre);
    let mut cambio = true;
    while cambio {
        cambio = false;
        for s in SOBRAS {
            if n.ends_with(s) && n.len() > s.len() + 2 {
                n.truncate(n.len() - s.len());
                cambio = true;
            }
        }
    }
    n
}

fn de_prueba() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)unstable|beta|nightly|canary|\bdev\b|preview|insiders|\bgit\b")
            .expect("regex de pruebas")
    })
}

/// «google-chrome-stable» → «google chrome»: lo que se le pregunta a Flathub.
pub fn para_buscar(nombre_paquete: &str) -> String {
    let mut partes: Vec<&str> = nombre_paquete
        .split(|c: char| c == '-' || c == '_' || c == '.' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .collect();
    while partes.len() > 1 && partes.last().is_some_and(|u| SOBRAS.contains(&norma(u).as_str())) {
        partes.pop();
    }
    if partes.is_empty() {
        nombre_paquete.into()
    } else {
        partes.join(" ")
    }
}

/// «Google Chrome (unstable)» → «Google Chrome».
fn nombre_limpio(nombre: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\([^)]*\)").expect("regex"))
        .replace_all(nombre, "")
        .into_owned()
}

/// El resultado que es el mismo programa («google-chrome-stable» ↔ «Google Chrome»), o `None`. Una versión
/// de prueba (beta, unstable, dev…) solo vale si el paquete también lo es.
pub fn mejor<'a>(nombre_paquete: &str, resultados: &'a [App]) -> Option<&'a App> {
    let objetivo = raiz(nombre_paquete);
    if objetivo.chars().count() < 2 {
        return None;
    }
    let es_prueba = de_prueba().is_match(&nombre_paquete.replace('-', " "));
    let ultimo = |id: &str| id.rsplit('.').next().unwrap_or("").to_string();
    let validos: Vec<&App> = resultados
        .iter()
        .filter(|r| es_prueba || !(de_prueba().is_match(&r.nombre) || de_prueba().is_match(&ultimo(&r.id))))
        .collect();
    for r in &validos {
        let candidatos = [raiz(&nombre_limpio(&r.nombre)), raiz(&ultimo(&r.id))];
        if candidatos.contains(&objetivo) {
            return Some(r);
        }
    }
    for r in &validos {
        let nombre = raiz(&nombre_limpio(&r.nombre));
        if nombre.chars().count() >= 4 && (objetivo.contains(&nombre) || nombre.contains(&objetivo)) {
            return Some(r);
        }
    }
    None
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hits() -> Vec<App> {
        vec![
            App::nuevo("com.google.Chrome", "Google Chrome"),
            App::nuevo("com.visualstudio.code", "Visual Studio Code"),
            App::nuevo("com.discordapp.Discord", "Discord"),
        ]
    }

    #[test]
    fn flathub_empareja_bien() {
        let h = hits();
        assert_eq!(mejor("google-chrome-stable", &h).unwrap().id, "com.google.Chrome");
        assert_eq!(mejor("code", &h).unwrap().id, "com.visualstudio.code");
        assert_eq!(mejor("Discord", &h).unwrap().id, "com.discordapp.Discord");
        assert!(mejor("libreoffice", &h).is_none());
        let mut dev = vec![App::nuevo("com.google.ChromeDev", "Google Chrome (unstable)")];
        dev.extend(hits());
        assert_eq!(
            mejor("google-chrome-stable", &dev).unwrap().id,
            "com.google.Chrome"
        );
        assert_eq!(
            mejor("google-chrome-unstable", &dev).unwrap().id,
            "com.google.ChromeDev"
        );
        assert!(mejor("google-chrome-stable", &dev[..1]).is_none());
        assert_eq!(para_buscar("google-chrome-stable"), "google chrome");
        assert_eq!(para_buscar("obs-studio"), "obs studio");
    }
}
