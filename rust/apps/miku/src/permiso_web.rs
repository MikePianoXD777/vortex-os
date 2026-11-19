//! Hook de Claude Code (PreToolUse) para la búsqueda web y la lectura de páginas de Miku (`miku-aux hook-web`).
//!
//! Claude Code lo corre antes de cada WebSearch o WebFetch. Pide permiso en la ventana de Miku por el mismo
//! socket que las acciones (`VORTEX_MIKU_AUTORIZAR`), mostrando la búsqueda o la dirección exacta: así una
//! página no puede hacer que Miku mande tus datos a otra dirección a escondidas. Contesta «allow» solo con
//! tu Autorizar; con Cancelar, sin respuesta o si algo falla, «deny».

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const ESPERA: u64 = 130;

fn pedir(titulo: &str, detalle: &str) -> bool {
    let ruta = std::env::var("VORTEX_MIKU_AUTORIZAR").unwrap_or_default();
    if ruta.is_empty() {
        return false;
    }
    let Ok(mut s) = UnixStream::connect(&ruta) else {
        return false;
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(ESPERA)));
    if writeln!(
        s,
        "{}",
        json!({"tipo": "pedir", "titulo": titulo, "detalle": detalle})
    )
    .is_err()
    {
        return false;
    }
    let mut l = String::new();
    if BufReader::new(s).read_line(&mut l).is_err() {
        return false;
    }
    serde_json::from_str::<Value>(&l)
        .map(|v| v["ok"].as_bool() == Some(true))
        .unwrap_or(false)
}

/// Lo que se le contesta a Claude Code para este pedido (JSON de `hookSpecificOutput`).
pub fn decidir_pedido(entrada: &str, pedir_permiso: &dyn Fn(&str, &str) -> bool) -> Value {
    let salida = |permitir: bool, motivo: &str| json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "permissionDecision": if permitir { "allow" } else { "deny" }, "permissionDecisionReason": motivo}});
    let Ok(datos) = serde_json::from_str::<Value>(entrada) else {
        return salida(false, "No entendí qué quería usar Claude.");
    };
    let herramienta = datos["tool_name"].as_str().unwrap_or("");
    let e = &datos["tool_input"];
    let (titulo, detalle) = match herramienta {
        "WebSearch" => (
            "Buscar en internet",
            format!("Búsqueda: {}", e["query"].as_str().unwrap_or("").trim()),
        ),
        "WebFetch" => (
            "Leer una página web",
            e["url"].as_str().unwrap_or("").trim().to_string(),
        ),
        otra => return salida(false, &format!("Miku no usa {otra}.")),
    };
    if pedir_permiso(titulo, &detalle) {
        return salida(true, "La persona lo autorizó en la ventana de Miku.");
    }
    salida(
        false,
        "La persona NO lo autorizó (o no contestó). No lo intentes de otra forma.",
    )
}

pub fn principal() {
    let mut entrada = String::new();
    let _ = std::io::stdin().read_to_string(&mut entrada);
    println!("{}", decidir_pedido(&entrada, &pedir));
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn solo_con_tu_clic() {
        let si = |_: &str, _: &str| true;
        let no = |_: &str, _: &str| false;
        let b = r#"{"tool_name": "WebSearch", "tool_input": {"query": "clima"}}"#;
        assert_eq!(
            decidir_pedido(b, &si)["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert_eq!(
            decidir_pedido(b, &no)["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        assert_eq!(
            decidir_pedido("no es json", &si)["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
        let otra = r#"{"tool_name": "Bash", "tool_input": {}}"#;
        assert_eq!(
            decidir_pedido(otra, &si)["hookSpecificOutput"]["permissionDecision"],
            "deny",
            "otra herramienta nunca"
        );
        let visto = std::cell::RefCell::new(String::new());
        let ver = |t: &str, d: &str| {
            *visto.borrow_mut() = format!("{t}|{d}");
            true
        };
        decidir_pedido(
            r#"{"tool_name": "WebFetch", "tool_input": {"url": "https://x.org"}}"#,
            &ver,
        );
        assert_eq!(*visto.borrow(), "Leer una página web|https://x.org");
    }
}
