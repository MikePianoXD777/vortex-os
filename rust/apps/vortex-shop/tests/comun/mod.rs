//! Lo que comparten las pruebas: el Flathub FALSO (la API de `/api/v2` servida en 127.0.0.1 con las
//! respuestas reales guardadas en `tests/datos`; nunca sale a internet), el «flatpak» y el «vortex-android»
//! falsos (programas de mentira en un PATH temporal) y las carpetas temporales.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use vortex_shop::proceso::Entorno;

pub mod falso;

pub const PRISM: &str = "org.prismlauncher.PrismLauncher";

pub fn aqui() -> PathBuf {
    // también la usan las pruebas de la ventana (crate `qt`), que comparten estos datos y falsos
    let propia = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    if propia.join("datos").is_dir() {
        propia
    } else {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tests")
    }
}

pub fn leer_datos(nombre: &str) -> String {
    std::fs::read_to_string(aqui().join("datos").join(nombre)).unwrap()
}

/// Una carpeta temporal que se borra al terminar.
pub fn tmp() -> tempfile::TempDir {
    tempfile::Builder::new()
        .prefix("vortex-tienda-prueba.")
        .tempdir()
        .unwrap()
}

// ====================================================================== el Flathub falso

#[derive(Default)]
struct EstadoServidor {
    datos: Mutex<HashMap<String, String>>,
    pedidos: Mutex<Vec<String>>,
    modo: Mutex<String>,
    parar: AtomicBool,
}

pub struct Servidor {
    estado: Arc<EstadoServidor>,
    pub puerto: u16,
    pub url: String,
}

impl Servidor {
    pub fn nuevo() -> Servidor {
        let estado = Arc::new(EstadoServidor::default());
        for e in std::fs::read_dir(aqui().join("datos")).unwrap() {
            let e = e.unwrap();
            estado.datos.lock().unwrap().insert(
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap(),
            );
        }
        let escucha = TcpListener::bind("127.0.0.1:0").unwrap();
        let puerto = escucha.local_addr().unwrap().port();
        let e2 = estado.clone();
        std::thread::spawn(move || {
            for conexion in escucha.incoming() {
                if e2.parar.load(Ordering::SeqCst) {
                    break; // al salir se cierra el puerto: conexión rechazada
                }
                let Ok(c) = conexion else { continue };
                let e3 = e2.clone();
                std::thread::spawn(move || atender(&e3, c));
            }
        });
        Servidor {
            estado,
            puerto,
            url: format!("http://127.0.0.1:{puerto}"),
        }
    }

    pub fn pedidos(&self) -> Vec<String> {
        self.estado.pedidos.lock().unwrap().clone()
    }

    /// «500» (todo responde 500) o «lento» (todo tarda 3 s).
    pub fn modo(&self, modo: &str) {
        *self.estado.modo.lock().unwrap() = modo.into();
    }

    pub fn datos_escribir(&self, nombre: &str, contenido: &Value) {
        self.estado
            .datos
            .lock()
            .unwrap()
            .insert(nombre.into(), contenido.to_string());
    }

    pub fn datos_escribir_crudo(&self, nombre: &str, texto: &str) {
        self.estado
            .datos
            .lock()
            .unwrap()
            .insert(nombre.into(), texto.into());
    }

    pub fn parar(&self) {
        self.estado.parar.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.puerto)); // despierta el accept
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

impl Drop for Servidor {
    fn drop(&mut self) {
        self.parar();
    }
}

/// Un puerto donde no escucha nadie (se abre y se cierra).
pub fn puerto_cerrado() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn responder(c: &mut TcpStream, codigo: u16, cuerpo: &str) {
    let texto = match codigo {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        422 => "Unprocessable Entity",
        _ => "Internal Server Error",
    };
    let r = format!(
        "HTTP/1.1 {codigo} {texto}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    );
    let _ = c.write_all(r.as_bytes());
}

fn consulta(q: &str) -> HashMap<String, String> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (k.to_string(), v.to_string())
        })
        .collect()
}

