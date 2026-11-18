//! Servicios de systemd para la página «Servicios»: los tuyos (`systemctl --user`) y los del sistema.
//!
//! De cada servicio: si está corriendo, si falló y si arranca solo al prender («Al iniciar»: enabled o
//! disabled; los static, generated, masked… no se pueden cambiar desde aquí). Las acciones sobre los del
//! sistema las autoriza polkit (el shell te pregunta tu contraseña).

use crate::ejecutar;
use serde_json::{Value, json};
use std::time::Duration;

pub const CAMBIABLES: [&str; 2] = ["enabled", "disabled"];
pub const ACCIONES: [&str; 5] = ["start", "stop", "restart", "enable", "disable"];

fn systemctl(usuario: bool, args: &[&str], limite: u64) -> Option<ejecutar::Salida> {
    let mut todos = Vec::new();
    if usuario {
        todos.push("--user");
    }
    todos.extend_from_slice(args);
    ejecutar::con_limite("systemctl", &todos, Duration::from_secs(limite))
}

fn json_de(usuario: bool, args: &[&str]) -> Vec<Value> {
    let mut a = args.to_vec();
    a.extend(["-o", "json", "--no-pager"]);
    let Some(r) = systemctl(usuario, &a, 10) else {
        return Vec::new();
    };
    match serde_json::from_str::<Value>(if r.stdout.trim().is_empty() {
        "[]"
    } else {
        &r.stdout
    }) {
        Ok(Value::Array(v)) => v,
        _ => Vec::new(),
    }
}

fn texto<'a>(v: &'a Value, k: &str) -> &'a str {
    v[k].as_str().unwrap_or("")
}

/// `list-units` (lo que corre) + `list-unit-files` (lo que arranca al prender) → filas ordenadas.
pub fn unir(unidades: &[Value], archivos: &[Value]) -> Vec<Value> {
    // Un mapa que recuerda el orden en que llegó cada unidad.
    let mut nombres: Vec<String> = Vec::new();
    let mut filas: Vec<Value> = Vec::new();
    for a in archivos {
        let nombre = texto(a, "unit_file");
        if !nombre.ends_with(".service") || nombre.ends_with("@.service") {
            continue;
        }
        if !nombres.iter().any(|n| n == nombre) {
            nombres.push(nombre.to_string());
            filas.push(json!({"unidad": nombre, "descripcion": "", "activo": "inactive", "sub": "dead", "alInicio": texto(a, "state")}));
        }
    }
    for u in unidades {
        let nombre = texto(u, "unit");
        if !nombre.ends_with(".service") || texto(u, "load") == "not-found" {
            continue;
        }
        let i = match nombres.iter().position(|n| n == nombre) {
            Some(i) => i,
            None => {
                nombres.push(nombre.to_string());
                filas.push(json!({"unidad": nombre, "alInicio": ""}));
                filas.len() - 1
            }
        };
        let activo = Some(texto(u, "active"))
            .filter(|s| !s.is_empty())
            .unwrap_or("inactive");
        filas[i]["descripcion"] = json!(texto(u, "description"));
        filas[i]["activo"] = json!(activo);
        filas[i]["sub"] = json!(texto(u, "sub"));
    }
    let mut salida = Vec::new();
    for mut f in filas {
        let al_inicio = texto(&f, "alInicio").to_string();
        let descripcion = texto(&f, "descripcion").to_string();
        let unidad = texto(&f, "unidad").to_string();
        if al_inicio.is_empty() && (descripcion.is_empty() || descripcion == unidad) {
            continue; // pasajeros (dbus-:1.14-…@0.service): sin archivo ni descripción, no se manejan
        }
        f["nombre"] = json!(unidad.strip_suffix(".service").unwrap_or(&unidad));
        f["cambiable"] = json!(CAMBIABLES.contains(&al_inicio.as_str()));
        salida.push(f);
    }
    // Primero los que fallaron, luego los que corren y luego los demás; cada grupo por nombre.
    let peso = |f: &Value| match texto(f, "activo") {
        "failed" => 0,
        "active" | "activating" | "deactivating" | "reloading" => 1,
        _ => 2,
    };
    salida.sort_by_cached_key(|f| (peso(f), texto(f, "nombre").to_lowercase()));
    salida
}

pub fn listar(usuario: bool) -> Vec<Value> {
    unir(
        &json_de(usuario, &["list-units", "--type=service", "--all"]),
        &json_de(usuario, &["list-unit-files", "--type=service"]),
    )
}

/// start/stop/restart/enable/disable. `None` si salió bien; si no, el mensaje de systemctl.
pub fn accion(unidad: &str, que: &str, usuario: bool) -> Option<String> {
    if !ACCIONES.contains(&que) || !unidad.ends_with(".service") || unidad.starts_with('-') {
        return Some("Acción no válida.".into());
    }
    let Some(r) = systemctl(usuario, &[que, unidad], 90) else {
        return Some(format!("systemctl tardó demasiado con {unidad}."));
    };
    if r.codigo == 0 {
        return None;
    }
    Some(
        r.stderr
            .trim()
            .lines()
            .last()
            .map_or_else(|| format!("No se pudo ({que} {unidad})."), String::from),
    )
}
