use std::os::unix::process::CommandExt;
use std::process::{Command, ExitCode};

use vortex_escritorio::{
    CATALOGO, Escritorio, buscar, instalado, linea_para_login, orden_de_arranque, sesiones, texto_lista,
    variables,
};

/// Vortex Shell, la sesión de siempre (lo que hace `entrar` sin nada).
const ENTRAR_VORTEX: &str = "/usr/share/vortex/greeter/entrar";

const AYUDA: &str = "vortex escritorio — otros escritorios además de Vortex Shell

  vortex escritorio                     los que tienes y los que se pueden instalar
  vortex escritorio instalar NOMBRE     kde, gnome, cosmic o xfce (pide tu contraseña)
  vortex escritorio quitar NOMBRE       lo desinstala (Vortex Shell no se puede quitar)

Para cambiar: cierra sesión y elige el escritorio abajo a la izquierda del inicio de sesión.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("lista") => {
            print!("{}", texto_lista(&sesiones()));
            ExitCode::SUCCESS
        }
        Some("sesiones") => {
            for s in sesiones() {
                println!("{}", linea_para_login(&s));
            }
            ExitCode::SUCCESS
        }
        Some("entrar") => entrar(args.get(1).map(String::as_str).unwrap_or("")),
        Some(accion @ ("instalar" | "quitar")) => {
            let Some(e) = args.get(1).and_then(|c| buscar(c)) else {
                eprintln!(
                    "¿Cuál? Puede ser: {}.",
                    CATALOGO.iter().map(|e| e.clave).collect::<Vec<_>>().join(", ")
                );
                return ExitCode::from(2);
            };
            if accion == "instalar" {
                instalar(e)
            } else {
                quitar(e)
            }
        }
        Some("-h" | "--help" | "ayuda") => {
            println!("{AYUDA}");
            ExitCode::SUCCESS
        }
        Some(otro) => {
            eprintln!("No conozco «{otro}».\n\n{AYUDA}");
            ExitCode::from(2)
        }
    }
}

/// Arranca la sesión `id` (la llama greetd por `config/greeter/entrar`). Si no existe o no arranca,
/// entra a Vortex Shell: nunca te quedas sin escritorio.
fn entrar(id: &str) -> ExitCode {
    let path = std::env::var("PATH").unwrap_or_default();
    let vt = std::env::var("XDG_VTNR").ok();
    if let Some(s) = sesiones().into_iter().find(|s| s.id == id) {
        match orden_de_arranque(&s, &path, vt.as_deref()) {
            Ok(orden) => {
                let mut c = Command::new(&orden[0]);
                c.args(&orden[1..]);
                for (k, v) in variables(&s) {
                    c.env(k, v);
                }
                let e = c.exec();
                eprintln!("vortex-escritorio: no pude arrancar «{}»: {e}", s.nombre);
            }
            Err(e) => eprintln!("vortex-escritorio: {e}"),
        }
    } else if !id.is_empty() && id != "vortex" {
        eprintln!("vortex-escritorio: ya no está la sesión «{id}»; entro a Vortex Shell.");
    }
    let e = Command::new(ENTRAR_VORTEX).exec();
    eprintln!("vortex-escritorio: tampoco pude abrir Vortex Shell: {e}");
    ExitCode::FAILURE
}

fn pacman(args: &[&str], paquetes: &[&str]) -> bool {
    Command::new("sudo")
        .arg("pacman")
        .args(args)
        .args(paquetes)
        .status()
        .is_ok_and(|s| s.success())
}

fn instalar(e: &Escritorio) -> ExitCode {
    if instalado(e, &sesiones()) {
        println!(
            "{} ya está instalado. Cierra sesión y elígelo abajo a la izquierda del inicio de sesión.",
            e.nombre
        );
        return ExitCode::SUCCESS;
    }
    println!("Voy a instalar {} ({}).", e.nombre, e.paquetes.join(" "));
    if !pacman(&["-S", "--needed"], e.paquetes) {
        eprintln!("No se instaló {}.", e.nombre);
        return ExitCode::FAILURE;
    }
    println!(
        "\n{} listo. Para usarlo: cierra sesión y, en el inicio de sesión, elígelo abajo a la izquierda.\n\
         Vortex Shell sigue ahí: vuelves a él igual, desde el inicio de sesión.",
        e.nombre
    );
    ExitCode::SUCCESS
}

/// Quita solo los paquetes que están instalados (los grupos, como `xfce4`, por sus paquetes).
fn quitar(e: &Escritorio) -> ExitCode {
    let mut quitar: Vec<String> = Vec::new();
    for p in e.paquetes {
        let solo = Command::new("pacman").args(["-Qq", p]).output();
        if solo.as_ref().is_ok_and(|o| o.status.success()) {
            quitar.push((*p).to_string());
            continue;
        }
        if let Ok(o) = Command::new("pacman").args(["-Qqg", p]).output()
            && o.status.success()
        {
            quitar.extend(String::from_utf8_lossy(&o.stdout).lines().map(String::from));
        }
    }
    if quitar.is_empty() {
        println!("{} no está instalado.", e.nombre);
        return ExitCode::SUCCESS;
    }
    let refs: Vec<&str> = quitar.iter().map(String::as_str).collect();
    if !pacman(&["-Rs"], &refs) {
        eprintln!("No se quitó {}.", e.nombre);
        return ExitCode::FAILURE;
    }
    println!(
        "{} quitado. Si era el que usabas, la próxima vez entras a Vortex Shell.",
        e.nombre
    );
    ExitCode::SUCCESS
}
