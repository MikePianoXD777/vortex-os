//! La voz desde el comando: elegirla, bajarla, probarla y los créditos. La voz en sí (Supertonic, la natural con
//! PyTorch y ElevenLabs) sigue siendo Python en procesos aparte; `natural` y `elevenlabs` (instalar el entorno
//! con pip, comprobar y guardar la clave) se le pasan tal cual al lanzador de Python.

use super::{Ctx, error, proc, strerror};
use crate::descargas::{self, Archivo, Descarga};
use crate::voz;
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const NO_NATURAL: &str = "la voz natural no está instalada. Instálala con: vortex-miku natural instalar";

pub fn voz(cx: &Ctx, args: &[String]) -> i32 {
    let cfg = cx.cfg();
    let r = &cx.rutas;
    if args.is_empty() {
        println!("Voces de Miku (la elegida lleva *):");
        for v in voz::catalogo(r) {
            let lista = voz::disponible(r, &cfg, &v);
            let motor = v["motor"].as_str().unwrap_or("");
            let estado = if motor == "qwen" {
                if lista {
                    "lista"
                } else {
                    "instálala con: vortex-miku natural instalar"
                }
            } else if motor == "elevenlabs" {
                if lista {
                    "lista"
                } else {
                    "ponla con: vortex-miku elevenlabs VOICE_ID"
                }
            } else if v["incluida"].as_bool() == Some(true) {
                if lista {
                    "lista"
                } else {
                    "se baja con el modelo la primera vez que abras a Miku"
                }
            } else if lista {
                "lista"
            } else {
                "se baja al elegirla (~290 KB)"
            };
            let id = v["id"].as_str().unwrap_or("");
            println!(
                "{} {:<14} {}  [{estado}]",
                if id == cfg.voz { '*' } else { ' ' },
                id,
                v["nombre"].as_str().unwrap_or("")
            );
            if let Some(d) = v["descripcion"].as_str().filter(|d| !d.is_empty()) {
                println!("    {d}");
            }
        }
        println!(
            "\nmiku-natural usa tu GPU o, lenta, tu CPU (Qwen3-TTS, Apache-2.0); las supertonic-* son locales en CPU\n(Supertonic 3, BigScience OpenRAIL-M); elevenlabs va por internet. Elige una con: vortex-miku voz ID"
        );
        return 0;
    }
    if args.len() != 1 {
        return error("uso: vortex-miku voz [ID]", 2);
    }
    let Some(v) = voz::buscar(r, &args[0]) else {
        return error(
            &format!(
                "no existe la voz «{}». Mira las que hay con: vortex-miku voz",
                args[0]
            ),
            1,
        );
    };
    let motor = v["motor"].as_str().unwrap_or("");
    if motor == "qwen" && !voz::disponible(r, &cfg, &v) {
        return error(NO_NATURAL, 1);
    }
    if motor == "elevenlabs" && !voz::disponible(r, &cfg, &v) {
        return error(
            "para usar ElevenLabs primero pon tu voz y tu clave con: vortex-miku elevenlabs VOICE_ID",
            1,
        );
    }
    if v["incluida"].as_bool() != Some(true)
        && let Err(e) = descargar_estilo(cx, &v)
    {
        return error(&e.to_string(), 1);
    }
    let nombre = v["nombre"].as_str().unwrap_or("").to_string();
    if let Err(c) = cx.guardar_o_error(&[("voz", json!(v["id"]))]) {
        return c;
    }
    println!("Voz elegida: {nombre}");
    if !descargas::paquete_listo(r, "voz") {
        println!(
            "Aún no está bajado el modelo de voz: se baja la primera vez que abras a Miku (o ya, con: vortex-miku preparar)."
        );
    }
    if proc::reabrir_si_esta_abierta(cx) {
        println!("Miku estaba abierta: la volví a abrir con la voz nueva.");
    }
    0
}

/// Baja el archivito de una voz opcional (con su sha256). Las de fábrica no necesitan nada más.
fn descargar_estilo(cx: &Ctx, v: &Value) -> Result<(), descargas::ErrorDescarga> {
    let a = &v["archivo"];
    if !a.is_object() || voz::archivo_estilo(&cx.rutas, v).is_file() {
        return Ok(());
    }
    let archivo = Archivo {
        ruta: a["ruta"].as_str().unwrap_or("").into(),
        url: a["url"].as_str().unwrap_or("").into(),
        bytes: a["bytes"].as_u64().unwrap_or(0),
        sha256: a["sha256"].as_str().unwrap_or("").into(),
    };
    descargas::bajar_archivo(&cx.rutas, &cx.rutas.voz, &archivo, None)
}

