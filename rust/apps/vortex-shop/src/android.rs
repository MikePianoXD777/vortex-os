//! Juegos de Android (4.0): la Play Store dentro de Vortexine's OS, con Waydroid.
//!
//! La tienda no habla con Google: todo lo hace el comando `vortex-android` (preparar Android, abrir la
//! Play Store en un juego, abrir y quitar lo instalado). Aquí solo se llama y se traduce a algo que QML pinta.
//!
//! Para pruebas: `VORTEX_SHOP_ANDROID=/ruta/a/un/vortex-android` (falso) cambia el comando que se llama.

use crate::errores::{Error, Resultado};
use crate::proceso::{self, Entorno, Fallo};
use crate::url_de_archivo;
use regex::Regex;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub const URL_REGISTRO: &str = "https://www.google.com/android/uncertified";

/// Juegos que funcionan bien en Waydroid (x86 con traducción de ARM) y no traen anti-trampas que bloqueen
/// Android en PC. Solo nombre y paquete: la ficha completa (fotos, reseñas) la da la Play Store.
pub const POPULARES: [(&str, &str, &str); 12] = [
    ("com.kiloo.subwaysurf", "Subway Surfers", "Carreras sin fin"),
    ("com.supercell.clashroyale", "Clash Royale", "Estrategia"),
    ("com.supercell.brawlstars", "Brawl Stars", "Acción"),
    ("com.supercell.clashofclans", "Clash of Clans", "Estrategia"),
    ("com.innersloth.spacemafia", "Among Us", "Fiesta"),
    ("com.kitkagames.fallbuddies", "Stumble Guys", "Fiesta"),
    ("com.robtopx.geometrydashlite", "Geometry Dash Lite", "Ritmo"),
    ("com.king.candycrushsaga", "Candy Crush Saga", "Puzles"),
    ("com.fingersoft.hillclimb", "Hill Climb Racing", "Carreras"),
    ("com.ea.game.pvz2_row", "Plants vs. Zombies 2", "Estrategia"),
    ("com.rovio.baba", "Angry Birds 2", "Puzles"),
    ("com.mojang.minecraftpe", "Minecraft", "Construcción"),
];

