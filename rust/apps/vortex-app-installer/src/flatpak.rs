//! Flatpak para tu usuario (`flatpak --user`): sin contraseña, como la tienda (`flatpak.py`).
//!
//! Instala un .flatpakref, un .flatpak (paquete suelto) o una app de Flathub, con el avance que va diciendo
//! `flatpak`, y desinstala lo que se instaló así.

use crate::util::matar_grupo;
use crate::{Contexto, Control, Error, Resultado};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

pub const REMOTO: &str = "flathub";
pub const URL_REPO: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";

fn exe(ctx: &Contexto) -> Resultado<String> {
    ctx.which("flatpak")
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| Error::Mensaje(ctx.tr("falta_flatpak", &[])))
}

pub fn hay_flatpak(ctx: &Contexto) -> bool {
    ctx.which("flatpak").is_some()
}

/// `{app_id, titulo, nombre_programa}` de un .flatpakref (un archivo de texto tipo INI); vacío si no sirve.
pub fn leer_ref(ruta: &Path) -> Map<String, Value> {
    let Ok(texto) = std::fs::read_to_string(ruta) else {
        return Map::new();
    };
    let mut seccion = String::new();
    let (mut nombre, mut titulo) = (String::new(), String::new());
    let mut hay = false;
    for linea in texto.lines() {
        let l = linea.trim();
        if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
            continue;
        }
        if let Some(s) = l.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            seccion = s.to_string();
            hay |= seccion == "Flatpak Ref";
            continue;
        }
        if seccion == "Flatpak Ref"
            && let Some((k, v)) = l.split_once(['=', ':'])
        {
            match k.trim().to_lowercase().as_str() {
                "name" => nombre = v.trim().to_string(),
                "title" => titulo = v.trim().to_string(),
                _ => {}
            }
        }
    }
    if !hay {
        return Map::new();
    }
    let programa = if titulo.is_empty() { nombre.clone() } else { titulo.clone() };
    Map::from_iter([
        ("app_id".into(), json!(nombre)),
        ("titulo".into(), json!(titulo)),
        ("nombre_programa".into(), json!(programa)),
    ])
}

pub fn apps_instaladas(ctx: &Contexto) -> BTreeSet<String> {
    let Ok(exe) = exe(ctx) else {
        return BTreeSet::new();
    };
    let mut c = std::process::Command::new(exe);
    ctx.preparar(&mut c);
    c.args(["list", "--user", "--app", "--columns=application"]);
    match crate::proc::ejecutar(c, None, Some(Duration::from_secs(60))) {
        Ok(r) => r
            .texto_out()
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => BTreeSet::new(),
    }
}

