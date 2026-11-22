//! El comando `vortex-miku` en Rust: los mismos subcomandos, textos, códigos de salida y archivos que el
//! lanzador de Python (`apps/miku/vortex-miku-python`). Lo que hay que ver con la voz natural y con ElevenLabs
//! (instalar PyTorch, guardar la clave en el llavero) se queda en Python: aquí solo se le pasa.
//!
//!   proc      abrir, cerrar y reconocer la app (PID, candado, cerrar todo lo que lanzó)
//!   ajustes   avatar, internet, estado, modelo, cámara, compañera, platicadora, habla-sola, sentimientos
//!   voz       voz, preparar, decir, licencias, y el paso a Python de natural y elevenlabs
//!   ia        con qué IA piensa (y su clave, y bajar la local)
//!   sentir    lo que usan el escritorio y vortex-partida (sentir …), grabar y momentos

mod ajustes;
mod ia;
mod proc;
mod sentir;
mod voz;

use crate::config::Config;
use crate::rutas::Rutas;
use serde_json::{Map, Value};

const AYUDA: &str = include_str!("ayuda.txt");

/// Lo que comparten todas las órdenes: dónde está cada cosa.
pub struct Ctx {
    pub rutas: Rutas,
}

impl Ctx {
    pub fn cfg(&self) -> Config {
        Config::cargar(&self.rutas.config, &self.rutas)
    }

    /// Pone esas claves en miku.json sin tocar las demás.
    pub fn guardar(&self, cambios: &[(&str, Value)]) -> std::io::Result<()> {
        let mut m = Map::new();
        for (k, v) in cambios {
            m.insert((*k).into(), v.clone());
        }
        Config::guardar(&self.rutas.config, &m)
    }

    pub fn guardar_mapa(&self, cambios: &Map<String, Value>) -> std::io::Result<()> {
        Config::guardar(&self.rutas.config, cambios)
    }

    /// Guarda y, si no se pudo, escribe el error de siempre y da el código de salida.
    pub fn guardar_o_error(&self, cambios: &[(&str, Value)]) -> Result<(), i32> {
        self.guardar(cambios).map_err(|e| self.error_guardar(&e))
    }

    pub fn error_guardar(&self, e: &std::io::Error) -> i32 {
        error(
            &format!("no pude guardar {}: {}", self.rutas.config.display(), strerror(e)),
            1,
        )
    }
}

/// «vortex-miku: texto» por la salida de errores; devuelve el código de salida.
pub fn error(texto: &str, codigo: i32) -> i32 {
    eprintln!("vortex-miku: {texto}");
    codigo
}

/// El mensaje del sistema sin el « (os error N)» que le pone Rust: «No such file or directory».
pub fn strerror(e: &std::io::Error) -> String {
    let t = e.to_string();
    match t.rfind(" (os error ") {
        Some(i) => t[..i].to_string(),
        None => t,
    }
}

/// on/off y sus sinónimos (en minúsculas).
pub fn si_no(texto: &str) -> Option<bool> {
    match texto.to_lowercase().as_str() {
        "on" | "si" | "sí" | "true" | "1" => Some(true),
        "off" | "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

pub fn principal(argv: &[String]) -> i32 {
    let cx = Ctx { rutas: Rutas::real() };
    let a = |i: usize| argv.get(i).map(String::as_str);
    let Some(orden) = a(0) else {
        return proc::alternar(&cx);
    };
    let args = &argv[1..];
    match orden {
        "alternar" | "abrir" | "toggle" => proc::alternar(&cx),
        "-h" | "--help" | "ayuda" => {
            println!("{}", AYUDA.trim());
            0
        }
        "avatar" => ajustes::avatar(&cx, args),
        "estado" => ajustes::estado(&cx),
        "internet" => ajustes::internet(&cx, args),
        "modelo" => ajustes::modelo(&cx, args),
        "voz" => voz::voz(&cx, args),
        "preparar" => voz::preparar(&cx),
        "licencias" | "creditos" | "créditos" => voz::licencias(&cx),
        "decir" => voz::decir(&cx, args),
        "elevenlabs" | "natural" => voz::en_python(&cx, argv),
        "grabar" => sentir::grabar(args),
        "momentos" => sentir::momentos(args),
        "ia" => ia::ia(&cx, args),
        "sentimientos" | "corazon" | "corazón" => ajustes::sentimientos(&cx, args),
        "platicadora" => ajustes::platicadora(&cx, args),
        "camara" | "cámara" => ajustes::camara(&cx, args),
        "companera" | "compañera" => ajustes::companera(&cx, args),
        "habla-sola" | "habla_sola" => ajustes::habla_sola(&cx, args),
        "sentir" => sentir::sentir(&cx, args),
        "mostrar" => proc::mostrar(&cx),
        "chat" | "mini" => proc::chat(&cx),
        otra => error(&format!("no conozco «{otra}». Mira: vortex-miku ayuda"), 2),
    }
}
