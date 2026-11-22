//! Lo que le pregunta a Hyprland por su socket (lo mismo que `hyprctl -j`, sin abrir procesos):
//! la pantalla, las ventanas del escritorio que se ve, dónde está el mouse y si hay algo en pantalla
//! completa. Cada respuesta se convierte a coordenadas de la pantalla donde vive la compañera.

use crate::mundo::{Entorno, Orilla};
use serde_json::Value;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

fn socket() -> Option<PathBuf> {
    let firma = std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")?;
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    Some(runtime.join("hypr").join(firma).join(".socket.sock"))
}

/// `hyprctl -j <orden>`; None si Hyprland no responde.
pub fn pedir(orden: &str) -> Option<Value> {
    let mut s = UnixStream::connect(socket()?).ok()?;
    s.set_read_timeout(Some(Duration::from_millis(500))).ok()?;
    s.write_all(format!("j/{orden}").as_bytes()).ok()?;
    let mut texto = String::new();
    s.read_to_string(&mut texto).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Una pantalla, en pixeles lógicos.
#[derive(Clone, Debug, PartialEq)]
pub struct Pantalla {
    pub nombre: String,
    pub x: f32,
    pub y: f32,
    pub ancho: f32,
    pub alto: f32,
    /// Lo que reservan las barras: izquierda, arriba, derecha, abajo.
    pub reservado: [f32; 4],
    pub escritorio: i64,
    pub especial: i64,
    pub enfocada: bool,
}

pub fn pantallas(v: &Value) -> Vec<Pantalla> {
    let Some(lista) = v.as_array() else { return vec![] };
    lista
        .iter()
        .filter_map(|m| {
            let escala = m["scale"].as_f64().filter(|e| *e > 0.0).unwrap_or(1.0) as f32;
            // Con la pantalla girada, el ancho y el alto se cambian.
            let girada = m["transform"].as_i64().is_some_and(|t| t % 2 == 1);
            let (w, h) = (
                m["width"].as_f64()? as f32 / escala,
                m["height"].as_f64()? as f32 / escala,
            );
            let (ancho, alto) = if girada { (h, w) } else { (w, h) };
            let r = m["reserved"].as_array();
            let res = |i: usize| r.and_then(|r| r.get(i)).and_then(Value::as_f64).unwrap_or(0.0) as f32;
            Some(Pantalla {
                nombre: m["name"].as_str()?.to_string(),
                x: m["x"].as_f64()? as f32,
                y: m["y"].as_f64()? as f32,
                ancho,
                alto,
                reservado: [res(0), res(1), res(2), res(3)],
                escritorio: m["activeWorkspace"]["id"].as_i64().unwrap_or(0),
                especial: m["specialWorkspace"]["id"].as_i64().unwrap_or(0),
                enfocada: m["focused"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

/// Las orillas de arriba de las ventanas que se ven en esa pantalla (en sus coordenadas). Una ventana
/// tapada por otra más arriba no cuenta en la parte tapada: se recorta para no pararse «dentro».
pub fn orillas(clientes: &Value, p: &Pantalla) -> Vec<Orilla> {
    let Some(lista) = clientes.as_array() else {
        return vec![];
    };
    let mut ventanas: Vec<(f32, f32, f32, f32)> = lista
        .iter()
        .filter(|c| {
            let ws = c["workspace"]["id"].as_i64().unwrap_or(i64::MIN);
            (ws == p.escritorio || (p.especial != 0 && ws == p.especial))
                && c["mapped"].as_bool().unwrap_or(true)
                && !c["hidden"].as_bool().unwrap_or(false)
        })
        .filter_map(|c| {
            let x = c["at"][0].as_f64()? as f32 - p.x;
            let y = c["at"][1].as_f64()? as f32 - p.y;
            let w = c["size"][0].as_f64()? as f32;
            let h = c["size"][1].as_f64()? as f32;
            (w > 40.0 && h > 40.0 && y > 8.0 && y < p.alto - p.reservado[3] - 40.0).then_some((x, y, w, h))
        })
        .collect();
    ventanas.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut res = Vec::new();
    for (i, &(x, y, w, _)) in ventanas.iter().enumerate() {
        // Partes de la orilla que tapa otra ventana (una que empieza más arriba y la cubre).
        let mut tramos = vec![(x.max(0.0), (x + w).min(p.ancho))];
        for &(ox, oy, ow, oh) in &ventanas[..i] {
            if oy < y && oy + oh > y {
                tramos = tramos
                    .into_iter()
                    .flat_map(|(a, b)| {
                        let (c, d) = (ox, ox + ow);
                        if d <= a || c >= b {
                            vec![(a, b)]
                        } else {
                            vec![(a, c.max(a)), (d.min(b), b)]
                        }
                    })
                    .filter(|(a, b)| b - a > 30.0)
                    .collect();
            }
        }
        res.extend(tramos.into_iter().map(|(x0, x1)| Orilla { x0, x1, y }));
    }
    res
}

/// El entorno completo de una pantalla. `cursor` es la posición global del mouse (para saber si se
/// movió); devuelve también si esa pantalla tiene algo en pantalla completa.
pub fn entorno(p: &Pantalla, clientes: &Value, activa: &Value) -> Entorno {
    let completa = activa["fullscreen"].as_i64().unwrap_or(0) != 0
        && activa["workspace"]["id"].as_i64() == Some(p.escritorio);
    Entorno {
        ancho: p.ancho,
        piso: p.alto - p.reservado[3],
        orillas: orillas(clientes, p),
        mouse_quieto: 0.0,
        esconder: completa,
    }
}

/// La posición del mouse (global).
pub fn cursor() -> Option<(f32, f32)> {
    let v = pedir("cursorpos")?;
    Some((v["x"].as_f64()? as f32, v["y"].as_f64()? as f32))
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use serde_json::json;

    fn pantalla() -> Pantalla {
        pantallas(&json!([{
            "name": "DP-2", "x": 1920, "y": 0, "width": 2560, "height": 1440, "scale": 1.0,
            "transform": 0, "reserved": [0, 40, 0, 0], "focused": true,
            "activeWorkspace": {"id": 2}, "specialWorkspace": {"id": 0}
        }]))
        .remove(0)
    }

    #[test]
    fn lee_la_pantalla() {
        let p = pantalla();
        assert_eq!((p.ancho, p.alto, p.escritorio), (2560.0, 1440.0, 2));
        assert_eq!(p.reservado[1], 40.0);
        let escalada = pantallas(
            &json!([{"name": "eDP-1", "x": 0, "y": 0, "width": 2880, "height": 1800,
            "scale": 1.5, "reserved": [0, 0, 0, 0], "activeWorkspace": {"id": 1}}]),
        );
        assert_eq!((escalada[0].ancho, escalada[0].alto), (1920.0, 1200.0));
    }

    #[test]
    fn solo_las_ventanas_del_escritorio_que_se_ve() {
        let p = pantalla();
        let clientes = json!([
            {"at": [2020, 300], "size": [800, 600], "workspace": {"id": 2}, "mapped": true, "hidden": false},
            {"at": [2020, 300], "size": [800, 600], "workspace": {"id": 5}, "mapped": true, "hidden": false},
            {"at": [2020, 500], "size": [800, 600], "workspace": {"id": 2}, "mapped": true, "hidden": true}
        ]);
        let o = orillas(&clientes, &p);
        assert_eq!(
            o,
            vec![Orilla {
                x0: 100.0,
                x1: 900.0,
                y: 300.0
            }]
        );
    }

    #[test]
    fn la_parte_tapada_no_cuenta() {
        let p = pantalla();
        // Una ventana alta a la izquierda tapa el principio de la orilla de la de abajo.
        let clientes = json!([
            {"at": [1920, 100], "size": [500, 1000], "workspace": {"id": 2}},
            {"at": [2120, 400], "size": [800, 300], "workspace": {"id": 2}}
        ]);
        let o = orillas(&clientes, &p);
        assert_eq!(o.len(), 2);
        assert_eq!(
            o[1],
            Orilla {
                x0: 500.0,
                x1: 1000.0,
                y: 400.0
            }
        );
    }

    #[test]
    fn pantalla_completa_la_esconde() {
        let p = pantalla();
        let e = entorno(&p, &json!([]), &json!({"fullscreen": 2, "workspace": {"id": 2}}));
        assert!(e.esconder);
        let e = entorno(&p, &json!([]), &json!({"fullscreen": 0, "workspace": {"id": 2}}));
        assert!(!e.esconder);
        assert_eq!(e.piso, 1440.0);
    }
}
