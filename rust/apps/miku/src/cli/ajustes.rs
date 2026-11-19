//! Las órdenes que cambian o muestran un ajuste de `miku.json`: avatar, internet, modelo, sentimientos,
//! platicadora, cámara, compañera y habla-sola.

use super::proc::{self, Candado};
use super::{Ctx, error, si_no, strerror};
use crate::camara;
use crate::companera::{self, Companera};
use crate::corazon::Corazon;
use serde_json::json;
use std::path::PathBuf;

fn onoff(b: bool) -> &'static str {
    if b { "on" } else { "off" }
}

pub fn avatar(cx: &Ctx, args: &[String]) -> i32 {
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku avatar on|off", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("avatar", json!(nuevo))]) {
        return c;
    }
    proc::reabrir_si_esta_abierta(cx);
    println!("avatar={}", onoff(nuevo));
    0
}

pub fn internet(cx: &Ctx, args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "estado" {
        println!("internet={}", onoff(cx.cfg().internet));
        return 0;
    }
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku internet on|off|estado", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("internet", json!(nuevo))]) {
        return c;
    }
    proc::reabrir_si_esta_abierta(cx);
    println!("internet={}", onoff(nuevo));
    if nuevo {
        println!("Miku podrá buscar en la web y leer páginas; cada vez te pedirá permiso en su ventana.");
    }
    0
}

pub fn estado(cx: &Ctx) -> i32 {
    let c = cx.cfg();
    println!("avatar={}", onoff(c.avatar));
    println!(
        "abierta={}",
        if proc::pid_abierta().is_some() { "si" } else { "no" }
    );
    println!("platicadora={}", onoff(c.platicadora));
    0
}

pub fn modelo(cx: &Ctx, args: &[String]) -> i32 {
    use std::io::Read;
    if args.len() != 1 {
        return error("uso: vortex-miku modelo RUTA.vrm", 2);
    }
    let origen = crate::rutas::expandir_tilde(&args[0]);
    let mut cabecera = [0u8; 12];
    let leido = std::fs::File::open(&origen).and_then(|mut f| f.read(&mut cabecera));
    let n = match leido {
        Ok(n) => n,
        Err(e) => {
            return error(
                &format!("no puedo leer «{}»: {}", origen.display(), strerror(&e)),
                1,
            );
        }
    };
    // Un VRM es un glTF binario (.glb): empieza con «glTF».
    if cabecera[..n.min(4)] != *b"glTF" {
        let nombre = origen.file_name().unwrap_or_default().to_string_lossy();
        return error(
            &format!("«{nombre}» no parece un modelo VRM (un VRM empieza con «glTF»)."),
            1,
        );
    }
    let destino = cx.rutas.modelo_usuario.clone();
    let carpeta = destino.parent().map(PathBuf::from).unwrap_or_default();
    let credito_dest = carpeta.join("CREDITO.txt");
    let credito_origen = origen.parent().map(|p| p.join("CREDITO.txt")).unwrap_or_default();
    let mismo = destino.exists() && crate::sistema::mismo_archivo(&origen, &destino);
    let copiar = || -> std::io::Result<()> {
        std::fs::create_dir_all(&carpeta)?;
        if !mismo {
            let mut tmp = destino.as_os_str().to_owned();
            tmp.push(format!(".{}.tmp", std::process::id()));
            let tmp = PathBuf::from(tmp);
            std::fs::copy(&origen, &tmp)?;
            std::fs::rename(&tmp, &destino)?;
            if credito_origen.is_file() {
                std::fs::copy(&credito_origen, &credito_dest)?;
            } else {
                let _ = std::fs::remove_file(&credito_dest); // el crédito de otro modelo no vale para este
            }
        }
        Ok(())
    };
    if let Err(e) = copiar() {
        return error(&format!("no pude copiar el modelo: {}", strerror(&e)), 1);
    }
    let crudo = vortex_core::config::cargar_crudo(&cx.rutas.config);
    if crudo.get("modelo").is_some_and(verdadero) {
        // Si no, ese «modelo» de miku.json taparía el nuevo.
        if let Err(c) = cx.guardar_o_error(&[("modelo", json!(""))]) {
            return c;
        }
    }
    println!("Modelo listo: {}", destino.display());
    if !mismo && !credito_origen.is_file() {
        println!(
            "Aviso: no hay un CREDITO.txt junto al modelo. Respeta la licencia de su autor y ponle el crédito que pida."
        );
    }
    if proc::reabrir_si_esta_abierta(cx) {
        println!("Miku estaba abierta: la volví a abrir con el modelo nuevo.");
    }
    0
}

