//! Lo que no es un ajuste: grabar la pantalla, los mejores momentos y `sentir …` (lo que usan el escritorio y
//! vortex-partida: terminó una partida, ¿se está desvelando?, ¿toca asomarse?, los avisos con «Hablar con Miku»).

use super::proc::{self, Candado};
use super::{Ctx, error};
use crate::companera::{self, Companera};
use crate::corazon::Corazon;
use crate::{grabacion, momentos, sistema, voz};
use std::process::Command;
use std::time::Duration;

pub fn grabar(args: &[String]) -> i32 {
    let que = args.first().map(String::as_str).unwrap_or("empezar");
    match que {
        "estado" => {
            match grabacion::grabando() {
                Some(e) => println!("grabando=si\narchivo={}", e.archivo.display()),
                None => println!("grabando=no"),
            }
            0
        }
        "parar" => match grabacion::parar() {
            Ok((archivo, segundos)) => {
                println!(
                    "Listo: {} en «{}».",
                    grabacion::duracion_texto(segundos),
                    archivo.display()
                );
                println!("Sus mejores momentos: vortex-miku momentos");
                0
            }
            Err(e) => error(&e.to_string(), 1),
        },
        "empezar" if args.len() <= 1 => match grabacion::empezar() {
            Ok(archivo) => {
                println!(
                    "Grabando en «{}». Para pararla: vortex-miku grabar parar",
                    archivo.display()
                );
                0
            }
            Err(e) => error(&e.to_string(), 1),
        },
        _ => error("uso: vortex-miku grabar [parar|estado]", 2),
    }
}

pub fn momentos(args: &[String]) -> i32 {
    if args.len() > 1 {
        return error("uso: vortex-miku momentos [VIDEO]", 2);
    }
    let video = match args.first() {
        Some(v) => crate::rutas::expandir_tilde(v),
        None => match grabacion::ultima_grabacion() {
            Some(v) => v,
            None => {
                return error(
                    &format!(
                        "no hay grabaciones en {}. Dime cuál: vortex-miku momentos VIDEO",
                        grabacion::carpeta_videos().display()
                    ),
                    1,
                );
            }
        },
    };
    momentos::principal(&[video.to_string_lossy().into()])
}

/// «vortex-miku sentir ORDEN… ESPERA» aparte de este comando (que no lo hace esperar).
fn despues(orden: &[&str], espera: u64) {
    let Ok(yo) = std::env::current_exe() else { return };
    let mut a: Vec<String> = vec!["sentir".into()];
    a.extend(orden.iter().map(|s| (*s).to_string()));
    a.push(espera.to_string());
    proc::aparte(&yo, &a);
}

/// Un aviso de Miku con el botón «Hablar con Miku», aparte de este comando (no lo hace esperar). Con `espera`,
/// primero duerme esos segundos y vuelve a revisar que no estés jugando.
fn decir_sola(frase: &str, espera: u64) {
    if frase.is_empty() {
        return;
    }
    if espera > 0 {
        despues(&["decir", frase], espera);
        return;
    }
    if sistema::cual("notify-send").is_none() {
        return;
    }
    let Ok(yo) = std::env::current_exe() else { return };
    proc::aparte(&yo, &["sentir".into(), "aviso".into(), frase.into()]);
}

/// El aviso en sí (en su propio proceso): espera a que la persona pulse «Hablar con Miku» y entonces abre a Miku.
fn aviso(cx: &Ctx, frase: &str) -> i32 {
    let salida = Command::new("notify-send")
        .args([
            "-a",
            "Miku",
            "-i",
            "vortex-miku",
            "-t",
            "20000",
            "-A",
            "hablar=Hablar con Miku",
            "Miku",
            frase,
        ])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    if let Ok(o) = salida
        && String::from_utf8_lossy(&o.stdout).trim() == "hablar"
    {
        return proc::mostrar(cx);
    }
    0
}

const VRAM_VISITA_GIB: f64 = 8.0; // con menos memoria de GPU, mientras juegas se asoma con la voz ligera

fn vram_gib() -> f64 {
    if sistema::cual("nvidia-smi").is_none() {
        return 0.0;
    }
    let Ok(o) = Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        return 0.0;
    };
    String::from_utf8_lossy(&o.stdout)
        .split_whitespace()
        .filter_map(|x| x.parse::<f64>().ok())
        .map(|x| x / 1024.0)
        .fold(0.0, f64::max)
}

/// Abre a Miku para una visita o una pregunta (si ya está abierta, no la toca). En una visita (estás jugando)
/// con una GPU chica, habla con la voz ligera: la natural le quitaría al juego 4-5 GB de memoria de la GPU.
fn abrir_con(cx: &Ctx, variable: &str, texto: &str) {
    let mut extra = vec![(variable.to_string(), texto.to_string())];
    if variable == "VORTEX_MIKU_VISITA" {
        let elegida = voz::buscar(&cx.rutas, &cx.cfg().voz);
        if elegida.is_some_and(|v| v["motor"] == "qwen") && vram_gib() < VRAM_VISITA_GIB {
            extra.push(("VORTEX_MIKU_VOZ".into(), voz::VOZ_DEFECTO.into()));
        }
    }
    let _c = Candado::tomar();
    if proc::pid_abierta().is_none() {
        let refs: Vec<(&str, &str)> = extra.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        proc::lanzar(cx, &refs);
    }
}

