//! Servidores de mentira para las pruebas: un HTTP mínimo (la IA compatible con OpenAI y Ollama) y un MCP
//! por stdio. Nunca se habla con nada real.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};

pub struct Peticion {
    pub metodo: String,
    pub ruta: String,
    pub cabeceras: Vec<(String, String)>,
    pub cuerpo: Vec<u8>,
}

impl Peticion {
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.cuerpo).unwrap_or_default()
    }
    pub fn cabecera(&self, n: &str) -> Option<String> {
        self.cabeceras.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.clone())
    }
}

/// (código, tipo, cuerpo)
pub type Contesta = (u16, &'static str, Vec<u8>);

pub struct Servidor {
    pub url: String,
    pub pedidos: Arc<Mutex<Vec<Peticion>>>,
}

fn atender(mut s: TcpStream, f: &(dyn Fn(&Peticion) -> Contesta + Send + Sync), pedidos: &Mutex<Vec<Peticion>>) {
    let mut lector = BufReader::new(s.try_clone().unwrap());
    let mut linea = String::new();
    if lector.read_line(&mut linea).unwrap_or(0) == 0 {
        return;
    }
    let mut p = linea.split_whitespace();
    let (metodo, ruta) = (p.next().unwrap_or("").to_string(), p.next().unwrap_or("").to_string());
    let (mut cabeceras, mut largo) = (vec![], 0usize);
    loop {
        let mut l = String::new();
        if lector.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = l.trim().split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                largo = v.trim().parse().unwrap_or(0);
            }
            cabeceras.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut cuerpo = vec![0u8; largo];
    let _ = lector.read_exact(&mut cuerpo);
    let pet = Peticion { metodo, ruta, cabeceras, cuerpo };
    let (codigo, tipo, datos) = f(&pet);
    pedidos.lock().unwrap().push(pet);
    let _ = write!(s, "HTTP/1.1 {codigo} X\r\nContent-Type: {tipo}\r\nConnection: close\r\n\r\n");
    let _ = s.write_all(&datos);
}

/// Levanta un servidor HTTP en un puerto libre de 127.0.0.1.
pub fn servidor(f: impl Fn(&Peticion) -> Contesta + Send + Sync + 'static) -> Servidor {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let pedidos = Arc::new(Mutex::new(vec![]));
    let (f, p2) = (Arc::new(f), pedidos.clone());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (f, p2) = (f.clone(), p2.clone());
            std::thread::spawn(move || atender(s, &*f, &p2));
        }
    });
    Servidor { url, pedidos }
}

pub fn json(v: serde_json::Value) -> Contesta {
    (200, "application/json", v.to_string().into_bytes())
}

/// Un flujo SSE al estilo OpenAI con estos «delta».
pub fn sse(deltas: &[serde_json::Value]) -> Contesta {
    let mut c = String::new();
    for d in deltas {
        c += &format!("data: {}\n\n", serde_json::json!({"choices": [{"delta": d}]}));
    }
    c += "data: [DONE]\n\n";
    (200, "text/event-stream", c.into_bytes())
}

/// Un servidor MCP por stdio (Python, como el de la prueba original): `eco` devuelve texto y una imagen.
pub const MCP_FALSO: &str = r#"
import json, sys
for linea in sys.stdin:
    m = json.loads(linea)
    if "id" not in m:
        continue
    r = {}
    if m["method"] == "tools/list":
        r = {"tools": [{"name": "eco", "description": "repite", "inputSchema": {"type": "object", "properties": {"texto": {"type": "string"}}}}]}
    elif m["method"] == "tools/call":
        r = {"content": [{"type": "text", "text": "eco: " + m["params"]["arguments"].get("texto", "")},
                         {"type": "image", "data": "aGVsbG8=", "mimeType": "image/jpeg"}]}
    print(json.dumps({"jsonrpc": "2.0", "id": m["id"], "result": r}), flush=True)
"#;