/// Lo que Python tomaría por «verdadero».
fn verdadero(v: &serde_json::Value) -> bool {
    use serde_json::Value;
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

pub fn sentimientos(cx: &Ctx, args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "estado" {
        println!("sentimientos={}", onoff(cx.cfg().sentimientos));
        println!("{}", Corazon::real().resumen());
        return 0;
    }
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku sentimientos [on|off]", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("sentimientos", json!(nuevo))]) {
        return c;
    }
    proc::reabrir_si_esta_abierta(cx);
    println!("sentimientos={}", onoff(nuevo));
    if !nuevo {
        println!(
            "Sus notas se quedan en {}: si los vuelves a encender, se acuerda de todo.",
            Corazon::real().carpeta.display()
        );
    }
    0
}

pub fn platicadora(cx: &Ctx, args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "estado" {
        println!("platicadora={}", onoff(cx.cfg().platicadora));
        return 0;
    }
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku platicadora on|off", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("platicadora", json!(nuevo))]) {
        return c;
    }
    println!("platicadora={}", onoff(nuevo));
    0
}

pub fn habla_sola(cx: &Ctx, args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "estado" {
        println!("habla-sola={}", onoff(cx.cfg().habla_sola));
        return 0;
    }
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku habla-sola on|off", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("habla_sola", json!(nuevo))]) {
        return c;
    }
    println!("habla-sola={}", onoff(nuevo));
    0
}

/// ¿Lo corrió la IA de Miku (correr_comando)? Entonces no puede cambiar la cámara ni el modo compañera.
fn desde_ia() -> bool {
    std::env::var("VORTEX_MIKU_DESDE_IA").as_deref() == Ok("1")
}

const NO_DESDE_IA: &str =
    "eso solo lo cambia la persona (en la ventana de Miku o en Ajustes), no un comando que corre la IA.";

/// 2.2: que Miku pueda tomar UNA foto con la webcam cuando se lo pides (apagada de fábrica).
pub fn camara(cx: &Ctx, args: &[String]) -> i32 {
    if args.is_empty() || args[0] == "estado" {
        println!("camara={}", onoff(cx.cfg().camara));
        return 0;
    }
    if desde_ia() {
        return error(NO_DESDE_IA, 1);
    }
    let Some(nuevo) = (args.len() == 1).then(|| si_no(&args[0])).flatten() else {
        return error("uso: vortex-miku camara on|off|estado", 2);
    };
    if let Err(c) = cx.guardar_o_error(&[("camara", json!(nuevo))]) {
        return c;
    }
    println!("camara={}", onoff(nuevo));
    if nuevo {
        println!("{}", camara::AVISO_PRIMERA_VEZ);
    }
    0
}

const USO_COMPANERA: &str =
    "uso: vortex-miku companera on|off|estado|tono|sin-cuenta|bitacora|borrar-bitacora";

