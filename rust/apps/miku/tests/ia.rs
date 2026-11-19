//! Las pruebas de `apps/miku/pruebas/ia.py`: el cerebro con cualquier IA (`cerebro_abierto` e `ia`), sin
//! internet ni GPU. Una IA falsa compatible con OpenAI (contesta en flujo y, la primera vez, pide una
//! herramienta), un MCP falso y un Ollama falso.

mod comun;

use comun::*;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use vortex_miku::cerebro_abierto::CerebroAbierto;
use vortex_miku::config::Config;
use vortex_miku::eventos::{Cerebro, Evento, Recolector};
use vortex_miku::ia::{self, Ia};
use vortex_miku::rutas::Rutas;

struct Mundo {
    _d: tempfile::TempDir,
    rutas: Rutas,
    srv: Servidor,
    estado: Arc<Mutex<(bool, u16)>>, // (modelo bajado, código de error)
    cargas: Arc<Mutex<Vec<Value>>>,
}

fn mundo() -> Mundo {
    let d = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(d.path());
    std::fs::create_dir_all(&rutas.config_dir).unwrap();
    let estado = Arc::new(Mutex::new((false, 200u16)));
    let cargas = Arc::new(Mutex::new(vec![]));
    let (e2, c2) = (estado.clone(), cargas.clone());
    let srv = servidor(move |p| {
        let (bajado, codigo) = *e2.lock().unwrap();
        match (p.metodo.as_str(), p.ruta.as_str()) {
            ("GET", "/api/version") => json(json!({"version": "0.0.0"})),
            (_, "/api/show") if bajado => json(json!({})),
            (_, "/api/show") => (404, "application/json", b"{}".to_vec()),
            (_, "/api/generate") => {
                c2.lock().unwrap().push(p.json());
                json(json!({"done": true}))
            }
            (_, "/api/pull") => {
                e2.lock().unwrap().0 = true;
                let mut c = String::new();
                for hecho in [0, 500, 1000] {
                    c += &format!("{}\n", json!({"status": "pulling", "digest": "sha256:a", "total": 1000, "completed": hecho}));
                }
                c += "{\"status\": \"success\"}\n";
                (200, "application/x-ndjson", c.into_bytes())
            }
            _ => {
                if codigo != 200 {
                    return (codigo, "application/json", json!({"error": {"message": "no"}}).to_string().into_bytes());
                }
                let d = p.json();
                if d["stream"] != json!(true) {
                    // mejores momentos: una respuesta entera
                    let t = r#"{"momentos": [{"candidato": 2, "titulo": "La remontada", "por_que": "gritos"}]}"#;
                    return json(json!({"choices": [{"message": {"role": "assistant", "content": t}}]}));
                }
                let ultimo = d["messages"].as_array().unwrap().last().unwrap().clone();
                if ultimo["role"] == "user" && ultimo["content"] == "mira mi pantalla" {
                    sse(&[
                        json!({"content": "Déjame ver. "}),
                        json!({"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "prueba__eco", "arguments": "{\"texto\":"}}]}),
                        json!({"tool_calls": [{"index": 0, "function": {"arguments": " \"hola\"}"}}]}),
                    ])
                } else {
                    sse(&[json!({"content": "Ya vi. "}), json!({"content": "Todo bien."})])
                }
            }
        }
    });
    Mundo { _d: d, rutas, srv, estado, cargas }
}

fn nueva_ia(m: &Mundo) -> Arc<Ia> {
    let mut ia = Ia::nueva(m.rutas.clone());
    ia.ollama = m.srv.url.clone();
    ia.llavero = false; // sin tocar el llavero de nadie
    Arc::new(ia)
}

fn cfg(m: &Mundo, ia_id: &str, modelo: &str) -> Config {
    Config { ia: ia_id.into(), ia_url: format!("{}/v1", m.srv.url), ia_modelo: modelo.into(), ..Config::default() }
}

fn cerebro(m: &Mundo, c: &Config) -> (CerebroAbierto, Recolector) {
    let r = Recolector::default();
    let mcp = m.rutas.config_dir.join("mcp_falso.py");
    std::fs::write(&mcp, MCP_FALSO).unwrap();
    let srvs = vec![("prueba".to_string(), vec!["python3".to_string(), "-I".into(), mcp.to_string_lossy().into_owned()], vec![])];
    (CerebroAbierto::nuevo(&m.rutas, nueva_ia(m), false, r.al_evento(), c, Some(srvs)), r)
}

