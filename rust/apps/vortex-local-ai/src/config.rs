//! La configuración de la interfaz (`datos/config.json`: vault, modelo, modelo de embeddings…), igual que
//! la clase `Config` de `puente.py`: un JSON plano que las dos apps leen y escriben igual.

use serde_json::{Map, Value};
use std::path::PathBuf;

pub struct Config {
    ruta: PathBuf,
    datos: Map<String, Value>,
}

impl Config {
    pub fn new(ruta: PathBuf) -> Config {
        let datos = std::fs::read_to_string(&ruta)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| match v {
                Value::Object(m) => Some(m),
                _ => None,
            })
            .unwrap_or_default();
        Config { ruta, datos }
    }

    pub fn get(&self, clave: &str) -> Option<&Value> {
        self.datos.get(clave)
    }

    pub fn texto(&self, clave: &str) -> String {
        self.datos
            .get(clave)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    }

    pub fn booleano(&self, clave: &str, defecto: bool) -> bool {
        match self.datos.get(clave) {
            Some(Value::Bool(b)) => *b,
            Some(Value::Null) | None => defecto,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        }
    }

    /// Guarda la clave (solo si cambió), con un temporal y `rename`, como la de Python.
    pub fn poner(&mut self, clave: &str, valor: Value) {
        if self.datos.get(clave) == Some(&valor) {
            return;
        }
        self.datos.insert(clave.to_string(), valor);
        let _ = self.guardar();
    }

    fn guardar(&self) -> std::io::Result<()> {
        if let Some(p) = self.ruta.parent() {
            std::fs::create_dir_all(p)?;
        }
        let temporal = self.ruta.with_extension("tmp");
        std::fs::write(
            &temporal,
            serde_json::to_string_pretty(&Value::Object(self.datos.clone()))?,
        )?;
        std::fs::rename(temporal, &self.ruta)
    }
}
