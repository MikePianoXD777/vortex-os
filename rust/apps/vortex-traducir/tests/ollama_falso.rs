//! La traducción contra un Ollama de mentira (un servidor HTTP mínimo en 127.0.0.1): no hace falta tener
//! Ollama ni modelos, y no se toca el de la persona.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use vortex_traducir::{Destino, modelos, traducir};

/// Contesta `/api/tags` y `/api/chat`; guarda lo que le pidieron. El modelo «viejo» no acepta `think`.
fn ollama_falso() -> (String, Arc<Mutex<Vec<Value>>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let pedidos = Arc::new(Mutex::new(Vec::new()));
    let p = pedidos.clone();
    std::thread::spawn(move || {
        for c in l.incoming().flatten() {
            let mut r = BufReader::new(c.try_clone().unwrap());
            let mut primera = String::new();
            r.read_line(&mut primera).unwrap();
            let mut largo = 0;
            loop {
                let mut h = String::new();
                r.read_line(&mut h).unwrap();
                if h.trim().is_empty() {
                    break;
                }
                if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
                    largo = v.trim().parse().unwrap();
                }
            }
            let mut cuerpo = vec![0; largo];
            r.read_exact(&mut cuerpo).unwrap();
            let respuesta = if primera.contains("/api/tags") {
                json!({"models": [{"name": "llama3.2:3b", "size": 2u64}, {"name": "viejo:1b", "size": 9u64}]})
            } else {
                let v: Value = serde_json::from_slice(&cuerpo).unwrap();
                p.lock().unwrap().push(v.clone());
                if v["model"] == "viejo:1b" && v.get("think").is_some() {
                    json!({"error": "\"viejo:1b\" does not support thinking"})
                } else {
                    let a = if v["messages"][0]["content"]
                        .as_str()
                        .unwrap()
                        .contains("al inglés")
                    {
                        "Hello, friend"
                    } else {
                        "Hola, amigo"
                    };
                    json!({"message": {"role": "assistant", "content": format!("<think>pienso</think>\n«{a}»")}})
                }
            };
            let t = respuesta.to_string();
            let mut c = c;
            let _ = write!(
                c,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{t}",
                t.len()
            );
        }
    });
    (url, pedidos)
}

#[test]
fn traduce_con_ollama() {
    let (url, pedidos) = ollama_falso();
    let tags = modelos(&url).expect("contesta");
    assert_eq!(tags["models"][0]["name"], "llama3.2:3b");
    assert_eq!(
        traducir(&url, "llama3.2:3b", "Hola, amigo", Destino::Ingles).unwrap(),
        "Hello, friend"
    );
    assert_eq!(
        traducir(&url, "llama3.2:3b", "Hello, friend", Destino::Espanol).unwrap(),
        "Hola, amigo"
    );
    let p = pedidos.lock().unwrap();
    assert_eq!(p[0]["think"], false);
    assert_eq!(p[0]["keep_alive"], "2m");
    assert_eq!(p[0]["messages"][1]["content"], "Hola, amigo");
}

#[test]
fn reintenta_sin_think() {
    let (url, pedidos) = ollama_falso();
    assert_eq!(
        traducir(&url, "viejo:1b", "Hola", Destino::Ingles).unwrap(),
        "Hello, friend"
    );
    let p = pedidos.lock().unwrap();
    assert_eq!(p.len(), 2);
    assert!(p[1].get("think").is_none());
}

#[test]
fn sin_ollama() {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    drop(l); // nadie escucha ahí
    assert!(modelos(&url).is_none());
    assert!(traducir(&url, "m", "hola", Destino::Ingles).is_err());
}