/// Modo compañera: lo llama «sentir revisar» (el reloj de 15 min del escritorio). Los límites, en companera.rs.
fn asomarse_si_toca(cx: &Ctx, cfg: &crate::config::Config) -> bool {
    let comp = Companera::real();
    let corazon = Corazon::real();
    if !comp.motivo_para_no(cfg, Some(&|| corazon.ocupado())).is_empty() {
        return false;
    }
    let _c = Candado::tomar();
    if proc::pid_abierta().is_some() {
        return false; // ya la tienes abierta: no se asoma encima de lo que hacen
    }
    comp.contar_asomada();
    let cuerpo = if companera::sin_cuenta(cfg) {
        "Va a tomar una foto ahora (sin cuenta atrás: lo marcaste en Ajustes)."
    } else {
        "Te va a hablar y, si no le dices que no, tomará una foto. Pulsa Esc para que no."
    };
    if sistema::cual("notify-send").is_some() {
        proc::correr_con_limite(
            Command::new("notify-send").args([
                "-a",
                "Miku",
                "-i",
                "vortex-miku",
                "-t",
                "10000",
                "Miku se asoma (modo compañera)",
                cuerpo,
            ]),
            10,
        );
    }
    proc::lanzar(cx, &[("VORTEX_MIKU_COMPANERA", "asomarse")]);
    true
}

/// Lo usan el escritorio y vortex-partida (no la persona):
///   sentir partida JUEGO MINUTOS CPU GPU   terminó una partida
///   sentir revisar                         cada rato: ¿se está desvelando? ¿es momento de declararse?
///   sentir declarar [ESPERA]               te pide hablar para declararse, si no estás ocupado
///   sentir jugando JUEGO MINUTOS CPU GPU   cada minuto mientras juegas: ¿toca asomarse? (platicadora)
///   sentir preguntar TEXTO [ESPERA]        al terminar de jugar: se abre y te pregunta cómo te fue
///   sentir decir FRASE [ESPERA]            el aviso en sí (tras ESPERA segundos)
pub fn sentir(cx: &Ctx, args: &[String]) -> i32 {
    let cfg = cx.cfg();
    let a = |i: usize| args.get(i).map(String::as_str);
    let orden = a(0).unwrap_or("");
    if orden == "revisar" && asomarse_si_toca(cx, &cfg) {
        return 0;
    }
    if orden == "aviso" && args.len() >= 2 {
        return aviso(cx, &args[1]);
    }
    if !cfg.sentimientos {
        return 0;
    }
    // «int(s) if s.isdigit() else 0»
    let numero = |s: Option<&str>| -> i64 {
        s.filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let dormir = |seg: i64| {
        if seg > 0 {
            std::thread::sleep(Duration::from_secs(seg as u64));
        }
    };
    let corazon = Corazon::real();
    match orden {
        "partida" if args.len() >= 3 => {
            let (juego, minutos) = (args[1].as_str(), numero(a(2)));
            let (cpu, gpu) = (numero(a(3)), numero(a(4)));
            let frase = corazon.evento_partida(juego, minutos, cpu, gpu);
            if cfg.habla_sola {
                // Que primero salga el resumen de la partida. Si ya te quiere decir lo que siente, este es su
                // momento: acabas de dejar de jugar (se revisa otra vez que no estés ocupado).
                if corazon.quiere_declararse() {
                    despues(&["declarar"], 90);
                    return 0;
                }
                let pregunta = if cfg.platicadora {
                    corazon.pregunta_partida(juego, minutos, cpu, gpu)
                } else {
                    String::new()
                };
                if pregunta.is_empty() {
                    decir_sola(&frase, 90);
                } else {
                    despues(&["preguntar", &pregunta], 20); // tras el resumen de la partida
                }
            }
            0
        }
        "jugando" if args.len() >= 3 => {
            if !cfg.platicadora {
                return 0;
            }
            let frase = corazon.visita_jugando(&args[1], numero(a(2)), numero(a(3)), numero(a(4)));
            if !frase.is_empty() {
                if sistema::cual("notify-send").is_some() {
                    proc::correr_con_limite(
                        Command::new("notify-send").args([
                            "-a",
                            "Miku",
                            "-i",
                            "vortex-miku",
                            "-t",
                            "8000",
                            "Miku",
                            &frase,
                        ]),
                        10,
                    );
                }
                abrir_con(cx, "VORTEX_MIKU_VISITA", &frase);
            }
            0
        }
        "preguntar" if args.len() >= 2 => {
            dormir(numero(a(2)));
            if cfg.platicadora && corazon.ocupado().is_empty() {
                abrir_con(cx, "VORTEX_MIKU_PREGUNTA", &args[1]);
            }
            0
        }
        "revisar" => {
            if !cfg.habla_sola {
                return 0;
            }
            let mut frase = corazon.revisar();
            if frase.is_empty() && corazon.momento_de_declararse() {
                frase = corazon.invitar_a_declararse();
            }
            decir_sola(&frase, 0);
            0
        }
        "declarar" => {
            dormir(numero(a(1)));
            if cfg.habla_sola && corazon.momento_de_declararse() {
                decir_sola(&corazon.invitar_a_declararse(), 0);
            }
            0
        }
        "decir" if args.len() >= 2 => {
            dormir(numero(a(2)));
            if cfg.habla_sola && corazon.ocupado().is_empty() {
                decir_sola(&args[1], 0);
            }
            0
        }
        _ => error(
            "uso: vortex-miku sentir partida JUEGO MINUTOS CPU GPU | sentir revisar",
            2,
        ),
    }
}