/// Baja la voz y el oído ya, mostrando el progreso en la terminal (la app hace lo mismo sola al abrirse).
pub fn preparar(cx: &Ctx) -> i32 {
    use std::io::Write;
    let r = &cx.rutas;
    let faltan = descargas::pendientes(r);
    if faltan.is_empty() {
        println!("La voz y el oído de Miku ya están listos.");
        return 0;
    }
    let tamano = if faltan.len() == descargas::paquetes().len() {
        descargas::TAMANO_TOTAL_TEXTO.to_string()
    } else {
        descargas::megas(descargas::bytes_pendientes(r, &faltan))
    };
    println!("Preparando la voz de Miku (una sola vez, {tamano})…");
    let cancelar = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut d = Descarga::nueva(
        r,
        Some(faltan),
        Arc::new(|texto, f| {
            print!("\r  {:>4}  {texto}   ", format!("{:.0}%", f * 100.0));
            let _ = std::io::stdout().flush();
        }),
        cancelar.clone(),
        Arc::new(|_| {}),
    );
    // Ctrl+C: se corta con calma y lo bajado se conserva.
    let interrupcion = crate::sistema::bandera_de_interrupcion();
    let vigilante = {
        let cancelar = cancelar.clone();
        std::thread::spawn(move || {
            while !cancelar.load(Ordering::Relaxed) {
                if interrupcion.load(Ordering::Relaxed) {
                    cancelar.store(true, Ordering::Relaxed);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        })
    };
    let resultado = d.correr();
    let cortada = interrupcion.load(Ordering::Relaxed);
    cancelar.store(true, Ordering::Relaxed);
    let _ = vigilante.join();
    match resultado {
        Ok(()) => {
            println!("\nListo: la voz y el oído de Miku están en {}", r.datos.display());
            0
        }
        Err(_) if cortada => {
            println!();
            error(
                "descarga cancelada; lo bajado se conserva y sigue donde se quedó la próxima vez.",
                130,
            )
        }
        Err(e) => {
            println!();
            error(&e.to_string(), 1)
        }
    }
}

pub fn licencias(cx: &Ctx) -> i32 {
    match std::fs::read_to_string(cx.rutas.raiz.join("licencias/CREDITOS.txt")) {
        Ok(t) => {
            println!("{t}");
            0
        }
        Err(_) => error("no encuentro el archivo de créditos. Reinstala vortex-miku.", 1),
    }
}

/// Prueba la voz elegida: suena o se guarda en un WAV.
pub fn decir(cx: &Ctx, args: &[String]) -> i32 {
    let mut args = args.to_vec();
    let mut salida = None;
    if let Some(i) = args.iter().position(|a| a == "-o") {
        if i + 1 >= args.len() {
            return error("uso: vortex-miku decir \"texto\" [-o archivo.wav]", 2);
        }
        salida = Some(args[i + 1].clone());
        args.drain(i..i + 2);
    }
    let texto = args.join(" ").trim().to_string();
    if texto.is_empty() {
        return error("uso: vortex-miku decir \"texto\" [-o archivo.wav]", 2);
    }
    let cfg = cx.cfg();
    let r = &cx.rutas;
    let elegida = voz::buscar(r, &cfg.voz);
    let motor = elegida
        .as_ref()
        .and_then(|v| v["motor"].as_str())
        .unwrap_or("")
        .to_string();
    if motor == "qwen" && !elegida.as_ref().is_some_and(|v| voz::disponible(r, &cfg, v)) {
        return error(NO_NATURAL, 1);
    }
    if motor != "elevenlabs" && motor != "qwen" && !descargas::paquete_listo(r, "voz") {
        return error(
            "el modelo de voz aún no está bajado. Bájalo con: vortex-miku preparar",
            1,
        );
    }
    let destino = salida.clone().unwrap_or_else(|| {
        r.temporales()
            .join(format!("decir-{}.wav", std::process::id()))
            .to_string_lossy()
            .into()
    });
    let voz_json = elegida.unwrap_or_else(|| json!({"id": cfg.voz, "motor": "supertonic"}));
    let (programa, mut orden, vars) = voz::comando_de_voz(r, &voz_json, cfg.avatar);
    orden.extend(["--salida".into(), destino.clone(), texto]);
    let mut c = Command::new(programa);
    c.args(&orden);
    for (k, v) in vars {
        c.env(k, v);
    }
    let codigo = match c.status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => return error(&format!("no pude lanzar la voz: {}", strerror(&e)), 1),
    };
    if codigo != 0 {
        return codigo;
    }
    if let Some(s) = salida {
        println!("Listo: {s}");
        return 0;
    }
    let reproducir = || -> i32 {
        for r in [vec!["pw-play"], vec!["paplay"], vec!["aplay", "-q"]] {
            if crate::sistema::cual(r[0]).is_some() {
                let _ = Command::new(r[0]).args(&r[1..]).arg(&destino).status();
                return 0;
            }
        }
        error("no encuentro con qué reproducir sonido (pw-play).", 1)
    };
    let codigo = reproducir();
    let _ = std::fs::remove_file(&destino);
    codigo
}

/// El lanzador de Python con todo lo de la voz natural y ElevenLabs. Va junto al código de Miku
/// (`vortex-miku-python`); `VORTEX_MIKU_CLI_PY` lo cambia (pruebas).
fn script_python(cx: &Ctx) -> Option<PathBuf> {
    std::env::var_os("VORTEX_MIKU_CLI_PY")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .chain([
            cx.rutas.raiz.join("vortex-miku-python"),
            "/usr/share/vortex-miku/vortex-miku-python".into(),
        ])
        .find(|p| p.is_file())
}

/// `natural …` y `elevenlabs …`: se quedan en Python (instalan el entorno con pip y hablan con el llavero).
pub fn en_python(cx: &Ctx, argv: &[String]) -> i32 {
    let Some(script) = script_python(cx) else {
        return error(
            "esta orden necesita la parte de Python de Miku (vortex-miku-python): reinstala el paquete vortex-miku.",
            1,
        );
    };
    let python = crate::sistema::cual("python3").unwrap_or_else(|| "python3".into());
    match Command::new(python).arg("-I").arg(script).args(argv).status() {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => error(&format!("no pude lanzar Python: {}", strerror(&e)), 1),
    }
}