/// 2.2: el modo compañera. Se activa solo después de aceptar la bienvenida.
pub fn companera(cx: &Ctx, args: &[String]) -> i32 {
    let sub = args
        .first()
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "estado".into());
    let cfg = cx.cfg();
    let comp = Companera::real();
    if sub == "estado" {
        let activo = cfg.companera && cfg.companera_consentimiento;
        println!("companera={}", onoff(activo));
        println!("tono={}", cfg.companera_tono);
        println!("sin_cuenta={}", onoff(companera::sin_cuenta(&cfg)));
        if activo {
            println!(
                "asomadas_hoy={}/{}",
                comp.veces_hoy(&comp.ahora(), &comp.leer_estado()),
                companera::TOPE_DIA
            );
            let corazon = Corazon::real();
            let motivo = comp.motivo_para_no(&cfg, Some(&|| corazon.ocupado()));
            println!(
                "ahora={}",
                if motivo.is_empty() {
                    "puede-asomarse"
                } else {
                    &motivo
                }
            );
        }
        return 0;
    }
    if sub != "bitacora" && sub != "bitácora" && desde_ia() {
        return error(NO_DESDE_IA, 1);
    }
    match sub.as_str() {
        "on" | "si" | "sí" | "1" | "true" | "acepto" | "aceptar" => {
            if !cfg.companera_consentimiento {
                // El permiso SOLO se da en la ventana de Miku (con su botón): ningún comando lo da, para que
                // nada lo active sin que la persona vea la bienvenida.
                println!("{}", companera::CONSENTIMIENTO);
                println!("\nSe acepta en la ventana de Miku, con el botón «Sí, activar el modo compañera».");
                {
                    let _c = Candado::tomar();
                    if proc::pid_abierta().is_some() {
                        println!(
                            "Miku ya está abierta: ciérrala y vuelve a correr «vortex-miku companera on»."
                        );
                    } else {
                        proc::lanzar(cx, &[("VORTEX_MIKU_COMPANERA", "consentimiento")]);
                    }
                }
                println!("companera=off");
                return 0;
            }
            if let Err(c) = cx.guardar_o_error(&[
                ("companera", json!(true)),
                ("camara", json!(true)),
                ("camara_aviso", json!(true)),
            ]) {
                return c;
            }
            println!("companera=on");
            0
        }
        "off" | "no" | "0" | "false" => {
            // Apagarlo también retira el permiso: para volver a prenderlo se acepta otra vez la bienvenida.
            if let Err(e) = cx.guardar_mapa(&companera::apagado()) {
                return cx.error_guardar(&e);
            }
            comp.anotar("Modo compañera apagado");
            println!("companera=off");
            0
        }
        "sin-cuenta" | "sin_cuenta" => {
            let valor = args.get(1).map(|s| s.to_lowercase()).unwrap_or_default();
            if matches!(valor.as_str(), "off" | "no" | "0" | "false") {
                if let Err(c) = cx.guardar_o_error(&[
                    ("companera_sin_cuenta", json!(false)),
                    ("companera_mayor", json!(false)),
                ]) {
                    return c;
                }
                comp.anotar("Casilla «asomarse sin avisar» desmarcada");
                println!("sin_cuenta=off");
                return 0;
            }
            if !matches!(valor.as_str(), "on" | "si" | "sí" | "1" | "true") {
                return error("uso: vortex-miku companera sin-cuenta on|off", 2);
            }
            if !(cfg.companera && cfg.companera_consentimiento) {
                return error("primero activa el modo compañera (vortex-miku companera on).", 1);
            }
            // «Soy mayor de 18 años» SOLO se confirma con el botón de la ventana de Miku: ningún comando la marca.
            println!("{}", companera::TEXTO_SIN_CUENTA);
            println!("\nSe marca en la ventana de Miku, con el botón «Soy mayor de 18 años».");
            {
                let _c = Candado::tomar();
                if proc::pid_abierta().is_some() {
                    println!(
                        "Miku ya está abierta: ciérrala y vuelve a correr «vortex-miku companera sin-cuenta on»."
                    );
                } else {
                    proc::lanzar(cx, &[("VORTEX_MIKU_COMPANERA", "mayoria")]);
                }
            }
            println!("sin_cuenta=off");
            0
        }
        "tono" => {
            let elegido = args
                .get(1)
                .map(|s| s.to_lowercase().replace('ñ', "n"))
                .unwrap_or_default();
            if elegido != "carinosa" && elegido != "suave" {
                return error("uso: vortex-miku companera tono carinosa|suave", 2);
            }
            if let Err(c) = cx.guardar_o_error(&[("companera_tono", json!(elegido))]) {
                return c;
            }
            println!("tono={elegido}");
            0
        }
        "bitacora" | "bitácora" => {
            let lineas = comp.leer_bitacora();
            if lineas.is_empty() {
                println!("La bitácora está vacía.");
            } else {
                println!("{}", lineas.join("\n"));
            }
            0
        }
        "borrar-bitacora" | "borrar-bitácora" => {
            comp.borrar_bitacora();
            println!("Bitácora borrada.");
            0
        }
        _ => error(USO_COMPANERA, 2),
    }
}
