//! Los ajustes: `~/.config/vortex/video.json`, el mismo archivo y las mismas reglas que `motor.py`.
//!
//! Una clave con el tipo equivocado se ignora (vale la de fábrica), «modo» solo es «foto» o «video» y el
//! temporizador solo 0, 3, 5 o 10 segundos. Al guardar no se pierden claves que no conocemos.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const TEMPORIZADORES: [i64; 4] = [0, 3, 5, 10];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Ajustes {
    pub modo: String,
    pub temporizador: i64,
    pub microfono: bool,
    pub cuadricula: bool,
    pub camara: String,
    pub resolucion: String,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            modo: "foto".into(),
            temporizador: 0,
            microfono: true,
            cuadricula: false,
            camara: String::new(),
            resolucion: String::new(),
        }
    }
}

pub fn ruta() -> PathBuf {
    vortex_core::config::ruta("video.json")
}

impl Ajustes {
    /// Los de `ruta()`.
    pub fn leer() -> Self {
        Self::leer_de(&ruta())
    }

    pub fn leer_de(ruta: &Path) -> Self {
        let mut a: Ajustes = vortex_core::config::cargar(ruta);
        a.validar();
        a
    }

    fn validar(&mut self) {
        if self.modo != "foto" && self.modo != "video" {
            self.modo = "foto".into();
        }
        if !TEMPORIZADORES.contains(&self.temporizador) {
            self.temporizador = 0;
        }
    }

    /// Guarda en `ruta()`. Si no se puede, la cámara sigue sirviendo (como en la de Python).
    pub fn guardar(&self) {
        self.guardar_en(&ruta());
    }

    pub fn guardar_en(&self, ruta: &Path) {
        let _ = vortex_core::config::guardar_todo(ruta, self);
    }

    /// El valor de una clave, como lo ve QML (`vv.ajuste("modo")`). `None` si no existe.
    pub fn valor(&self, clave: &str) -> Option<Value> {
        serde_json::to_value(self).ok()?.get(clave).cloned()
    }

    /// `vv.guardar(clave, valor)`: solo claves conocidas, con su mismo tipo, y solo si cambió.
    /// Devuelve `true` si cambió (entonces hay que guardar).
    pub fn poner(&mut self, clave: &str, valor: Value) -> bool {
        let Ok(Value::Object(mut mapa)) = serde_json::to_value(&*self) else {
            return false;
        };
        let Some(actual) = mapa.get(clave) else {
            return false;
        };
        if *actual == valor || !mismo_tipo(actual, &valor) {
            return false;
        }
        mapa.insert(clave.into(), valor);
        let Ok(mut nuevo) = serde_json::from_value::<Ajustes>(Value::Object(mapa)) else {
            return false;
        };
        nuevo.validar();
        if nuevo == *self {
            return false;
        }
        *self = nuevo;
        true
    }
}

fn mismo_tipo(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x.is_i64() == y.is_i64() || y.as_f64().is_some_and(|f| f.fract() == 0.0)
        }
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}
