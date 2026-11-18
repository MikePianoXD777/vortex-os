//! Las órdenes de la línea de comandos que no abren ventana. Las comparten el binario del motor
//! (`vortex-connect-motor`) y el de la ventana (`vortex-connect-rs`), que las atiende antes de cargar Qt:
//! el modo viaje no necesita 100 MB.
//!
//! ```text
//!     estado          dice en qué red estás y si va cifrado
//!     redes           las redes abiertas cerca
//!     vigilar         el modo viaje (lo arranca su servicio)
//!     --version
//! ```

use crate::motor::Connect;
use crate::sistema::Sistema;
use crate::{NOMBRE, VERSION, viaje};

/// Atiende la orden si es de las que no abren ventana; `None` si no lo es.
pub fn ejecutar(args: &[String], sistema: &Sistema) -> Option<i32> {
    match args.first().map(String::as_str)? {
        "--version" => {
            println!("{NOMBRE} {VERSION} (Rust)");
            Some(0)
        }
        "estado" => {
            let e = Connect::real(sistema.clone()).estado();
            let o = |s: &str| {
                if s.is_empty() {
                    "-".to_string()
                } else {
                    s.to_string()
                }
            };
            println!(
                "{} {} {}",
                e.situacion.como_texto(),
                o(e.red.tipo.como_texto()),
                o(&e.red.ssid)
            );
            Some(0)
        }
        "redes" => {
            for r in Connect::real(sistema.clone())
                .redes
                .lista(true)
                .unwrap_or_default()
            {
                println!(
                    "{:3} %  {} {}",
                    r.senal,
                    if r.conectada { '*' } else { ' ' },
                    r.ssid
                );
            }
            Some(0)
        }
        "vigilar" => Some(viaje::vigilar(sistema.clone())),
        _ => None,
    }
}
