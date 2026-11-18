//! Las últimas 60 muestras de cada medida, para las gráficas de «Rendimiento».

use serde_json::{Map, Value, json};

pub const HISTORIA: usize = 60;

fn empujar(lista: &mut Value, valor: f64) {
    if !lista.is_array() {
        *lista = json!([]);
    }
    let a = lista.as_array_mut().expect("es una lista");
    a.push(json!(valor));
    if a.len() > HISTORIA {
        a.drain(..a.len() - HISTORIA);
    }
}

fn por_id<'a>(mapa: &'a mut Value, clave: &str, id: &str, campos: &[&str]) -> &'a mut Value {
    let m: &mut Map<String, Value> = mapa[clave].as_object_mut().expect("es un mapa");
    m.entry(id.to_string())
        .or_insert_with(|| Value::Object(campos.iter().map(|c| (c.to_string(), json!([]))).collect()))
}

pub struct Historia {
    pub valor: Value,
}

impl Default for Historia {
    fn default() -> Self {
        Self::new()
    }
}

impl Historia {
    pub fn new() -> Self {
        Historia {
            valor: json!({"cpu": [], "mem": [], "nucleos": [], "discos": {}, "redes": {}, "gpus": {}}),
        }
    }

    /// Agrega una muestra de `Rendimiento::muestrear`.
    pub fn guardar(&mut self, r: &Value) {
        let h = &mut self.valor;
        empujar(&mut h["cpu"], r["cpu"]["uso"].as_f64().unwrap_or(0.0));
        empujar(&mut h["mem"], r["mem"]["uso"].as_f64().unwrap_or(0.0));
        let nucleos = r["cpu"]["nucleos"].as_array().cloned().unwrap_or_default();
        while h["nucleos"].as_array().map_or(0, Vec::len) < nucleos.len() {
            h["nucleos"].as_array_mut().expect("es una lista").push(json!([]));
        }
        for (i, v) in nucleos.iter().enumerate() {
            empujar(&mut h["nucleos"][i], v.as_f64().unwrap_or(0.0));
        }
        for d in r["discos"].as_array().into_iter().flatten() {
            let id = d["id"].as_str().unwrap_or("");
            let hd = por_id(h, "discos", id, &["activo", "lectura", "escritura"]);
            for k in ["activo", "lectura", "escritura"] {
                empujar(&mut hd[k], d[k].as_f64().unwrap_or(0.0));
            }
        }
        for n in r["redes"].as_array().into_iter().flatten() {
            let id = n["id"].as_str().unwrap_or("");
            let hn = por_id(h, "redes", id, &["envio", "recepcion"]);
            for k in ["envio", "recepcion"] {
                empujar(&mut hn[k], n[k].as_f64().unwrap_or(0.0));
            }
        }
        for g in r["gpus"].as_array().into_iter().flatten() {
            let id = g["id"].as_str().unwrap_or("");
            let (usada, total) = (
                g["vramUsada"].as_f64().unwrap_or(0.0),
                g["vramTotal"].as_f64().unwrap_or(0.0),
            );
            let hg = por_id(h, "gpus", id, &["uso", "vram"]);
            empujar(&mut hg["uso"], g["uso"].as_f64().unwrap_or(0.0).max(0.0));
            empujar(
                &mut hg["vram"],
                if total > 0.0 { 100.0 * usada / total } else { 0.0 },
            );
        }
    }
}