/// Corre `flatpak` con el avance (el último «NN%» que diga) y la cancelación.
fn correr(ctx: &Contexto, args: &[&str], control: &Control) -> Resultado<String> {
    let mut c = ctx.comando(&exe(ctx)?);
    c.args(args)
        .env("LC_ALL", "C.UTF-8")
        .env("LANG", "C.UTF-8")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::util::nueva_sesion(&mut c);
    let mut hijo = c.spawn()?;
    let pid = hijo.id();
    let salida = Arc::new(Mutex::new(Vec::<u8>::new()));
    let mut lectores = Vec::new();
    let flujos: Vec<Box<dyn Read + Send>> = [
        hijo.stdout.take().map(|o| Box::new(o) as Box<dyn Read + Send>),
        hijo.stderr.take().map(|e| Box::new(e) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .collect();
    for mut f in flujos {
        let salida = Arc::clone(&salida);
        let control = control.clone();
        lectores.push(std::thread::spawn(move || {
            static PORCIENTO: OnceLock<Regex> = OnceLock::new();
            let r = PORCIENTO.get_or_init(|| Regex::new(r"(\d{1,3})%").expect("regex"));
            let mut buf = [0u8; 4096];
            loop {
                let n = match f.read(&mut buf) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                let Ok(mut s) = salida.lock() else { return };
                s.extend_from_slice(&buf[..n]);
                if control.hay_progreso() {
                    let desde = s.len().saturating_sub(400);
                    let trozo = String::from_utf8_lossy(&s[desde..]).into_owned();
                    if let Some(m) = r.captures_iter(&trozo).last()
                        && let Ok(n) = m[1].parse::<u32>()
                    {
                        control.avanzar((f64::from(n) / 100.0).min(1.0));
                    }
                }
            }
        }));
    }
    let estado = loop {
        match hijo.try_wait() {
            Ok(Some(e)) => break e,
            Ok(None) => {}
            Err(e) => return Err(Error::Io(e)),
        }
        if control.cancelado() {
            matar_grupo(pid, libc::SIGTERM);
            let _ = hijo.wait();
            for l in lectores {
                let _ = l.join();
            }
            return Err(Error::Cancelado);
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    for l in lectores {
        let _ = l.join();
    }
    let texto = String::from_utf8_lossy(&salida.lock().map(|s| s.clone()).unwrap_or_default()).into_owned();
    if !estado.success() {
        let t = texto.to_lowercase();
        if ["could not resolve", "network", "unable to connect", "timeout"]
            .iter()
            .any(|x| t.contains(x))
        {
            return Err(Error::Mensaje(ctx.tr("sin_internet", &[])));
        }
        if t.contains("already installed") {
            return Ok(texto);
        }
        return Err(Error::Mensaje(ctx.tr("flatpak_fallo", &[])));
    }
    Ok(texto)
}

pub fn preparar_flathub(ctx: &Contexto) -> Resultado<()> {
    correr(
        ctx,
        &["remote-add", "--user", "--if-not-exists", REMOTO, URL_REPO],
        &Control::nulo(),
    )
    .map(|_| ())
}

/// Instala un .flatpakref o un .flatpak. Devuelve el id de la app instalada (si se pudo saber).
pub fn instalar_archivo(ctx: &Contexto, ruta: &Path, tipo: &str, control: &Control) -> Resultado<Option<String>> {
    let antes = apps_instaladas(ctx);
    let opcion = if tipo == "flatpakref" { "--from" } else { "--bundle" };
    correr(
        ctx,
        &["install", "--user", "-y", "--noninteractive", opcion, &ruta.to_string_lossy()],
        control,
    )?;
    let nuevas: BTreeSet<String> = apps_instaladas(ctx).difference(&antes).cloned().collect();
    if tipo == "flatpakref"
        && let Some(app) = leer_ref(ruta).get("app_id").and_then(Value::as_str).filter(|s| !s.is_empty())
    {
        return Ok(Some(app.to_string()));
    }
    Ok(nuevas.into_iter().next())
}

pub fn instalar_flathub(ctx: &Contexto, app_id: &str, control: &Control) -> Resultado<String> {
    preparar_flathub(ctx)?;
    correr(ctx, &["install", "--user", "-y", "--noninteractive", REMOTO, app_id], control)?;
    Ok(app_id.to_string())
}

pub fn desinstalar(ctx: &Contexto, app_id: &str) -> Resultado<()> {
    correr(ctx, &["uninstall", "--user", "-y", "--noninteractive", app_id], &Control::nulo()).map(|_| ())
}

pub fn abrir(ctx: &Contexto, app_id: &str) -> Resultado<()> {
    ctx.lanzar(&[exe(ctx)?, "run".into(), app_id.into()], &[], None)
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn lee_un_flatpakref() {
        let t = vortex_core::carpeta_de_prueba("flatpakref");
        let r = t.join("a.flatpakref");
        std::fs::write(&r, "[Flatpak Ref]\nName=org.example.App\nTitle=Ejemplo\nUrl=https://x/\n").unwrap();
        let m = leer_ref(&r);
        assert_eq!(m["app_id"], "org.example.App");
        assert_eq!(m["nombre_programa"], "Ejemplo");
        std::fs::write(&r, "esto no es un ref").unwrap();
        assert!(leer_ref(&r).is_empty());
        let _ = std::fs::remove_dir_all(&t);
    }
}
