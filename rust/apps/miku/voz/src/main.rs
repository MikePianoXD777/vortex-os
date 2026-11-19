//! `miku-voz`: el proceso de la voz de Miku. Carga el modelo una vez y convierte frases en audio, con el
//! protocolo de `miku/voz_worker.py`:
//!
//!   entra   {"id": 1, "texto": "Hola."}                 (o {"cmd": "salir"})
//!   sale    {"listo": true, "voz": "supertonic-f1"}     al cargar el modelo
//!           {"id": 1, "wav": "/run/user/1000/vortex-miku/voz-1.wav", "seg": 1.8, "visemas": [[0,0,0,0,0], ...]}
//!           {"id": 1, "error": "texto del problema"}    (o sin id: no pudo cargar)
//!
//! También saca una frase a un archivo (para probar voces): `miku-voz --voz supertonic-f1 --salida a.wav "Hola."`

use serde_json::{Map, Value, json};
use std::io::{BufRead, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::process::ExitCode;
use std::time::Instant;

use vortex_miku::rutas::{self, Rutas};
use vortex_miku::{sistema, voz};
use vortex_miku_voz::{audio, estilo, motor::Supertonic, texto};

struct Opciones {
    voz: String,
    con_avatar: bool,
    salida: Option<String>,
    ajustes: Map<String, Value>,
    texto: Vec<String>,
}

fn opciones() -> Result<Opciones, String> {
    let mut o = Opciones {
        voz: voz::VOZ_DEFECTO.into(),
        con_avatar: true,
        salida: None,
        ajustes: Map::new(),
        texto: vec![],
    };
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        let mut valor = |n: &str| args.next().ok_or(format!("a {n} le falta el valor"));
        match a.as_str() {
            "--voz" => o.voz = valor("--voz")?,
            "--estilo" => o.con_avatar = valor("--estilo")? != "normal",
            "--salida" => o.salida = Some(valor("--salida")?),
            "--tono" | "--velocidad" | "--corte" | "--estante" | "--nivel" => {
                let v: f64 = valor(&a)?.parse().map_err(|_| format!("{a} pide un número"))?;
                o.ajustes.insert(a[2..].to_string(), json!(v));
            }
            "-h" | "--help" => {
                return Err("uso: miku-voz [--voz ID] [--estilo miku|normal] [--salida A.wav TEXTO]".into());
            }
            _ => o.texto.push(a),
        }
    }
    Ok(o)
}

struct Voz {
    motor: Supertonic,
    estilo: Map<String, Value>,
}

impl Voz {
    fn cargar(o: &Opciones) -> Result<Self, String> {
        let r = Rutas::real();
        let v = voz::buscar(&r, &o.voz).ok_or(format!(
            "No existe la voz «{}». Mira las que hay con: vortex-miku voz",
            o.voz
        ))?;
        match v["motor"].as_str() {
            Some("elevenlabs") | Some("qwen") => {
                return Err(format!(
                    "La voz «{}» no es de Supertonic (esa la dice voz_worker.py).",
                    o.voz
                ));
            }
            _ => {}
        }
        if !voz::archivo_estilo(&r, &v).is_file() {
            return Err(format!(
                "La voz «{}» no está instalada. Instálala con: vortex-miku voz {}",
                o.voz, o.voz
            ));
        }
        let mut e = estilo::de_voz(&v, o.con_avatar);
        e.extend(o.ajustes.clone());
        let motor = Supertonic::cargar(&r.voz, v["estilo_voz"].as_str().unwrap_or("F1"), rutas::hilos())
            .map_err(|e| e.to_string())?;
        Ok(Self { motor, estilo: e })
    }

    fn sintetizar(&mut self, t: &str) -> Result<Vec<f32>, String> {
        let hablado = texto::texto_hablado(&voz::quitar_etiquetas(t));
        if hablado.is_empty() {
            return Err("no hay nada que decir".into());
        }
        let x = self
            .motor
            .sintetizar(&hablado, estilo::velocidad(&self.estilo))
            .map_err(|e| e.to_string())?;
        Ok(estilo::procesar(&x, self.motor.sr, &self.estilo))
    }
}

fn enviar(v: &Value) {
    let mut s = std::io::stdout().lock();
    let _ = writeln!(s, "{v}");
    let _ = s.flush();
}

fn redondear(v: f64, decimales: i32) -> f64 {
    let f = 10f64.powi(decimales);
    (v * f).round() / f
}

fn servir(mut v: Voz, id_voz: &str) {
    sistema::morir_con_el_padre();
    if std::os::unix::process::parent_id() == 1 {
        return;
    }
    let carpeta = Rutas::real().temporales();
    enviar(&json!({"listo": true, "voz": id_voz}));
    let mut contador = 0u64;
    for linea in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(pedido) = serde_json::from_str::<Value>(&linea) else {
            continue;
        };
        if pedido["cmd"] == "salir" {
            break;
        }
        let id = pedido["id"].clone();
        let t0 = Instant::now();
        let t = match &pedido["texto"] {
            Value::String(s) => s.clone(),
            Value::Null => String::new(),
            otro => otro.to_string(),
        };
        let r = v.sintetizar(&t).and_then(|x| {
            let sr = v.motor.sr;
            let ruta = carpeta.join(format!("voz-{}-{contador}.wav", std::process::id()));
            contador += 1;
            std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&ruta)
                .and_then(|mut f| f.write_all(&audio::a_wav(&x, sr)))
                .map_err(|e| format!("no pude guardar el audio: {e}"))?;
            Ok(json!({
                "id": id,
                "wav": ruta.to_string_lossy(),
                "seg": redondear(x.len() as f64 / sr as f64, 3),
                "visemas": audio::visemas(&x, sr, 30),
                "tardo": redondear(t0.elapsed().as_secs_f64(), 2),
            }))
        });
        enviar(&r.unwrap_or_else(|e| json!({"id": id, "error": e})));
    }
}

fn main() -> ExitCode {
    let o = match opciones() {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let v = match Voz::cargar(&o) {
        Ok(v) => v,
        Err(e) => {
            if o.salida.is_some() {
                eprintln!("vortex-miku: {e}");
            } else {
                enviar(&json!({"error": e}));
            }
            return ExitCode::FAILURE;
        }
    };
    if let Some(salida) = &o.salida {
        let mut v = v;
        return match v.sintetizar(&o.texto.join(" ")) {
            Ok(x) => match std::fs::write(salida, audio::a_wav(&x, v.motor.sr)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("vortex-miku: {salida}: {e}");
                    ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("vortex-miku: {e}");
                ExitCode::FAILURE
            }
        };
    }
    servir(v, &o.voz);
    ExitCode::SUCCESS
}
