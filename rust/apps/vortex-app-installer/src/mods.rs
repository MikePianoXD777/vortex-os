//! Mods de Minecraft (.jar de Fabric, Quilt, Forge o NeoForge) (`mods.py`).
//!
//! Si existe el comando «vortex mods» (de Vortexine's OS), la app le pide las instancias de Minecraft
//! («vortex mods instancias») y le pasa el mod («vortex mods mover ARCHIVO RUTA»). Si no, la persona elige la
//! carpeta a mano. Los .jar se leen con `bsdtar` (un .jar es un zip).

use crate::comprimidos::nombre_libre;
use crate::util::{existe_o_enlace, nombre_de, raiz_de_nombre};
use crate::{Contexto, Error, Resultado, proc};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn bsdtar(ctx: &Contexto, args: &[&str]) -> Option<Vec<u8>> {
    let exe = ctx.which("bsdtar")?;
    let mut c = std::process::Command::new(exe);
    ctx.preparar(&mut c);
    c.args(args).env("LC_ALL", "C.UTF-8").env("LANG", "C.UTF-8");
    let s = proc::ejecutar(c, None, Some(Duration::from_secs(60))).ok()?;
    s.ok().then_some(s.stdout)
}

/// `str(valor)` de Python, para lo poco que importa aquí.
fn como_texto(v: Option<&Value>) -> String {
    match v {
        None => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "None".into(),
        Some(Value::Bool(b)) => if *b { "True" } else { "False" }.into(),
        Some(otro) => otro.to_string(),
    }
}

/// `json.loads(…, strict=False)`: deja pasar saltos de línea y tabuladores dentro de los textos.
fn json_flexible(texto: &str) -> Option<Value> {
    if let Ok(v) = serde_json::from_str(texto) {
        return Some(v);
    }
    let mut r = String::with_capacity(texto.len());
    let (mut dentro, mut escape) = (false, false);
    for c in texto.chars() {
        if dentro && !escape && (c as u32) < 0x20 {
            r.push_str(&format!("\\u{:04x}", c as u32));
            continue;
        }
        if escape {
            escape = false;
        } else if dentro && c == '\\' {
            escape = true;
        } else if c == '"' {
            dentro = !dentro;
        }
        r.push(c);
    }
    serde_json::from_str(&r).ok()
}

/// Lo que importa de un `mods.toml`: la primera tabla `[[mods]]` y sus textos entre comillas.
fn primer_mod_toml(texto: &str) -> Map<String, Value> {
    let mut mod_ = Map::new();
    let mut dentro = false;
    for linea in texto.lines() {
        let l = linea.trim();
        if l.starts_with('[') {
            if dentro {
                break;
            }
            dentro = l == "[[mods]]";
            continue;
        }
        if dentro && let Some((k, v)) = l.split_once('=') {
            let v = v.trim();
            let comilla = v.chars().next();
            if let Some(c @ ('"' | '\'')) = comilla {
                let resto = &v[1..];
                if let Some(fin) = resto.find(c) {
                    mod_.insert(k.trim().into(), json!(&resto[..fin]));
                }
            }
        }
    }
    mod_
}

/// `{nombre, id, version, loader, minecraft}` si el .jar es un mod; `None` si es otro programa de Java.
pub fn leer_mod(ctx: &Contexto, ruta: &Path) -> Option<Map<String, Value>> {
    let r = ruta.to_string_lossy();
    let lista = bsdtar(ctx, &["-tf", &r])?;
    let nombres: Vec<String> = String::from_utf8_lossy(&lista)
        .lines()
        .map(str::to_string)
        .collect();
    let hay = |n: &str| nombres.iter().any(|x| x == n);
    let stem = raiz_de_nombre(&nombre_de(ruta)).to_string();
    let leer = |n: &str| bsdtar(ctx, &["-xOf", &r, n]).map(|b| String::from_utf8_lossy(&b).into_owned());
    if hay("fabric.mod.json") || hay("quilt.mod.json") {
        let quilt = hay("quilt.mod.json") && !hay("fabric.mod.json");
        let datos = json_flexible(&leer(if quilt {
            "quilt.mod.json"
        } else {
            "fabric.mod.json"
        })?)?;
        let datos = datos.as_object()?;
        if quilt {
            let meta = datos
                .get("quilt_loader")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let nombre = meta
                .get("metadata")
                .and_then(|m| m.get("name"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| meta.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()))
                .unwrap_or(&stem);
            return Some(Map::from_iter([
                ("nombre".into(), json!(nombre)),
                ("id".into(), json!(como_texto(meta.get("id")))),
                ("version".into(), json!(como_texto(meta.get("version")))),
                ("loader".into(), json!("quilt")),
                ("minecraft".into(), json!("")),
            ]));
        }
        let depende = datos.get("depends").and_then(Value::as_object);
        let mc = match depende.and_then(|d| d.get("minecraft")) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(v)) => v
                .iter()
                .map(|x| como_texto(Some(x)))
                .collect::<Vec<_>>()
                .join(", "),
            _ => String::new(),
        };
        let nombre = datos
            .get("name")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .or_else(|| datos.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()))
            .unwrap_or(&stem);
        return Some(Map::from_iter([
            ("nombre".into(), json!(nombre)),
            ("id".into(), json!(como_texto(datos.get("id")))),
            ("version".into(), json!(como_texto(datos.get("version")))),
            ("loader".into(), json!("fabric")),
            ("minecraft".into(), json!(mc)),
        ]));
    }
    for (archivo, loader) in [
        ("META-INF/neoforge.mods.toml", "neoforge"),
        ("META-INF/mods.toml", "forge"),
    ] {
        if hay(archivo) {
            let mod_ = primer_mod_toml(&leer(archivo)?);
            let version = como_texto(mod_.get("version"));
            let texto = |k: &str| mod_.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
            return Some(Map::from_iter([
                (
                    "nombre".into(),
                    json!(texto("displayName").or(texto("modId")).unwrap_or(&stem)),
                ),
                ("id".into(), json!(como_texto(mod_.get("modId")))),
                (
                    "version".into(),
                    json!(if version.starts_with('$') {
                        String::new()
                    } else {
                        version
                    }),
                ),
                ("loader".into(), json!(loader)),
                ("minecraft".into(), json!("")),
            ]));
        }
    }
    None
}

