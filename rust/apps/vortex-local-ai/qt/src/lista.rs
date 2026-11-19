//! Las listas de filas que ven los QML (`ModeloLista` de `puente.py`, en C++: `cpp/modelo.h`). Cada clave de
//! la fila es un rol. Solo se tocan desde el hilo de la interfaz.

use crate::arranque::ffi;
use cxx_qt_lib::QVariant;
use serde_json::{Map, Value};

#[derive(Clone, Copy)]
pub struct Lista(usize);

impl Lista {
    /// Roles en orden, cada uno con su valor por defecto.
    pub fn nueva(roles: &[(&str, Value)]) -> Lista {
        let nombres: Vec<&str> = roles.iter().map(|r| r.0).collect();
        let defectos: Map<String, Value> = roles.iter().map(|r| (r.0.to_string(), r.1.clone())).collect();
        Lista(ffi::modelo_nuevo(
            &serde_json::to_string(&nombres).unwrap_or_default(),
            &Value::Object(defectos).to_string(),
        ))
    }

    pub fn poner(&self, filas: Vec<Value>) {
        ffi::modelo_poner(self.0, &Value::Array(filas).to_string());
    }

    pub fn agregar(&self, fila: Value) {
        ffi::modelo_agregar(self.0, &fila.to_string());
    }

    pub fn quitar_desde(&self, i: i32) {
        ffi::modelo_quitar_desde(self.0, i);
    }

    /// Cambia solo esos campos (y avisa solo de los roles que de verdad cambian).
    pub fn cambiar(&self, i: i32, campos: Value) {
        ffi::modelo_cambiar(self.0, i, &campos.to_string());
    }

    pub fn cuenta(&self) -> i32 {
        ffi::modelo_cuenta(self.0)
    }

    /// La fila `i` (vacía si no existe).
    pub fn fila(&self, i: i32) -> Map<String, Value> {
        match serde_json::from_str(&ffi::modelo_fila(self.0, i)) {
            Ok(Value::Object(m)) => m,
            _ => Map::new(),
        }
    }

    pub fn filas(&self) -> Vec<Map<String, Value>> {
        (0..self.cuenta()).map(|i| self.fila(i)).collect()
    }

    /// El índice de la primera fila con `clave == valor`, o -1.
    pub fn buscar(&self, clave: &str, valor: &Value) -> i32 {
        (0..self.cuenta())
            .find(|i| self.fila(*i).get(clave) == Some(valor))
            .unwrap_or(-1)
    }

    /// Para que una propiedad de QML la devuelva (el QObject adentro de un QVariant).
    pub fn variante(&self) -> QVariant {
        ffi::modelo_a_variante(self.0)
    }
}
