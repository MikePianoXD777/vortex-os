//! Un Ollama de mentira: un servidor HTTP en 127.0.0.1 (puerto libre) con las mismas respuestas que el de
//! `pruebas/test_motor.py`. Lo usan las pruebas del motor y de la ventana (`include!`/`mod`): no hace falta
//! Ollama ni modelos, y no se toca `~/.ollama`.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Default)]
pub struct EstadoFalso {
    pub textos_embebidos: AtomicUsize,
    pub borrados: Mutex<Vec<String>>,
    pub peticiones: Mutex<Vec<String>>,
    /// Los cuerpos de cada `/api/chat`.
    pub chats: Mutex<Vec<Value>>,
}

pub struct ServidorFalso {
    pub url: String,
    pub estado: Arc<EstadoFalso>,
    vivo: Arc<AtomicBool>,
}

impl Drop for ServidorFalso {
    fn drop(&mut self) {
        self.vivo.store(false, Ordering::SeqCst);
        // Despierta al accept().
        let _ = TcpStream::connect(self.url.trim_start_matches("http://"));
    }
}

pub fn crc32(datos: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for b in datos {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Un «embedding» de juguete: cada palabra suma 1 en una casilla. Textos con las mismas palabras se parecen.
pub fn vector_falso(texto: &str) -> Vec<f64> {
    let mut v = vec![0.0; 64];
    for palabra in texto.to_lowercase().replace('\n', " ").split_whitespace() {
        let p = palabra.trim_matches(|c| ".,;:¿?¡!()[]#»«".contains(c));
        if p.chars().count() >= 3 {
            v[crc32(p.as_bytes()) as usize % 64] += 1.0;
        }
    }
    v[63] += 0.01;
    v
}

fn json_resp(c: &mut TcpStream, datos: &Value, codigo: u16) {
    let cuerpo = datos.to_string();
    let _ = write!(
        c,
        "HTTP/1.1 {codigo} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    );
}

fn empezar_stream(c: &mut TcpStream) {
    // HTTP/1.0 sin Content-Length: se cierra al terminar.
    let _ = write!(c, "HTTP/1.0 200 OK\r\nContent-Type: application/x-ndjson\r\n\r\n");
    let _ = c.flush();
}

fn linea(c: &mut TcpStream, datos: &Value) -> bool {
    c.write_all(format!("{datos}\n").as_bytes()).is_ok() && c.flush().is_ok()
}

fn callado(segundos: f64) {
    std::thread::sleep(Duration::from_secs_f64(segundos));
}

fn atender(mut c: TcpStream, estado: &EstadoFalso) {
    let mut r = BufReader::new(c.try_clone().unwrap());
    let mut primera = String::new();
    if r.read_line(&mut primera).unwrap_or(0) == 0 {
        return;
    }
    let mut largo = 0;
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
            largo = v.trim().parse().unwrap_or(0);
        }
    }
    let mut cuerpo = vec![0; largo];
    let _ = r.read_exact(&mut cuerpo);
    let datos: Value = serde_json::from_slice(&cuerpo).unwrap_or(json!({}));
    let mut p = primera.split_whitespace();
    let (metodo, ruta) = (p.next().unwrap_or(""), p.next().unwrap_or(""));
    estado.peticiones.lock().unwrap().push(ruta.to_string());
    let modelo = datos["model"].as_str().unwrap_or("").to_string();
    match (metodo, ruta) {
        ("GET", "/api/version") => json_resp(&mut c, &json!({"version": "0.99.0"}), 200),
        ("GET", "/api/tags") => json_resp(
            &mut c,
            &json!({"models": [
                {"name": "qwen3:8b", "size": 5_200_000_000u64, "modified_at": "2026-11-01T10:00:00Z",
                 "details": {"family": "qwen3", "families": ["qwen3"], "parameter_size": "8.2B", "quantization_level": "Q4_K_M"}},
                {"name": "emb:latest", "size": 600_000_000u64, "modified_at": "2026-11-02T10:00:00Z",
                 "details": {"family": "bert", "families": ["bert"], "parameter_size": "600M", "quantization_level": "F16"}},
            ]}),
            200,
        ),
        ("GET", "/api/ps") => json_resp(
            &mut c,
            &json!({"models": [{"name": "qwen3:8b", "size": 6_000_000_000u64, "size_vram": 4_500_000_000u64,
                                "expires_at": "2026-11-07T12:00:00Z"}]}),
            200,
        ),
        ("DELETE", "/api/delete") => {
            if modelo == "qwen3:8b" {
                estado.borrados.lock().unwrap().push(modelo);
                json_resp(&mut c, &json!({}), 200);
            } else {
                json_resp(
                    &mut c,
                    &json!({"error": format!("model '{modelo}' not found")}),
                    404,
                );
            }
        }
        ("POST", "/api/pull") => {
            empezar_stream(&mut c);
            if modelo == "noexiste:1b" {
                linea(&mut c, &json!({"status": "pulling manifest"}));
                linea(
                    &mut c,
                    &json!({"error": "pull model manifest: file does not exist"}),
                );
                return;
            }
            linea(&mut c, &json!({"status": "pulling manifest"}));
            if modelo == "lento:1b" {
                linea(
                    &mut c,
                    &json!({"status": "pulling aaa", "digest": "sha256:aaa", "total": 1000, "completed": 10}),
                );
                callado(5.0);
                return;
            }
            for completado in [0, 250, 500, 750, 1000] {
                linea(
                    &mut c,
                    &json!({"status": "pulling aaa", "digest": "sha256:aaa", "total": 1000, "completed": completado}),
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            for completado in [0, 500] {
                linea(
                    &mut c,
                    &json!({"status": "pulling bbb", "digest": "sha256:bbb", "total": 500, "completed": completado}),
                );
            }
            for e in ["verifying sha256 digest", "writing manifest", "success"] {
                linea(&mut c, &json!({"status": e}));
            }
        }
        ("POST", "/api/chat") => {
            estado.chats.lock().unwrap().push(datos.clone());
            if modelo == "noexiste" {
                return json_resp(
                    &mut c,
                    &json!({"error": format!("model '{modelo}' not found")}),
                    404,
                );
            }
            empezar_stream(&mut c);
            let msg = |t: &str| json!({"message": {"role": "assistant", "content": t}, "done": false});
            match modelo.as_str() {
                "pensador" => {
                    for t in ["<thi", "nk>Hmm, ", "pienso</th", "ink>Hola", " mundo", "<"] {
                        linea(&mut c, &msg(t));
                    }
                }
                "campo" => {
                    linea(
                        &mut c,
                        &json!({"message": {"role": "assistant", "content": "", "thinking": "Primero pienso"}, "done": false}),
                    );
                    linea(&mut c, &msg("Respuesta"));
                }
                "lento" => {
                    linea(&mut c, &msg("Empiezo"));
                    callado(5.0);
                    return;
                }
                "paso_a_paso" => {
                    // La respuesta que la interfaz ve llegar «por trozos» (con pensamiento).
                    for t in ["Veamos, ", "lo pienso."] {
                        linea(
                            &mut c,
                            &json!({"message": {"role": "assistant", "content": "", "thinking": t}, "done": false}),
                        );
                        std::thread::sleep(Duration::from_millis(30));
                    }
                    for t in [
                        "Hola, ",
                        "esto ",
                        "es ",
                        "**Markdown** ",
                        "que ",
                        "llega ",
                        "por ",
                        "trozos. ",
                        "Mira [[Ideas]].",
                    ] {
                        if !linea(&mut c, &msg(t)) {
                            return;
                        }
                        std::thread::sleep(Duration::from_millis(70));
                    }
                }
                _ => {
                    let ultimo = datos["messages"]
                        .as_array()
                        .and_then(|m| m.last())
                        .map_or("", |m| m["content"].as_str().unwrap_or(""));
                    linea(&mut c, &msg(&format!("Eco: {ultimo}")));
                }
            }
            linea(
                &mut c,
                &json!({"message": {"role": "assistant", "content": ""}, "done": true,
                                   "eval_count": 10, "eval_duration": 1_000_000_000u64}),
            );
        }
        ("POST", "/api/embed") => {
            if modelo != "emb:latest" && modelo != "emb" {
                return json_resp(
                    &mut c,
                    &json!({"error": format!("model '{modelo}' not found")}),
                    404,
                );
            }
            let textos: Vec<String> = match &datos["input"] {
                Value::String(s) => vec![s.clone()],
                Value::Array(a) => a.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect(),
                _ => vec![],
            };
            estado.textos_embebidos.fetch_add(textos.len(), Ordering::SeqCst);
            let v: Vec<Vec<f64>> = textos.iter().map(|t| vector_falso(t)).collect();
            json_resp(&mut c, &json!({"model": modelo, "embeddings": v}), 200);
        }
        ("POST", "/api/generate") => json_resp(&mut c, &json!({"done": true}), 200),
        _ => json_resp(&mut c, &json!({"error": "no existe"}), 404),
    }
}

impl ServidorFalso {
    pub fn new() -> ServidorFalso {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let estado = Arc::new(EstadoFalso::default());
        let vivo = Arc::new(AtomicBool::new(true));
        let (e, v) = (estado.clone(), vivo.clone());
        std::thread::spawn(move || {
            for c in l.incoming().flatten() {
                if !v.load(Ordering::SeqCst) {
                    break;
                }
                let e = e.clone();
                std::thread::spawn(move || atender(c, &e));
            }
        });
        ServidorFalso { url, estado, vivo }
    }
}

impl Default for ServidorFalso {
    fn default() -> Self {
        Self::new()
    }
}

/// Un puerto en el que nadie escucha.
pub fn puerto_cerrado() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
