//! `vortex-miku ia …`: con qué IA piensa Miku (Claude, la local, Gemini, ChatGPT u otra compatible), su clave y
//! bajar la IA local.

use super::{Ctx, error, proc};
use crate::config::{Config, IAS};
use crate::ia::{self, Clave, Ia};
use serde_json::json;
use std::io::{IsTerminal, Write};
use std::sync::atomic::AtomicBool;

fn pedir_clave(p: &ia::Proveedor) -> String {
    println!(
        "Tu clave de {}: {}.",
        p.nombre,
        if p.donde.is_empty() {
            "la de tu servicio"
        } else {
            &p.donde
        }
    );
    crate::sistema::leer_secreto("Pégala aquí (no se ve al escribir) y Enter: ")
}

fn bajar_ia_local(motor: &Ia, modelo: &str) -> i32 {
    println!(
        "Bajando la IA local «{modelo}» (una sola vez, ~{})…",
        ia::TAMANO_LOCAL
    );
    let progreso = |detalle: &str, fraccion: f64| {
        print!("\r  {:5.1} %  {detalle}      ", fraccion * 100.0);
        let _ = std::io::stdout().flush();
    };
    match motor.bajar_modelo(modelo, &progreso, &AtomicBool::new(false)) {
        Ok(()) => {
            println!("\nLista: Miku ya puede pensar en tu PC, sin internet.");
            0
        }
        Err(e) => {
            println!();
            error(&e.to_string(), 1)
        }
    }
}

fn donde_quedo(donde: &str) -> &str {
    if donde == "llavero" { "tu llavero" } else { donde }
}

pub fn ia(cx: &Ctx, args: &[String]) -> i32 {
    let cfg = cx.cfg();
    let motor = Ia::nueva(cx.rutas.clone());
    let p = motor.elegido(&cfg);
    if args.is_empty() || args[0] == "estado" {
        println!("ia={}", p.id);
        if p.id != "claude" {
            println!(
                "  {}: modelo «{}» en {}",
                p.nombre,
                if p.modelo.is_empty() {
                    "(sin poner)"
                } else {
                    &p.modelo
                },
                if p.url.is_empty() {
                    "(sin dirección)"
                } else {
                    &p.url
                }
            );
        }
        if p.clave != Clave::No {
            println!(
                "  clave: {}",
                if motor.leer_clave(&p.id).is_empty() {
                    "(sin poner: vortex-miku ia clave)"
                } else {
                    "guardada"
                }
            );
        }
        if p.id == "local" {
            if !motor.ollama_instalado() {
                println!("  falta Ollama: actualiza con «vortex actualizar»");
            // Ajustes lo consulta cada vez que se abre: aquí no se arranca Ollama, Miku lo arranca al usarlo.
            } else if motor.ollama_corriendo() {
                println!(
                    "  modelo bajado: {}",
                    if motor.modelo_local_listo(&p.modelo) {
                        "sí"
                    } else {
                        "no (vortex-miku ia bajar)"
                    }
                );
            } else {
                println!("  Ollama: apagado (Miku lo arranca cuando lo necesita)");
            }
        }
        println!(
            "Cámbiala con: vortex-miku ia claude|local|gemini|openai [MODELO]  ·  vortex-miku ia otra URL MODELO"
        );
        return 0;
    }
    if args[0] == "clave" {
        if p.clave == Clave::No || p.id == "claude" {
            return error(&format!("{} no usa clave.", p.nombre), 1);
        }
        let clave = if args.len() > 1 {
            args[1].clone()
        } else {
            pedir_clave(&p)
        };
        if clave.is_empty() {
            return error("no escribiste ninguna clave.", 1);
        }
        let donde = match motor.guardar_clave(&p.id, &clave) {
            Ok(d) => d,
            Err(e) => return error(&format!("no pude guardar la clave: {}", super::strerror(&e)), 1),
        };
        println!(
            "Listo: la clave de {} quedó en {}.",
            p.nombre,
            donde_quedo(&donde)
        );
        proc::reabrir_si_esta_abierta(cx);
        return 0;
    }
    if args[0] == "bajar" {
        let local = Config {
            ia: "local".into(),
            ..cfg
        };
        return bajar_ia_local(&motor, &motor.elegido(&local).modelo);
    }
    let nuevo = args[0].to_lowercase();
    if !IAS.contains(&nuevo.as_str()) {
        return error(
            "uso: vortex-miku ia claude|local|gemini|openai [MODELO] · ia otra URL MODELO · ia clave · ia bajar",
            2,
        );
    }
    let mut cambios = vec![("ia", json!(nuevo)), ("ia_modelo", json!(""))];
    let mut nueva = Config {
        ia: nuevo.clone(),
        ia_modelo: String::new(),
        ..cfg.clone()
    };
    if nuevo == "otra" {
        if args.len() != 3 || !(args[1].starts_with("http://") || args[1].starts_with("https://")) {
            return error(
                "uso: vortex-miku ia otra URL MODELO   (p. ej. https://openrouter.ai/api/v1 un/modelo)",
                2,
            );
        }
        cambios.push(("ia_url", json!(args[1])));
        cambios[1] = ("ia_modelo", json!(args[2]));
        nueva.ia_url = args[1].clone();
        nueva.ia_modelo = args[2].clone();
    } else if args.len() > 1 && nuevo != "claude" {
        cambios[1] = ("ia_modelo", json!(args[1]));
        nueva.ia_modelo = args[1].clone();
    }
    if let Err(c) = cx.guardar_o_error(&cambios) {
        return c;
    }
    let p = motor.elegido(&nueva);
    println!(
        "ia={nuevo}: Miku piensa con {}{}",
        p.nombre,
        if nuevo != "claude" {
            format!(" («{}»).", p.modelo)
        } else {
            ".".into()
        }
    );
    let mut codigo = 0;
    if p.clave != Clave::No && motor.leer_clave(&nuevo).is_empty() {
        let clave = if std::io::stdin().is_terminal() {
            pedir_clave(&p)
        } else {
            String::new()
        };
        if clave.is_empty() {
            println!("Falta tu clave: ponla con «vortex-miku ia clave».");
        } else if let Err(e) = motor.guardar_clave(&nuevo, &clave) {
            println!("No pude guardar la clave: {}", super::strerror(&e));
        }
    }
    if nuevo == "local" {
        if !motor.ollama_instalado() {
            println!("Falta Ollama: actualiza Vortexine's con «vortex actualizar» y vuelve a correr esto.");
        } else if !motor.arrancar_ollama(std::time::Duration::from_secs(20)) {
            codigo = error(
                "no pude arrancar Ollama. Mira: systemctl --user status vortex-ollama",
                1,
            );
        } else if !motor.modelo_local_listo(&p.modelo) {
            codigo = bajar_ia_local(&motor, &p.modelo);
        }
    }
    if proc::reabrir_si_esta_abierta(cx) {
        println!("Miku estaba abierta: la volví a abrir con la IA nueva.");
    }
    codigo
}