#[test]
fn configuracion_y_proveedores() {
    let m = mundo();
    let ia = nueva_ia(&m);
    let c = Config { ia: "gemini".into(), ia_modelo: "otro-modelo".into(), ..Config::default() };
    assert_eq!(ia.elegido(&c).modelo, "otro-modelo");
    let c = Config { ia: "inventada".into(), ..Config::default() };
    assert_eq!(ia.elegido(&c).id, "claude");
    let local = ia.elegido(&Config { ia: "local".into(), ..Config::default() });
    assert_eq!(local.extra["reasoning_effort"], "none");
    assert_eq!((local.extra["temperature"].as_f64(), local.extra["top_p"].as_f64()), (Some(0.7), Some(0.8)));
    assert_eq!(local.url, format!("{}/v1", m.srv.url));
    assert_eq!(local.modelo, ia::MODELO_LOCAL);
    assert!(vortex_miku::cerebro_abierto::hoy().starts_with("\n\nHoy es "));
}

#[test]
fn clave_en_archivo_0600() {
    use std::os::unix::fs::PermissionsExt;
    let m = mundo();
    let ia = nueva_ia(&m);
    let donde = ia.guardar_clave("gemini", "clave-secreta").unwrap();
    assert!(donde.ends_with("ia-gemini.clave"));
    assert_eq!(std::fs::metadata(ia.archivo_clave("gemini")).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(ia.leer_clave("gemini"), "clave-secreta");
    ia.guardar_clave("gemini", "\x1b[200~AIza-clave_1\x1b[201~\n").unwrap();
    assert_eq!(ia.leer_clave("gemini"), "AIza-clave_1", "se quita el pegado entre corchetes");
    std::fs::write(ia.archivo_clave("gemini"), "[200~AIza-vieja [201~\n").unwrap();
    assert_eq!(ia.leer_clave("gemini"), "AIza-vieja");
    ia.guardar_clave("gemini", "clave-secreta").unwrap();
    assert!(!std::fs::read_to_string(&m.rutas.config).unwrap_or_default().contains("clave-secreta"), "no va en miku.json");
    ia.borrar_clave("gemini");
    assert_eq!(ia.leer_clave("gemini"), "");
}

#[test]
fn conversacion_con_herramienta_e_imagen() {
    let m = mundo();
    let (c, r) = cerebro(&m, &cfg(&m, "otra", "falso"));
    c.preguntar("mira mi pantalla", 1);
    let ev = r.eventos();
    assert_eq!(ev.first().unwrap().1, Evento::Inicio);
    assert!(matches!(ev.last().unwrap().1, Evento::Listo(_)), "{ev:?}");
    assert!(ev.contains(&(1, Evento::Herramienta("mcp__prueba__eco".into()))));
    assert_eq!(r.texto(), "Déjame ver. Ya vi. Todo bien.");
    let pedidos = m.srv.pedidos.lock().unwrap();
    let ultimo = pedidos.iter().rev().find(|p| p.ruta == "/v1/chat/completions").unwrap().json();
    let msgs = ultimo["messages"].as_array().unwrap();
    let tool = msgs.iter().find(|x| x["role"] == "tool").unwrap();
    assert_eq!((tool["content"].as_str(), tool["tool_call_id"].as_str()), (Some("eco: hola"), Some("c1")));
    let imagen = &msgs.last().unwrap()["content"];
    assert_eq!(imagen[1]["image_url"]["url"], "data:image/jpeg;base64,aGVsbG8=");
    assert_eq!(ultimo["model"], "falso");
    assert_eq!(ultimo["tools"][0]["function"]["name"], "prueba__eco");
    let auth = pedidos.iter().rev().find(|p| p.ruta == "/v1/chat/completions").unwrap().cabecera("authorization");
    assert!(auth.is_none(), "sin clave guardada no manda Authorization");
    drop(pedidos);

    // Memoria: recuerda la pregunta, pero guarda la captura como texto.
    c.preguntar("¿y ahora?", 2);
    let pedidos = m.srv.pedidos.lock().unwrap();
    let msgs = pedidos.iter().rev().find(|p| p.ruta == "/v1/chat/completions").unwrap().json()["messages"].clone();
    let msgs = msgs.as_array().unwrap();
    assert!(msgs.iter().any(|x| x["content"] == "mira mi pantalla"));
    assert!(!msgs.iter().any(|x| x["content"].is_array()), "el historial no guarda la imagen");
    drop(pedidos);
    c.olvidar();
    assert!(c.historial.lock().unwrap().is_empty());
}

#[test]
fn errores_en_espanol() {
    let m = mundo();
    for (codigo, clave) in [(401u16, "clave"), (429, "cuota"), (503, "saturado")] {
        m.estado.lock().unwrap().1 = codigo;
        let (c, r) = cerebro(&m, &cfg(&m, "otra", "falso"));
        c.preguntar("hola", 3);
        let err = r.eventos().into_iter().find_map(|(_, e)| if let Evento::Error { clave, mensaje } = e { Some((clave, mensaje)) } else { None });
        assert_eq!(err.as_ref().map(|e| e.0.as_str()), Some(clave), "{codigo}: {err:?}");
    }
    m.estado.lock().unwrap().1 = 200;
    // Gemini sin clave pide la clave antes de preguntar (VORTEX_MIKU_IA_CLAVE no está puesta).
    let (c, _) = cerebro(&m, &Config { ia: "gemini".into(), ..Config::default() });
    assert_eq!(c.comprobar().0, "clave");
    let e = ia::explicar(&ia::proveedor_base("gemini", ""), 400, r#"[{"error": {"code": 400, "message": "API key not valid. Please pass a valid API key."}}]"#);
    assert!(e.clave == "clave" && e.mensaje.contains("vortex-miku ia clave"));
    // Sin conexión → «internet».
    let (c, r) = cerebro(&m, &Config { ia: "otra".into(), ia_url: "http://127.0.0.1:9/v1".into(), ia_modelo: "x".into(), ..Config::default() });
    c.preguntar("hola", 4);
    assert!(r.eventos().iter().any(|(_, e)| matches!(e, Evento::Error { clave, .. } if clave == "internet")), "{:?}", r.eventos());
}

#[test]
fn ia_local_con_ollama_falso() {
    let m = mundo();
    let ia = nueva_ia(&m);
    // Un «ollama» de mentira en un PATH solo nuestro: `ollama_instalado` lo ve; no arranca nada porque el
    // servidor falso ya contesta.
    let local = ia.elegido(&Config { ia: "local".into(), ..Config::default() });
    if ia.ollama_instalado() {
        assert_eq!(ia.comprobar(&local).0, "modelo", "sin el modelo pide bajarlo");
    }
    let avance = Mutex::new(vec![]);
    ia.bajar_modelo(&local.modelo, &|_, f| avance.lock().unwrap().push(f), &AtomicBool::new(false)).unwrap();
    let a = avance.lock().unwrap().clone();
    assert_eq!(a.last(), Some(&1.0), "{a:?}");
    assert!(a.contains(&0.5));
    assert!(ia.modelo_local_listo(&local.modelo));
    if ia.ollama_instalado() {
        assert_eq!(ia.comprobar(&local), ("ok".to_string(), String::new()));
    }
    assert!(m.cargas.lock().unwrap().is_empty(), "comprobar no sube el modelo a la GPU");
    ia.precargar(&local.modelo);
    for _ in 0..60 {
        if !m.cargas.lock().unwrap().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if ia.ollama_instalado() {
        assert_eq!(m.cargas.lock().unwrap()[0], json!({"model": ia::MODELO_LOCAL, "keep_alive": "10m"}));
        ia.precargar(&local.modelo);
        std::thread::sleep(std::time::Duration::from_millis(300));
        assert_eq!(m.cargas.lock().unwrap().len(), 1, "precargar otra vez enseguida no repite");
    }
    ia.soltar(&local.modelo);
    assert_eq!(*m.cargas.lock().unwrap().last().unwrap(), json!({"model": ia::MODELO_LOCAL, "keep_alive": 0}));
    // Cancelar la descarga.
    m.estado.lock().unwrap().0 = false;
    let e = ia.bajar_modelo(&local.modelo, &|_, _| {}, &AtomicBool::new(true)).unwrap_err();
    assert_eq!(e.clave, "cancelado");
}

#[test]
fn respuesta_entera_para_momentos() {
    let m = mundo();
    let ia = nueva_ia(&m);
    let p = ia.elegido(&cfg(&m, "otra", "falso"));
    let t = ia.completar(&p, &json!([{"role": "user", "content": "elige"}]), 30).unwrap();
    assert!(t.contains("La remontada"));
}
