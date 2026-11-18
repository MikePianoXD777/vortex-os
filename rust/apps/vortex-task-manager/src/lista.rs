//! La lista plana que ve QML: secciones, apps y procesos, cada fila un objeto JSON con los mismos campos
//! que las filas de `puente.py`.

use crate::grupos::{Grupo, Grupos, coincide, ordenar};
use serde_json::{Value, json};
use std::collections::HashSet;

pub const SECCIONES: [(&str, &str); 3] = [
    ("apps", "Apps"),
    ("fondo", "Segundo plano"),
    ("sistema", "Del sistema"),
];

/// Lo que cambia cómo se arma la lista (lo que la persona eligió).
pub struct Opciones<'a> {
    pub filtro: &'a str,
    pub orden: &'a str,
    pub expandidos: &'a HashSet<String>,
    pub colapsadas: &'a HashSet<String>,
}

/// Las filas de la lista. `icono` devuelve el ícono a usar para un nombre (o «» si no hay).
pub fn filas(grupos: &Grupos, o: &Opciones, icono: &mut dyn FnMut(&str) -> String) -> Vec<Value> {
    let mut filas = Vec::new();
    for (seccion, titulo) in SECCIONES {
        let del: Vec<&Grupo> = grupos
            .seccion(seccion)
            .iter()
            .filter(|g| coincide(g, o.filtro))
            .collect();
        if del.is_empty() {
            continue;
        }
        let colapsada = o.colapsadas.contains(seccion) && o.filtro.is_empty();
        filas.push(
            json!({"tipo": "seccion", "clave": format!("s:{seccion}"), "seccion": seccion, "nombre": titulo,
            "cantidad": del.len(), "colapsada": colapsada,
            "cpu": del.iter().map(|g| g.cpu).sum::<f64>(), "mem": del.iter().map(|g| g.mem).sum::<u64>(),
            "vram": del.iter().map(|g| g.vram).sum::<u64>()}),
        );
        if colapsada {
            continue;
        }
        let propios: Vec<Grupo> = del.into_iter().cloned().collect();
        for g in ordenar(&propios, o.orden) {
            let n = g.procesos.len();
            let expandido = o.expandidos.contains(&g.clave) && n > 1;
            filas.push(json!({"tipo": "grupo", "clave": g.clave, "seccion": seccion, "nombre": g.nombre,
                "icono": icono(&g.icono), "subtitulo": g.subtitulo, "cpu": g.cpu, "mem": g.mem, "disco": g.disco,
                "vram": g.vram, "n": n, "expandido": expandido, "suspendido": g.suspendido,
                "ventana": !g.ventanas.is_empty(), "usuario": g.usuario}));
            if expandido {
                for p in ordenar(&g.procesos, o.orden) {
                    filas.push(json!({"tipo": "proceso", "clave": format!("{}/{}", g.clave, p.pid), "seccion": seccion,
                        "padre": g.clave, "nombre": p.nombre, "pid": p.pid, "cpu": p.cpu, "mem": p.mem, "disco": p.disco,
                        "vram": p.vram, "usuario": p.usuario, "suspendido": p.estado == "stopped", "subtitulo": ""}));
                }
            }
        }
    }
    filas
}

/// El vistazo del lado izquierdo y de las tarjetas de arriba: quién gasta más CPU, memoria y VRAM.
/// `mi_pid` no cuenta: la app no se acusa a sí misma.
pub fn resumen(
    grupos: &Grupos,
    rend: &Value,
    vram_disponible: bool,
    mi_pid: i32,
    icono: &mut dyn FnMut(&str) -> String,
) -> Value {
    let apps: Vec<&Grupo> = grupos.todos().filter(|g| !g.pids.contains(&mi_pid)).collect();
    let mayor = |f: &dyn Fn(&Grupo) -> f64| -> Option<&Grupo> {
        // El primero de los empatados, como `max()` de Python.
        apps.iter().copied().fold(None, |m: Option<&Grupo>, g| match m {
            Some(a) if f(g) <= f(a) => Some(a),
            _ => Some(g),
        })
    };
    let top = |g: Option<&Grupo>,
               valor: &dyn Fn(&Grupo) -> Value,
               con_icono: bool,
               icono: &mut dyn FnMut(&str) -> String|
     -> Value {
        match g {
            Some(g) if con_icono => json!({"nombre": g.nombre, "valor": valor(g), "icono": icono(&g.icono)}),
            Some(g) => json!({"nombre": g.nombre, "valor": valor(g)}),
            None => json!({}),
        }
    };
    let top_cpu = mayor(&|g| g.cpu);
    let top_mem = mayor(&|g| g.mem as f64);
    let top_vram = mayor(&|g| g.vram as f64).filter(|g| g.vram > 0 && vram_disponible);
    // La tarjeta principal: la de más VRAM (en una laptop con dos, la dedicada).
    let gpu = rend["gpus"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|g| g["vramTotal"].as_f64().unwrap_or(0.0) > 0.0)
        .fold(None, |m: Option<&Value>, g| match m {
            Some(a) if g["vramTotal"].as_f64() <= a["vramTotal"].as_f64() => Some(a),
            _ => Some(g),
        });
    json!({
        "cpu": rend["cpu"]["uso"], "memUsada": rend["mem"]["usada"], "memTotal": rend["mem"]["total"],
        "topCpu": top(top_cpu, &|g| json!(g.cpu), true, icono),
        "topMem": top(top_mem, &|g| json!(g.mem), true, icono),
        "topVram": top(top_vram, &|g| json!(g.vram), false, icono),
        "gpu": match gpu {
            Some(g) => json!({"nombre": g["nombre"].as_str().unwrap_or("").replace("GeForce ", "").replace("Radeon ", ""),
                "usada": g["vramUsada"], "total": g["vramTotal"]}),
            None => json!({}),
        },
    })
}
