//! La paleta de Rust sale idéntica a la de `colores.py` (apps/vortex-player). Los casos de
//! `datos/paletas.json` los generó la app de Python: la paleta gris del escritorio de Mike, Catppuccin
//! Mocha y Latte, una de matugen, una con alfa y dos rotas (que dan el respaldo).

use serde_json::Value;
use vortex_core::paleta;

fn datos() -> Value {
    serde_json::from_str(include_str!("datos/paletas.json")).expect("paletas.json")
}

#[test]
fn las_paletas_salen_iguales_que_en_python() {
    let d = datos();
    for caso in d["casos"].as_array().unwrap() {
        let nombre = caso["nombre"].as_str().unwrap();
        let rust = paleta::desde_matugen(caso["entrada"].as_object().unwrap());
        let python = &caso["salida"];
        match (rust, python) {
            (None, Value::Null) => {}
            (Some(p), Value::Object(esperado)) => {
                assert_eq!(p.len(), esperado.len(), "{nombre}: mismas claves");
                for (k, v) in esperado {
                    assert_eq!(
                        p.get(k).map(String::as_str),
                        v.as_str(),
                        "{nombre}: el color «{k}»"
                    );
                }
            }
            (r, p) => panic!("{nombre}: Rust dio {r:?} y Python {p}"),
        }
    }
}

#[test]
fn el_respaldo_es_el_mismo() {
    let d = datos();
    let esperado = d["respaldo"].as_object().unwrap();
    let r = paleta::respaldo();
    assert_eq!(r.len(), esperado.len());
    for (k, v) in esperado {
        assert_eq!(r[k], v.as_str().unwrap(), "respaldo «{k}»");
    }
}

#[test]
fn mezclas_y_contrastes_iguales() {
    let d = datos();
    for m in d["mezclas"].as_array().unwrap() {
        let (a, b, t) = (
            m[0].as_str().unwrap(),
            m[1].as_str().unwrap(),
            m[2].as_f64().unwrap(),
        );
        assert_eq!(
            paleta::mezclar(a, b, t),
            m[3].as_str().unwrap(),
            "mezclar({a}, {b}, {t})"
        );
    }
    for c in d["contrastes"].as_array().unwrap() {
        let r = paleta::contraste(c[0].as_str().unwrap(), c[1].as_str().unwrap());
        assert!((r - c[2].as_f64().unwrap()).abs() < 1e-9, "contraste {c}");
    }
}