fn atender(e: &EstadoServidor, c: TcpStream) {
    let mut lector = BufReader::new(c.try_clone().unwrap());
    let mut c = c;
    let mut linea = String::new();
    if lector.read_line(&mut linea).unwrap_or(0) == 0 {
        return;
    }
    let mut partes = linea.split_whitespace();
    let (metodo, destino) = (
        partes.next().unwrap_or("").to_string(),
        partes.next().unwrap_or("").to_string(),
    );
    let (mut largo, mut agente) = (0usize, String::new());
    loop {
        let mut h = String::new();
        if lector.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            match k.to_ascii_lowercase().as_str() {
                "content-length" => largo = v.trim().parse().unwrap_or(0),
                "user-agent" => agente = v.trim().to_string(),
                _ => {}
            }
        }
    }
    let mut cuerpo = vec![0u8; largo];
    let _ = lector.read_exact(&mut cuerpo);
    let cuerpo: Option<Value> =
        (metodo == "POST").then(|| serde_json::from_slice(&cuerpo).unwrap_or(json!({})));
    let (ruta, q) = destino.split_once('?').unwrap_or((&destino, ""));
    let consulta = consulta(q);
    e.pedidos.lock().unwrap().push(format!(
        "{metodo} {destino}{}",
        cuerpo.as_ref().map(|b| format!(" {b}")).unwrap_or_default()
    ));
    let modo = e.modo.lock().unwrap().clone();
    if modo == "500" {
        return responder(&mut c, 500, r#"{"detail": "Internal Server Error"}"#);
    }
    if modo == "lento" {
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    if !agente.starts_with("vortexines-shop/") {
        return responder(
            &mut c,
            403,
            &json!({"detail": format!("User-Agent no esperado: {agente}")}).to_string(),
        );
    }
    let cargar = |n: &str| e.datos.lock().unwrap().get(n).cloned();
    let no_encontrada = || r#"{"detail": "App not found"}"#;
    if let Some(resto) = ruta.strip_prefix("/collection/") {
        if resto != "category" && (!consulta.contains_key("page") || !consulta.contains_key("per_page")) {
            return responder(&mut c, 400, r#"{"detail": "Bad Request"}"#);
        }
        if resto == "category" {
            return responder(&mut c, 200, &cargar("categorias.json").unwrap_or_default());
        }
        if resto == "trending" || resto == "popular" {
            return responder(&mut c, 200, &cargar("trending.json").unwrap_or_default());
        }
        if let Some(cat) = resto.strip_prefix("category/") {
            let validas: Value =
                serde_json::from_str(&cargar("categorias.json").unwrap_or_default()).unwrap_or(json!([]));
            if !validas.as_array().is_some_and(|a| a.iter().any(|v| v == cat)) {
                return responder(&mut c, 422, r#"{"detail": "categoría inválida"}"#);
            }
            let pagina: i64 = consulta["page"].parse().unwrap_or(1);
            if cat == "game" && pagina == 1 {
                return responder(&mut c, 200, &cargar("categoria_game.json").unwrap_or_default());
            }
            return responder(
                &mut c,
                200,
                &json!({"hits": [], "page": pagina, "totalHits": 0}).to_string(),
            );
        }
    } else if ruta == "/search" && metodo == "POST" {
        let q = cuerpo
            .as_ref()
            .and_then(|b| b.get("query"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase();
        if q.contains("minecraft") {
            return responder(
                &mut c,
                200,
                &cargar("busqueda_minecraft.json").unwrap_or_default(),
            );
        }
        return responder(
            &mut c,
            200,
            &json!({"hits": [], "query": q, "totalHits": 0}).to_string(),
        );
    } else if let Some(id) = ruta.strip_prefix("/appstream/") {
        return match cargar(&format!("appstream_{id}.json")) {
            Some(f) => responder(&mut c, 200, &f),
            None => responder(&mut c, 404, no_encontrada()),
        };
    } else if let Some(id) = ruta.strip_prefix("/summary/") {
        return match cargar(&format!("resumen_{id}.json")) {
            Some(f) => responder(&mut c, 200, &f),
            None => responder(&mut c, 404, no_encontrada()),
        };
    } else if ruta.starts_with("/app-picks/app-of-the-day/") {
        return responder(&mut c, 200, &cargar("app_del_dia.json").unwrap_or_default());
    } else if ruta.starts_with("/app-picks/apps-of-the-week/") {
        return responder(&mut c, 200, &cargar("apps_de_la_semana.json").unwrap_or_default());
    }
    responder(&mut c, 404, r#"{"detail": "Not Found"}"#);
}

// ====================================================================== los programas falsos

/// Copia un programa falso a `destino/nombre` (ejecutable).
pub fn instalar_falso(origen: &str, destino: &Path, nombre: &str) {
    std::fs::create_dir_all(destino).unwrap();
    // Enlace, no copia: un archivo recién escrito puede dar «Text file busy» al ejecutarlo si otra
    // prueba hace fork en ese momento (el hijo hereda el descriptor de escritura hasta su exec).
    // El falso del repo ya es ejecutable y nunca se abre para escribir.
    std::os::unix::fs::symlink(aqui().join("falsos").join(origen), destino.join(nombre)).unwrap();
}

/// Un PATH con un «flatpak» falso al principio y su carpeta de estado. HOME es la carpeta temporal.
pub struct BaseFlatpak {
    pub t: tempfile::TempDir,
    pub estado: PathBuf,
    pub bin: PathBuf,
}

impl BaseFlatpak {
    pub fn nueva() -> Self {
        let t = tmp();
        let estado = t.path().join("estado");
        let bin = t.path().join("bin");
        std::fs::create_dir_all(&estado).unwrap();
        instalar_falso("flatpak_falso", &bin, "flatpak");
        BaseFlatpak { t, estado, bin }
    }

    pub fn entorno(&self, falla: Option<&str>) -> Entorno {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut e = Entorno::default()
            .con("PATH", path)
            .con("FAKE_FLATPAK_DIR", self.estado.as_os_str())
            .con("HOME", self.t.path().as_os_str()); // nada escribe en tu casa de verdad
        e.quitar.push("FAKE_FLATPAK_FALLA".into());
        if let Some(f) = falla {
            e = e.con("FAKE_FLATPAK_FALLA", f);
        }
        e
    }

    pub fn flatpak(&self) -> vortex_shop::flatpak::Flatpak {
        vortex_shop::flatpak::Flatpak::nuevo(self.entorno(None))
    }

    pub fn escribir(&self, nombre: &str, lineas: &[&str]) {
        let t: String = lineas.iter().map(|l| format!("{l}\n")).collect();
        std::fs::write(self.estado.join(nombre), t).unwrap();
    }

    pub fn leer(&self, nombre: &str) -> Vec<String> {
        std::fs::read_to_string(self.estado.join(nombre))
            .map(|t| t.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default()
    }

    pub fn llamadas(&self) -> Vec<String> {
        std::fs::read_to_string(self.estado.join("llamadas.log"))
            .map(|t| t.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }
}

pub type Avances = Arc<Mutex<Vec<(i32, String)>>>;

/// Recoge lo que avisa `progreso`.
pub fn avance() -> (Avances, impl Fn(i32, &str) + Sync + Clone) {
    let eventos = Arc::new(Mutex::new(Vec::new()));
    let e2 = eventos.clone();
    (eventos, move |p, t: &str| {
        e2.lock().unwrap().push((p, t.to_string()))
    })
}

/// Un `vortex` falso (para las apps propias): deja la carpeta como el de verdad, o falla si existe `fallar`.
pub fn entorno_vortex_falso(t: &Path) -> Entorno {
    // El falso es un archivo del repo y se enlaza (ver instalar_falso: así no da «Text file busy»).
    instalar_falso("vortex_falso", t, "vortex");
    let falso = t.join("vortex");
    Entorno::default()
        .con("VORTEX_SHOP_VORTEX", falso.as_os_str())
        .con("XDG_DATA_HOME", t.join("datos").as_os_str())
}

/// Un `vortex-android` falso (con su carpeta de estado en `t/estado`).
pub fn entorno_android_falso(t: &Path, nvidia: bool) -> Entorno {
    instalar_falso("android_falso", t, "vortex-android");
    Entorno::default()
        .con("VORTEX_SHOP_ANDROID", t.join("vortex-android").as_os_str())
        .con("ANDROID_FALSO_DIR", t.join("estado").as_os_str())
        .con("ANDROID_FALSO_NVIDIA", if nvidia { "true" } else { "false" })
}
