//! Lo que la app instaló o descomprimió, para poder deshacerlo (pestaña «Instalado») (`registro.py`).
//!
//! Se guarda en `~/.local/share/vortex-app-installer/instalado.json`: una lista de entradas con
//! `{id, tipo, nombre, fecha, rutas, icono, flatpak, archivo, abrir}`. Deshacer manda sus rutas a la papelera
//! (nunca borra para siempre) o, si es un Flatpak, lo desinstala con `flatpak --user`. Una entrada cuyas
//! rutas ya no existen (alguien las borró a mano) desaparece sola de la lista.

use crate::util::{a_ascii, abspath, existe_o_enlace};
use crate::{Contexto, Resultado, flatpak, papelera, proc};
use regex::Regex;
use serde::Serialize;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

pub const TIPOS: [&str; 5] = ["appimage", "programa", "flatpak", "mod", "descomprimido"];

/// Una entrada del registro (un objeto JSON: se conservan los campos que esta versión no conoce).
pub type Entrada = Map<String, Value>;

/// «Vortex Studio 1.0» → «vortex-studio-1.0» (para nombres de carpetas, íconos e ids).
pub fn slug(texto: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"[^A-Za-z0-9._-]+").expect("regex"));
    let t = r.replace_all(&a_ascii(texto), "-").into_owned();
    let t = t.trim_matches(|c| "-._".contains(c)).to_lowercase();
    let t: String = t.chars().take(60).collect();
    if t.is_empty() { "app".into() } else { t }
}

pub fn texto_de<'a>(e: &'a Entrada, clave: &str) -> &'a str {
    e.get(clave).and_then(Value::as_str).unwrap_or("")
}

fn rutas_de(e: &Entrada) -> Vec<String> {
    e.get("rutas")
        .and_then(Value::as_array)
        .map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default()
}

pub fn cargar(ctx: &Contexto) -> Vec<Entrada> {
    let Ok(texto) = std::fs::read_to_string(ctx.rutas.registro()) else {
        return Vec::new();
    };
    let Ok(Value::Array(datos)) = serde_json::from_str::<Value>(&texto) else {
        return Vec::new();
    };
    let lista: Vec<Entrada> = datos
        .into_iter()
        .filter_map(|d| match d {
            Value::Object(e)
                if e.get("id").is_some_and(|i| i.as_str().is_some_and(|s| !s.is_empty()))
                    && TIPOS.contains(&texto_de(&e, "tipo")) =>
            {
                Some(e)
            }
            _ => None,
        })
        .collect();
    let n = lista.len();
    let vivas: Vec<Entrada> = lista
        .into_iter()
        .filter(|e| texto_de(e, "tipo") == "flatpak" || rutas_de(e).iter().any(|r| existe_o_enlace(Path::new(r))))
        .collect();
    if vivas.len() != n {
        let _ = guardar(ctx, &vivas);
    }
    vivas
}

pub fn guardar(ctx: &Contexto, lista: &[Entrada]) -> std::io::Result<()> {
    let destino = ctx.rutas.registro();
    let carpeta = destino.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(carpeta)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".instalado-")
        .suffix(".json")
        .tempfile_in(carpeta)?;
    {
        use std::io::Write;
        let fmt = serde_json::ser::PrettyFormatter::with_indent(b" ");
        let mut ser = serde_json::Serializer::with_formatter(&mut tmp, fmt);
        lista
            .serialize(&mut ser)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        tmp.flush()?;
    }
    tmp.persist(&destino).map_err(|e| e.error)?;
    Ok(())
}

pub fn buscar(ctx: &Contexto, id: &str) -> Option<Entrada> {
    cargar(ctx).into_iter().find(|e| texto_de(e, "id") == id)
}

pub fn agregar(ctx: &Contexto, mut entrada: Entrada) -> std::io::Result<Entrada> {
    entrada
        .entry("fecha")
        .or_insert_with(|| json!(crate::util::ahora().iso()));
    entrada.entry("rutas").or_insert_with(|| json!([]));
    let id = texto_de(&entrada, "id").to_string();
    let mut lista: Vec<Entrada> = cargar(ctx).into_iter().filter(|e| texto_de(e, "id") != id).collect();
    lista.insert(0, entrada.clone());
    guardar(ctx, &lista)?;
    Ok(entrada)
}

/// Id nuevo para lo que puede repetirse (cada carpeta descomprimida, cada mod).
pub fn id_libre(ctx: &Contexto, tipo: &str, nombre: &str) -> String {
    let base = format!("{tipo}-{}", slug(nombre));
    let usados: HashSet<String> = cargar(ctx).iter().map(|e| texto_de(e, "id").to_string()).collect();
    let mut n = 1;
    let mut id = base.clone();
    while usados.contains(&id) {
        n += 1;
        id = format!("{base}-{n}");
    }
    id
}

/// Nunca la carpeta personal, ni Descargas, ni ~/Applications enteras, ni nada fuera de lo absoluto.
fn se_puede_quitar(ctx: &Contexto, ruta: &str) -> bool {
    if ruta.is_empty() || !ruta.starts_with('/') {
        return false;
    }
    let p = abspath(Path::new(ruta));
    if ctx.rutas.protegidas().contains(&p) {
        return false;
    }
    // ni /home ni /: ningún ancestro (ni la casa misma) de la carpeta personal
    !abspath(&ctx.rutas.casa).starts_with(&p)
}

/// Deshace una entrada. Devuelve la entrada quitada, o `None` si no existe.
pub fn deshacer(ctx: &Contexto, id: &str) -> Resultado<Option<Entrada>> {
    let Some(entrada) = buscar(ctx, id) else {
        return Ok(None);
    };
    if texto_de(&entrada, "tipo") == "flatpak" && !texto_de(&entrada, "flatpak").is_empty() {
        flatpak::desinstalar(ctx, texto_de(&entrada, "flatpak"))?;
    }
    for r in rutas_de(&entrada) {
        if se_puede_quitar(ctx, &r) {
            papelera::mandar(ctx, Path::new(&r))?;
        }
    }
    let resto: Vec<Entrada> = cargar(ctx).into_iter().filter(|e| texto_de(e, "id") != id).collect();
    guardar(ctx, &resto)?;
    refrescar_lanzador(ctx);
    Ok(Some(entrada))
}

/// Que el lanzador vea al momento los .desktop nuevos o quitados.
pub fn refrescar_lanzador(ctx: &Contexto) {
    let carpeta = ctx.rutas.lanzadores();
    if let Some(exe) = ctx.which("update-desktop-database")
        && carpeta.is_dir()
    {
        let mut c = std::process::Command::new(exe);
        ctx.preparar(&mut c);
        c.arg("-q").arg(&carpeta);
        let _ = proc::ejecutar(c, None, Some(Duration::from_secs(30)));
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("Vortex Studio 1.0"), "vortex-studio-1.0");
        assert_eq!(slug("¡Visualización!"), "visualizacion");
        assert_eq!(slug("日本"), "app");
        assert_eq!(slug("--x--"), "x");
        assert_eq!(slug(&"a".repeat(80)).len(), 60);
    }
}
