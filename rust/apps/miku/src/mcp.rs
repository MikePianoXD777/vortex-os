//! Un cliente MCP por stdio (JSON-RPC por líneas), como los que arranca Claude Code: lo usa el cerebro de
//! las demás IA para correr las mismas herramientas (la vista de vortex-mcp y las acciones de Miku).

use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

type Esperas = Arc<Mutex<HashMap<u64, Sender<Value>>>>;

pub struct ServidorMcp {
    pub nombre: String,
    hijo: Mutex<Child>,
    entrada: Mutex<ChildStdin>,
    id: Mutex<u64>,
    esperas: Esperas,
    pub herramientas: Vec<Value>,
}

impl ServidorMcp {
    pub fn arrancar(nombre: &str, comando: &[String], env: &[(String, String)]) -> Result<Self, String> {
        let (programa, args) = comando.split_first().ok_or("comando vacío")?;
        let mut hijo = Command::new(programa)
            .args(args)
            .envs(env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("{nombre}: {e}"))?;
        let entrada = hijo.stdin.take().ok_or("sin stdin")?;
        let salida = hijo.stdout.take().ok_or("sin stdout")?;
        let esperas: Esperas = Arc::new(Mutex::new(HashMap::new()));
        {
            let esperas = esperas.clone();
            std::thread::spawn(move || {
                for l in BufReader::new(salida).lines().map_while(Result::ok) {
                    let Ok(m) = serde_json::from_str::<Value>(&l) else {
                        continue;
                    };
                    if let Some(id) = m["id"].as_u64()
                        && let Some(tx) = esperas.lock().unwrap().remove(&id)
                    {
                        let _ = tx.send(m);
                    }
                }
                // Se cayó: que nadie se quede esperando.
                for (_, tx) in esperas.lock().unwrap().drain() {
                    let _ = tx.send(json!({"error": {"message": "el servidor se cerró"}}));
                }
            });
        }
        let mut s = Self {
            nombre: nombre.into(),
            hijo: Mutex::new(hijo),
            entrada: Mutex::new(entrada),
            id: Mutex::new(0),
            esperas,
            herramientas: vec![],
        };
        s.pedir(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "vortex-miku", "version": "4.0"}}),
            Duration::from_secs(15),
        )?;
        s.mandar(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))?;
        let lista = s.pedir("tools/list", json!({}), Duration::from_secs(15))?;
        s.herramientas = lista["tools"].as_array().cloned().unwrap_or_default();
        Ok(s)
    }

    pub fn vivo(&self) -> bool {
        matches!(self.hijo.lock().unwrap().try_wait(), Ok(None))
    }

    fn mandar(&self, msg: &Value) -> Result<(), String> {
        let mut e = self.entrada.lock().unwrap();
        writeln!(e, "{msg}")
            .and_then(|_| e.flush())
            .map_err(|e| e.to_string())
    }

    pub fn pedir(&self, metodo: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        let id = {
            let mut i = self.id.lock().unwrap();
            *i += 1;
            *i
        };
        let (tx, rx) = channel();
        self.esperas.lock().unwrap().insert(id, tx);
        if self
            .mandar(&json!({"jsonrpc": "2.0", "id": id, "method": metodo, "params": params}))
            .is_err()
        {
            self.esperas.lock().unwrap().remove(&id);
            return Err(format!("{} no contestó", self.nombre));
        }
        let Ok(msg) = rx.recv_timeout(timeout) else {
            self.esperas.lock().unwrap().remove(&id);
            return Err(format!("{} no contestó", self.nombre));
        };
        if let Some(e) = msg.get("error").filter(|e| !e.is_null()) {
            return Err(e["message"]
                .as_str()
                .filter(|m| !m.is_empty())
                .unwrap_or("error")
                .to_string());
        }
        Ok(msg.get("result").cloned().unwrap_or(json!({})))
    }

    pub fn cerrar(&self) {
        let mut h = self.hijo.lock().unwrap();
        if matches!(h.try_wait(), Ok(None)) {
            let _ = h.kill();
        }
        let _ = h.wait();
    }
}

impl Drop for ServidorMcp {
    fn drop(&mut self) {
        self.cerrar();
    }
}