/// Para QML (`androidPopulares`).
pub fn populares() -> Vec<Value> {
    POPULARES
        .iter()
        .map(|(p, n, g)| json!({"paquete": p, "nombre": n, "genero": g}))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EstadoAndroid {
    pub instalado: bool,
    pub preparado: bool,
    pub corriendo: bool,
    pub nvidia: bool,
    pub software: bool,
    pub registrado: bool,
}

impl EstadoAndroid {
    pub fn a_json(&self) -> Value {
        json!({"instalado": self.instalado, "preparado": self.preparado, "corriendo": self.corriendo,
               "nvidia": self.nvidia, "software": self.software, "registrado": self.registrado})
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppAndroid {
    pub paquete: String,
    pub nombre: String,
    pub icono: String,
}

impl AppAndroid {
    pub fn a_json(&self) -> Value {
        json!({"paquete": self.paquete, "nombre": self.nombre, "icono": self.icono})
    }
}

fn paquete_valido(p: &str) -> bool {
    !p.is_empty()
        && p.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.')
}

fn err(m: impl Into<String>) -> Error {
    Error::Android(m.into())
}

/// Los errores del comando, dichos para la persona.
fn explicar(detalle: &str) -> String {
    if detalle.contains("primero") && detalle.contains("preparar") {
        return "Primero hay que preparar Android (botón «Preparar Android»).".into();
    }
    if detalle.contains("no arrancó") {
        return "Android no arrancó. Ciérralo desde el panel y vuelve a intentarlo.".into();
    }
    if detalle.contains("abre la Play Store") {
        return "Abre la Play Store una vez y vuelve a pedir el número.".into();
    }
    let mut c = detalle.chars();
    match c.next() {
        Some(p) => format!("{}{}.", p.to_uppercase(), c.as_str()),
        None => "No se pudo. Vuelve a intentarlo.".into(),
    }
}

pub struct Android {
    entorno: Entorno,
}

impl Android {
    pub fn nuevo(entorno: Entorno) -> Self {
        Android { entorno }
    }

    fn comando(&self) -> String {
        self.entorno
            .valor("VORTEX_SHOP_ANDROID")
            .filter(|v| !v.is_empty())
            .map_or_else(|| "vortex-android".into(), |v| v.to_string_lossy().into_owned())
    }

    fn falta() -> Error {
        err("Falta el comando vortex-android: corre «vortex actualizar».")
    }

    fn correr(&self, args: &[&str], segundos: u64) -> Resultado<String> {
        let mut cmd = Command::new(self.comando());
        cmd.args(args);
        self.entorno.aplicar(&mut cmd);
        let r = match proceso::correr(cmd, None, Duration::from_secs(segundos)) {
            Err(Fallo::NoArranca(e)) if e.kind() == std::io::ErrorKind::NotFound => return Err(Self::falta()),
            Err(Fallo::NoArranca(_)) => return Err(Self::falta()),
            Err(Fallo::Tiempo) => {
                return Err(err("Android tardó demasiado en responder. Vuelve a intentarlo."));
            }
            Ok(r) => r,
        };
        if !r.bien() {
            let ultima = r.stderr.trim().lines().next_back().unwrap_or("");
            let detalle = ultima.strip_prefix("vortex-android: ").unwrap_or(ultima);
            return Err(err(explicar(detalle)));
        }
        Ok(r.stdout)
    }

    pub fn estado(&self) -> EstadoAndroid {
        let datos: Value = self
            .correr(&["estado"], 20)
            .ok()
            .and_then(|s| serde_json::from_str(if s.trim().is_empty() { "{}" } else { &s }).ok())
            .unwrap_or(Value::Null);
        let b = |k: &str| match datos.get(k) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(Value::Array(a)) => !a.is_empty(),
            Some(Value::Object(o)) => !o.is_empty(),
            _ => false,
        };
        EstadoAndroid {
            instalado: b("instalado"),
            preparado: b("preparado"),
            corriendo: b("corriendo"),
            nvidia: b("nvidia"),
            software: b("software"),
            registrado: b("registrado"),
        }
    }

    /// La primera vez (pide la contraseña una vez con pkexec). Avisa el paso por `progreso(pct, texto)`.
    pub fn preparar(&self, progreso: &dyn Fn(i32, &str)) -> Resultado<()> {
        let paso = Regex::new(r"PASO (\d+)/(\d+) (.*)").expect("regex");
        let mut cmd = Command::new(self.comando());
        cmd.arg("preparar")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        self.entorno.aplicar(&mut cmd);
        let mut hijo = cmd.spawn().map_err(|_| Self::falta())?;
        let mut stderr = hijo.stderr.take();
        let errores = std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(e) = stderr.as_mut() {
                let _ = e.read_to_string(&mut s);
            }
            s
        });
        if let Some(out) = hijo.stdout.take() {
            for linea in BufReader::new(out).lines().map_while(Result::ok) {
                if let Some(m) = paso.captures(linea.trim()) {
                    let n: i32 = m[1].parse().unwrap_or(1);
                    let total: i32 = m[2].parse().unwrap_or(1).max(1);
                    progreso((n - 1) * 100 / total, &m[3]);
                }
            }
        }
        let errores = errores.join().unwrap_or_default();
        let estado = hijo.wait().map_err(|_| Self::falta())?;
        if !estado.success() {
            let codigo = estado.code();
            if matches!(codigo, Some(126 | 127)) && errores.trim().is_empty() {
                return Err(err(
                    "No se dio permiso. Vuelve a intentarlo y escribe tu contraseña.",
                ));
            }
            let ultima = errores.trim().lines().next_back().unwrap_or("");
            let detalle = ultima.strip_prefix("vortex-android: ").unwrap_or(ultima);
            return Err(err(explicar(detalle)));
        }
        progreso(100, "Listo");
        Ok(())
    }

    fn con_paquete(paquete: &str) -> Resultado<&str> {
        if paquete_valido(paquete) {
            Ok(paquete)
        } else {
            Err(err("Ese juego no tiene un nombre válido."))
        }
    }

    pub fn tienda(&self, paquete: &str) -> Resultado<()> {
        let mut args = vec!["tienda"];
        if !paquete.is_empty() {
            args.push(Self::con_paquete(paquete)?);
        }
        self.correr(&args, 180).map(|_| ())
    }

    pub fn buscar(&self, texto: &str) -> Resultado<()> {
        self.correr(&["buscar", texto.trim()], 180).map(|_| ())
    }

    pub fn abrir(&self, paquete: &str) -> Resultado<()> {
        self.correr(&["abrir", Self::con_paquete(paquete)?], 180)
            .map(|_| ())
    }

    pub fn quitar(&self, paquete: &str) -> Resultado<()> {
        self.correr(&["quitar", Self::con_paquete(paquete)?], 180)
            .map(|_| ())
    }

    pub fn apps(&self) -> Vec<AppAndroid> {
        let lista: Vec<Value> = self
            .correr(&["apps"], 30)
            .ok()
            .and_then(|s| serde_json::from_str(if s.trim().is_empty() { "[]" } else { &s }).ok())
            .unwrap_or_default();
        let mut salida: Vec<AppAndroid> = lista
            .iter()
            .map(|a| {
                let texto = |k: &str| a.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                let mut icono = texto("icono");
                if !icono.is_empty() {
                    icono = if Path::new(&icono).is_file() {
                        url_de_archivo(Path::new(&icono))
                    } else {
                        String::new()
                    };
                }
                let paquete = texto("paquete");
                let nombre = if texto("nombre").is_empty() {
                    paquete.clone()
                } else {
                    texto("nombre")
                };
                AppAndroid {
                    paquete,
                    nombre,
                    icono,
                }
            })
            .collect();
        salida.sort_by_key(|a| a.nombre.to_lowercase());
        salida
    }

    pub fn id_google(&self) -> Resultado<String> {
        let numero = self.correr(&["id-google"], 180)?.trim().to_string();
        if numero.is_empty() || !numero.bytes().all(|b| b.is_ascii_digit()) {
            return Err(err("Abre la Play Store una vez y vuelve a pedir el número."));
        }
        Ok(numero)
    }

    pub fn registrado(&self) -> Resultado<()> {
        self.correr(&["registrado"], 10).map(|_| ())
    }

    pub fn parar(&self) -> Resultado<()> {
        self.correr(&["parar"], 60).map(|_| ())
    }
}
