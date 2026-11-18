//! Las órdenes de la línea de comandos que no abren ventana. Las comparten el binario del motor
//! (`vortex-shared-motor`, el que arranca el socket de systemd) y el de la ventana (`vortex-shared-rs`), que
//! las atiende antes de cargar Qt: escuchar un aviso no necesita 100 MB.
//!
//! ```text
//! enviar DESTINO USUARIO ARCHIVOS...
//! codigo ARCHIVOS...      crea un código; sigue mandando hasta que lo cortes
//! recibir CODIGO
//! abrir-puerto            abre el puerto en ufw, si hace falta (pide la contraseña)
//! accion                  pone «Enviar con Vortexine Shared» en el clic derecho de Thunar
//! escuchar                lo arranca vortex-shared.socket
//! --version
//! ```

use crate::motor::{self, Envio, ErrorShared};
use crate::{Config, NOMBRE, VERSION, cortar, servicio, thunar};
use std::path::PathBuf;
use std::time::Duration;

fn rutas(a: &[String]) -> Vec<PathBuf> {
    a.iter().map(PathBuf::from).collect()
}

fn enviar(cfg: &Config, args: &[String]) -> i32 {
    if args.len() < 3 {
        eprintln!("Uso: vortex-shared enviar DESTINO USUARIO ARCHIVOS...");
        return 2;
    }
    cortar::instalar();
    let r = (|| -> Result<(), ErrorShared> {
        let mut envio = Envio::new(cfg, &rutas(&args[2..]))?;
        println!("Preparando…");
        envio.preparar(Duration::from_secs(900))?;
        println!("Esperando a que acepten…");
        motor::mandar(
            cfg,
            &args[0],
            &args[1],
            &envio,
            &mut |_, _| {},
            &cortar::cortado,
            Duration::from_secs(150),
        )?;
        envio.terminar();
        println!("Listo: ya llegó.");
        Ok(())
    })();
    match r {
        Ok(()) => 0,
        Err(_) if cortar::cortado() => 130,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn codigo(cfg: &Config, args: &[String]) -> i32 {
    cortar::instalar();
    let mut envio = match Envio::new(cfg, &rutas(args)) {
        Ok(e) => e,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    println!("Preparando…");
    match envio.preparar(Duration::from_secs(900)) {
        Ok(t) => {
            println!("\n{t}\n\nEn la otra compu: vortex-shared recibir CODIGO. Ctrl+C corta el envío.");
        }
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    }
    while !cortar::cortado() {
        std::thread::sleep(Duration::from_millis(200));
    }
    envio.terminar();
    0
}

fn recibir(cfg: &Config, args: &[String]) -> i32 {
    if args.len() != 1 {
        eprintln!("Uso: vortex-shared recibir CODIGO");
        return 2;
    }
    match motor::recibir(cfg, &args[0], None, &|| false, 0) {
        Ok(rutas) => {
            for r in rutas {
                println!("{}", r.display());
            }
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Atiende la orden si es de las que no abren ventana; `None` si no lo es.
pub fn ejecutar(args: &[String], cfg: &Config) -> Option<i32> {
    let (orden, resto) = args.split_first()?;
    match orden.as_str() {
        "--version" => {
            println!("{NOMBRE} {VERSION} (Rust)");
            Some(0)
        }
        "escuchar" => Some(servicio::main(cfg.clone())),
        "enviar" => Some(enviar(cfg, resto)),
        "codigo" => Some(codigo(cfg, resto)),
        "recibir" => Some(recibir(cfg, resto)),
        "abrir-puerto" => Some(i32::from(motor::abrir_puerto(cfg) != "listo")),
        "accion" => {
            thunar::poner_accion(cfg);
            Some(0)
        }
        _ => None,
    }
}
