//! Cómo suena la voz: «normal» (la F1 tal cual, clara) o «miku» (apagada y un poco grave), como
//! `ESTILO_NORMAL`, `ESTILO_MIKU` y `procesar` de `motor_voz.py`.

use crate::audio;
use serde_json::{Map, Value, json};

pub fn normal() -> Map<String, Value> {
    json!({"tono": 0, "velocidad": 1.05, "corte": 0, "estante": 0.0, "nivel": -20.0, "compresion": 2.2})
        .as_object()
        .cloned()
        .unwrap()
}

/// «Apagada y ligeramente grave»: ~170 Hz en vez de ~196, un poco más lenta, con menos brillo y bajita.
pub fn miku() -> Map<String, Value> {
    json!({"tono": -250, "velocidad": 1.08, "corte": 7500, "estante": -3.0, "nivel": -26.0, "compresion": 3.0})
        .as_object()
        .cloned()
        .unwrap()
}

/// El de la voz: normal, o el de Miku con lo que la voz traiga en su `"estilo"` de voces.json.
pub fn de_voz(voz: &Value, con_avatar: bool) -> Map<String, Value> {
    if !con_avatar {
        return normal();
    }
    let mut e = miku();
    if let Some(propio) = voz["estilo"].as_object() {
        e.extend(propio.clone());
    }
    e
}

fn num(e: &Map<String, Value>, k: &str, defecto: f64) -> f64 {
    e.get(k).and_then(Value::as_f64).unwrap_or(defecto)
}

pub fn velocidad(e: &Map<String, Value>) -> f64 {
    num(e, "velocidad", 1.0)
}

/// Del audio crudo del motor al que suena: tono, brillo y volumen.
pub fn procesar(x: &[f32], sr: u32, e: &Map<String, Value>) -> Vec<f32> {
    let x = audio::cambiar_tono(x, num(e, "tono", 0.0));
    let corte = num(e, "corte", 0.0);
    let x = if corte != 0.0 {
        audio::filtrar_agudos(&x, sr, corte, num(e, "estante", 0.0))
    } else {
        x
    };
    audio::nivelar(&x, sr, num(e, "nivel", -22.0), -3.0, num(e, "compresion", 2.2))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn miku_con_lo_de_la_voz() {
        let v = json!({"estilo": {"tono": -100}});
        let e = de_voz(&v, true);
        assert_eq!(e["tono"], -100);
        assert_eq!(e["nivel"], -26.0);
        assert_eq!(de_voz(&v, false)["tono"], 0);
    }
}