/// Las instancias de Minecraft según «vortex mods instancias», o `None` si ese comando no existe.
pub fn instancias(ctx: &Contexto) -> Option<Vec<Value>> {
    let exe = ctx.which("vortex")?;
    let mut c = std::process::Command::new(exe);
    ctx.preparar(&mut c);
    c.args(["mods", "instancias"]);
    let r = proc::ejecutar(c, None, Some(Duration::from_secs(30))).ok()?;
    if !r.ok() {
        return None;
    }
    let texto = r.texto_out();
    let texto = texto.trim();
    // una lista JSON o una instancia JSON por renglón
    let datos: Vec<Value> = if texto.starts_with('[') {
        serde_json::from_str(texto).ok()?
    } else {
        texto
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).ok())
            .collect::<Option<Vec<_>>>()?
    };
    Some(
        datos
            .iter()
            .filter_map(Value::as_object)
            .filter(|d| {
                d.get("ruta")
                    .is_some_and(|r| !matches!(r, Value::Null) && r != &json!("") && r != &json!(0))
            })
            .map(|d| {
                let t = |k: &str| json!(como_texto_vacio(d.get(k)));
                json!({"nombre": t("nombre"), "launcher": t("launcher"), "ruta": t("ruta"),
                       "version": t("version"), "loader": t("loader")})
            })
            .collect(),
    )
}

fn como_texto_vacio(v: Option<&Value>) -> String {
    match v {
        None => String::new(),
        Some(v) => como_texto(Some(v)),
    }
}

/// Pasa el mod a la instancia con «vortex mods mover». Devuelve dónde quedó.
pub fn mover(ctx: &Contexto, archivo: &Path, ruta_instancia: &str) -> Resultado<PathBuf> {
    let Some(exe) = ctx.which("vortex") else {
        return Err(Error::Mensaje(ctx.tr("mods_sin_comando", &[])));
    };
    let destino = Path::new(ruta_instancia).join("mods").join(nombre_de(archivo));
    if existe_o_enlace(&destino) {
        return Err(Error::Conflicto {
            sugerida: nombre_libre(&destino),
            ruta: destino,
        });
    }
    let mut c = std::process::Command::new(exe);
    ctx.preparar(&mut c);
    c.args(["mods", "mover", &archivo.to_string_lossy(), ruta_instancia]);
    let r = proc::ejecutar(c, None, Some(Duration::from_secs(60)))?;
    if !r.ok() {
        return Err(Error::Mensaje(ctx.tr("mods_no_se_movio", &[])));
    }
    let texto = r.texto_out();
    let ultima = texto.trim().lines().rfind(|l| !l.trim().is_empty());
    Ok(ultima.map_or(destino, PathBuf::from))
}

/// Copia el mod a la carpeta que eligió la persona (a su «mods/» si eligió la del juego).
pub fn copiar_a_carpeta(archivo: &Path, carpeta: &Path, nombre: Option<&str>) -> Resultado<PathBuf> {
    let mut carpeta = carpeta.to_path_buf();
    if carpeta.join("mods").is_dir() && nombre_de(&carpeta) != "mods" {
        carpeta = carpeta.join("mods");
    }
    let destino = carpeta.join(
        nombre
            .filter(|n| !n.is_empty())
            .map_or_else(|| nombre_de(archivo), str::to_string),
    );
    if existe_o_enlace(&destino) {
        return Err(Error::Conflicto {
            sugerida: nombre_libre(&destino),
            ruta: destino,
        });
    }
    std::fs::create_dir_all(&carpeta)?;
    std::fs::copy(archivo, &destino)?;
    Ok(destino)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn toml_de_forge() {
        let t = "modLoader=\"javafml\"\n[[mods]]\nmodId=\"jei\"\nversion=\"${file.jarVersion}\"\ndisplayName='Just Enough Items'\n[[dependencies.jei]]\nmodId=\"otro\"\n";
        let m = primer_mod_toml(t);
        assert_eq!(m["modId"], "jei");
        assert_eq!(m["displayName"], "Just Enough Items");
    }

    #[test]
    fn json_con_saltos_dentro_de_un_texto() {
        let v = json_flexible("{\"name\": \"a\nb\"}").unwrap();
        assert_eq!(v["name"], "a\nb");
    }
}
