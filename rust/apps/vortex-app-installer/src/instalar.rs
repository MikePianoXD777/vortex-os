//! Instalar AppImages y programas .tar.gz, y abrir lo que corre código (siempre ya confirmado) (`instalar.py`).
//!
//! - AppImage: se copia a ~/Applications, se le da permiso de ejecución, se sacan su .desktop y su ícono
//!   (`--appimage-extract`) y se pone en el lanzador, con un desinstalador.
//! - Programa .tar.gz: se descomprime en ~/.local/opt/<app>, se busca su ejecutable de verdad (el .desktop
//!   que trae puede tener un «@EXEC@» de relleno), y se pone en el lanzador con su ícono y un desinstalar.sh.
//! - Scripts (.sh .run .bin), .exe y .jar: se abren en una terminal o con Wine/Java. Nunca con sudo.
//!
//! Nada se escribe encima de lo que ya existía: si hace falta, [`Error::Conflicto`] y la persona decide.

use crate::comprimidos::{self, Entrada, Tipo, nombre_libre};
use crate::util::{
    basename, comillas_shell, dividir_comandos, existe_o_enlace, nombre_de, partes, raiz_de_nombre,
};
use crate::{Contexto, Control, Error, Resultado, papelera, proc, registro};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

pub const TAMANOS: [u32; 11] = [16, 22, 24, 32, 48, 64, 96, 128, 192, 256, 512];
/// Clave que se pone en los .desktop que crea la app.
pub const MARCA: &str = "X-Vortex-App-Installer";

// ------------------------------------------------------------------------------------ .desktop

/// `[(grupo, [(clave, valor), …]), …]` en el orden del archivo (los comentarios se pierden).
pub type Grupos = Vec<(String, Vec<(String, String)>)>;

pub fn leer_desktop(texto: &str) -> Grupos {
    let mut grupos: Grupos = Vec::new();
    for linea in texto.lines() {
        let s = linea.trim();
        if s.is_empty() || s.starts_with('#') {
            continue;
        }
        if s.starts_with('[') && s.ends_with(']') {
            grupos.push((s[1..s.len() - 1].to_string(), Vec::new()));
        } else if let Some((k, v)) = s.split_once('=')
            && let Some(g) = grupos.last_mut()
        {
            g.1.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    grupos
}

pub fn valor(grupos: &Grupos, clave: &str, grupo: &str) -> String {
    for (nombre, pares) in grupos {
        if nombre == grupo {
            for (k, v) in pares {
                if k == clave {
                    return v.clone();
                }
            }
        }
    }
    String::new()
}

fn valor_entrada(grupos: &Grupos, clave: &str) -> String {
    valor(grupos, clave, "Desktop Entry")
}

pub fn poner(grupos: &mut Grupos, clave: &str, nuevo: Option<&str>, grupo: &str) {
    for (nombre, pares) in grupos.iter_mut() {
        if nombre != grupo {
            continue;
        }
        if let Some(i) = pares.iter().position(|(k, _)| k == clave) {
            match nuevo {
                None => {
                    pares.remove(i);
                }
                Some(n) => pares[i].1 = n.to_string(),
            }
            return;
        }
        if let Some(n) = nuevo {
            pares.push((clave.to_string(), n.to_string()));
        }
        return;
    }
}

fn poner_entrada(grupos: &mut Grupos, clave: &str, nuevo: Option<&str>) {
    poner(grupos, clave, nuevo, "Desktop Entry");
}

pub fn escribir_desktop(grupos: &Grupos) -> String {
    grupos
        .iter()
        .map(|(g, pares)| {
            let cuerpo: Vec<String> = pares.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!("[{g}]\n{}\n", cuerpo.join("\n"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn seguro_exec() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^[A-Za-z0-9_@%+=:,./-]+$").expect("regex"))
}

/// Una ruta para la línea `Exec=` (la especificación pide comillas dobles y escapar `"` `` ` `` `$` `\`).
pub fn comillas_exec(ruta: &str) -> String {
    if seguro_exec().is_match(ruta) {
        return ruta.into();
    }
    let mut r = String::from("\"");
    for c in ruta.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            r.push('\\');
        }
        r.push(c);
    }
    r.push('"');
    r
}

fn partes_exec(exec: &str) -> Vec<String> {
    dividir_comandos(exec).unwrap_or_else(|| exec.split_whitespace().map(str::to_string).collect())
}

/// Cambia el programa de la línea Exec por el instalado y deja sus argumentos (%U, %F…).
fn nuevo_exec(viejo: &str, programa: &str, prefijo: &str) -> String {
    static PALABRA: OnceLock<Regex> = OnceLock::new();
    let palabra = PALABRA.get_or_init(|| Regex::new(r"^[\w@%+=:,./-]+$").expect("regex"));
    let partes = partes_exec(viejo);
    let mut resto: Vec<&String> = partes.iter().skip(1).collect();
    while !resto.is_empty() && partes.first().is_some_and(|p| p == "env") && resto[0].contains('=') {
        resto.remove(0); // «env VAR=1 programa %U»
    }
    if partes.first().is_some_and(|p| p == "env") && !resto.is_empty() {
        resto.remove(0);
    }
    let args: Vec<String> = resto
        .iter()
        .map(|p| {
            if p.starts_with('%') || palabra.is_match(p) {
                (*p).clone()
            } else {
                comillas_exec(p)
            }
        })
        .collect();
    let args = args.join(" ");
    format!(
        "{prefijo}{}{}",
        comillas_exec(programa),
        if args.is_empty() {
            String::new()
        } else {
            format!(" {args}")
        }
    )
    .trim()
    .to_string()
}

fn preparar_desktop(
    grupos: &mut Grupos,
    nombre: &str,
    programa: &str,
    icono: Option<&str>,
    id: &str,
    prefijo: &str,
) {
    if !grupos.iter().any(|(g, _)| g == "Desktop Entry") {
        grupos.insert(0, ("Desktop Entry".into(), Vec::new()));
    }
    poner_entrada(grupos, "Type", Some("Application"));
    if valor_entrada(grupos, "Name").is_empty() {
        poner_entrada(grupos, "Name", Some(nombre));
    }
    let exec = valor_entrada(grupos, "Exec");
    let exec = if exec.is_empty() {
        programa.to_string()
    } else {
        exec
    };
    poner_entrada(grupos, "Exec", Some(&nuevo_exec(&exec, programa, prefijo)));
    poner_entrada(grupos, "TryExec", Some(programa));
    poner_entrada(grupos, "Path", None);
    poner_entrada(grupos, "DBusActivatable", None); // sin servicio D-Bus instalado, el lanzador no la abriría
    let terminal = valor_entrada(grupos, "Terminal");
    poner_entrada(
        grupos,
        "Terminal",
        Some(if terminal.is_empty() { "false" } else { &terminal }),
    );
    if let Some(i) = icono.filter(|i| !i.is_empty()) {
        poner_entrada(grupos, "Icon", Some(i));
    }
    poner_entrada(grupos, MARCA, Some(id));
    // las acciones del lanzador (clic derecho) también llaman al programa nuevo
    let acciones: Vec<String> = grupos
        .iter()
        .filter(|(g, _)| g.starts_with("Desktop Action "))
        .map(|(g, _)| g.clone())
        .collect();
    for g in acciones {
        let ex = valor(grupos, "Exec", &g);
        if !ex.is_empty() {
            poner(grupos, "Exec", Some(&nuevo_exec(&ex, programa, prefijo)), &g);
        }
    }
}

/// La ruta para el .desktop: con su nombre original si está libre (así el ícono de la barra coincide con el
/// de la ventana) o si ya era de esta misma instalación; si no, con prefijo.
fn desktop_libre(ctx: &Contexto, nombre_archivo: &str, id: &str) -> PathBuf {
    let carpeta = ctx.rutas.lanzadores();
    let base = if nombre_archivo.ends_with(".desktop") {
        nombre_archivo.to_string()
    } else {
        format!("{nombre_archivo}.desktop")
    };
    for candidato in [base.clone(), format!("vortex-app-installer-{base}")] {
        let p = carpeta.join(candidato);
        if !p.exists() || es_de(&p, id) {
            return p;
        }
    }
    nombre_libre(&carpeta.join(format!("vortex-app-installer-{base}")))
}

fn es_de(desktop: &Path, id: &str) -> bool {
    std::fs::read(desktop)
        .map(|b| valor_entrada(&leer_desktop(&String::from_utf8_lossy(&b)), MARCA) == id)
        .unwrap_or(false)
}

fn leer_texto(ruta: &Path) -> std::io::Result<String> {
    Ok(String::from_utf8_lossy(&std::fs::read(ruta)?).into_owned())
}

// ------------------------------------------------------------------------------------ íconos

pub fn tamano_png(ruta: &Path) -> u32 {
    let mut cabeza = Vec::new();
    if let Ok(f) = std::fs::File::open(ruta) {
        let _ = f.take(24).read_to_end(&mut cabeza);
    }
    if cabeza.len() < 20 || &cabeza[..8] != b"\x89PNG\r\n\x1a\n" {
        return 0;
    }
    u32::from_be_bytes([cabeza[16], cabeza[17], cabeza[18], cabeza[19]])
}

fn tipo_imagen(ruta: &Path) -> &'static str {
    let mut cabeza = Vec::new();
    let Ok(f) = std::fs::File::open(ruta) else {
        return "";
    };
    let _ = f.take(512).read_to_end(&mut cabeza);
    let contiene = |aguja: &[u8]| cabeza.windows(aguja.len()).any(|w| w == aguja);
    if cabeza.starts_with(b"\x89PNG") {
        "png"
    } else if contiene(b"<svg") || cabeza.trim_ascii_start().starts_with(b"<?xml") {
        "svg"
    } else if cabeza.starts_with(b"/* XPM */") {
        "xpm"
    } else {
        ""
    }
}

/// Copia el ícono al tema hicolor de tu usuario. Devuelve (nombre para `Icon=`, archivos creados).
pub fn instalar_icono(
    ctx: &Contexto,
    origen: &Path,
    nombre: &str,
    id: &str,
) -> std::io::Result<(String, Vec<PathBuf>)> {
    let tipo = tipo_imagen(origen);
    if tipo.is_empty() {
        return Ok((String::new(), vec![]));
    }
    let carpeta = if tipo == "svg" {
        ctx.rutas.iconos().join("scalable/apps")
    } else {
        let lado = if tipo == "png" { tamano_png(origen) } else { 48 };
        let tam = if lado == 0 {
            256
        } else {
            *TAMANOS.iter().min_by_key(|t| t.abs_diff(lado)).unwrap_or(&256)
        };
        ctx.rutas.iconos().join(format!("{tam}x{tam}/apps"))
    };
    std::fs::create_dir_all(&carpeta)?;
    for candidato in [nombre.to_string(), format!("vortex-app-installer-{nombre}")] {
        let destino = carpeta.join(format!("{candidato}.{tipo}"));
        let ds = destino.to_string_lossy().into_owned();
        let propio = registro::cargar(ctx).iter().any(|e| {
            registro::texto_de(e, "id") == id
                && e.get("rutas")
                    .and_then(Value::as_array)
                    .is_some_and(|r| r.iter().any(|x| x.as_str() == Some(&ds)))
        });
        if !existe_o_enlace(&destino) || propio {
            std::fs::copy(origen, &destino)?;
            return Ok((candidato, vec![destino]));
        }
    }
    let destino = nombre_libre(&carpeta.join(format!("vortex-app-installer-{nombre}.{tipo}")));
    std::fs::copy(origen, &destino)?;
    let stem = raiz_de_nombre(&nombre_de(&destino)).to_string();
    Ok((stem, vec![destino]))
}

// ------------------------------------------------------------------------------------ desinstalador

pub fn escribir_desinstalador(
    ctx: &Contexto,
    ruta: &Path,
    nombre: &str,
    borrar: &[PathBuf],
) -> std::io::Result<()> {
    let mut lineas = vec![
        "#!/bin/sh".to_string(),
        format!(
            "# Desinstalador de {nombre}, creado por Vortex App Installer el {}.",
            crate::util::ahora().dia_mes_anio()
        ),
        "# También se puede desinstalar desde la app, en la pestaña «Instalado» (así va a la papelera)."
            .into(),
        "set -u".into(),
    ];
    for p in borrar {
        lineas.push(format!("rm -rf -- {}", comillas_shell(&p.to_string_lossy())));
    }
    lineas.push(format!(
        "command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q {} 2>/dev/null",
        comillas_shell(&ctx.rutas.lanzadores().to_string_lossy())
    ));
    lineas.push(format!(
        "echo {}",
        comillas_shell(&ctx.tr("desinstalado_terminal", &[("nombre", nombre)]))
    ));
    lineas.push(String::new());
    if let Some(p) = ruta.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(ruta, lineas.join("\n"))?;
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755))
}

// ------------------------------------------------------------------------------------ AppImage

fn sacar_de_appimage(ctx: &Contexto, appimage: &Path, carpeta: &Path, patron: &str) {
    let mut c = std::process::Command::new(appimage);
    ctx.preparar(&mut c);
    c.arg("--appimage-extract")
        .arg(patron)
        .current_dir(carpeta)
        .env_remove("APPIMAGE_EXTRACT_AND_RUN");
    let _ = proc::ejecutar(c, None, Some(Duration::from_secs(120)));
}

/// Los archivos de una carpeta que terminan en `sufijo`, ordenados.
fn listar_con_sufijo(carpeta: &Path, sufijo: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(carpeta)
        .map(|l| {
            l.flatten()
                .map(|e| e.path())
                .filter(|p| nombre_de(p).ends_with(sufijo))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// La ruta real (siguiendo enlaces) si queda dentro de `raiz`; `None` si se sale o no existe.
fn dentro_de(ruta: &Path, raiz: &Path) -> Option<PathBuf> {
    let real = std::fs::canonicalize(ruta).ok()?;
    let r = std::fs::canonicalize(raiz).ok()?;
    real.starts_with(&r).then_some(real)
}

/// Saca el .desktop y el ícono del AppImage a `carpeta`. Devuelve (desktop, ícono).
fn metadatos_appimage(ctx: &Contexto, appimage: &Path, carpeta: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let raiz = carpeta.join("squashfs-root");
    sacar_de_appimage(ctx, appimage, carpeta, "*.desktop");
    if !raiz.is_dir() {
        // AppImage tipo 1 (una imagen ISO 9660): lo lee bsdtar sin correr nada
        let _ = std::fs::create_dir_all(&raiz);
        if let Some(bsdtar) = ctx.which("bsdtar") {
            let mut c = std::process::Command::new(bsdtar);
            ctx.preparar(&mut c);
            c.arg("-xf").arg(appimage).arg("-C").arg(&raiz).args([
                "--no-same-owner",
                "*.desktop",
                ".DirIcon",
                "*.png",
                "*.svg",
            ]);
            let _ = proc::ejecutar(c, None, Some(Duration::from_secs(120)));
        }
    }
    let mut desktops = listar_con_sufijo(&raiz, ".desktop");
    if desktops.is_empty() {
        desktops = listar_con_sufijo(&raiz.join("usr/share/applications"), ".desktop");
    }
    let desktop = desktops
        .iter()
        .find(|d| d.is_file() && !d.is_symlink())
        .or_else(|| desktops.iter().find(|d| d.exists()))
        .cloned();
    let icono_nombre = desktop
        .as_ref()
        .and_then(|d| leer_texto(d).ok())
        .map(|t| valor_entrada(&leer_desktop(&t), "Icon"))
        .unwrap_or_default();
    let icono_nombre = basename(&icono_nombre).to_string();
    let mut patrones = vec![".DirIcon".to_string()];
    if !icono_nombre.is_empty() {
        patrones.extend([
            format!("{icono_nombre}.svg"),
            format!("{icono_nombre}.png"),
            icono_nombre.clone(),
            format!("usr/share/icons/hicolor/256x256/apps/{icono_nombre}.png"),
            format!("usr/share/icons/hicolor/scalable/apps/{icono_nombre}.svg"),
        ]);
    }
    for p in &patrones {
        sacar_de_appimage(ctx, appimage, carpeta, p);
    }
    let mut candidatos = Vec::new();
    if !icono_nombre.is_empty() {
        candidatos.extend([
            raiz.join(format!("{icono_nombre}.svg")),
            raiz.join(format!("{icono_nombre}.png")),
            raiz.join(&icono_nombre),
            raiz.join("usr/share/icons/hicolor/scalable/apps")
                .join(format!("{icono_nombre}.svg")),
            raiz.join("usr/share/icons/hicolor/256x256/apps")
                .join(format!("{icono_nombre}.png")),
        ]);
    }
    candidatos.push(raiz.join(".DirIcon"));
    let icono = candidatos.iter().find_map(|c| {
        let real = dentro_de(c, &raiz)?;
        (real.is_file() && !tipo_imagen(&real).is_empty()).then_some(real)
    });
    (desktop.and_then(|d| dentro_de(&d, &raiz)), icono)
}

fn copiar(origen: &Path, destino: &Path, control: &Control) -> Resultado<()> {
    let total = std::fs::metadata(origen)?.len().max(1);
    let mut f = std::fs::File::open(origen)?;
    let mut fuera = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destino)?;
    let mut buf = vec![0u8; 4 << 20];
    let mut hecho = 0u64;
    loop {
        if control.cancelado() {
            drop(fuera);
            let _ = std::fs::remove_file(destino);
            return Err(Error::Cancelado);
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        fuera.write_all(&buf[..n])?;
        hecho += n as u64;
        control.avanzar(0.85 * hecho as f64 / total as f64);
    }
    Ok(())
}

/// Id estable para un AppImage aunque cambie de versión: «BedrockOnLinux-2.2.7-x86_64» → «appimage-bedrockonlinux».
pub fn id_appimage(ruta: &Path) -> String {
    static EXT: OnceLock<Regex> = OnceLock::new();
    static VERSION: OnceLock<Regex> = OnceLock::new();
    static ARQ: OnceLock<Regex> = OnceLock::new();
    let ext = EXT.get_or_init(|| Regex::new(r"(?i)\.appimage$").expect("regex"));
    let version = VERSION.get_or_init(|| Regex::new(r"[-_ ]v?\d").expect("regex"));
    let arq =
        ARQ.get_or_init(|| Regex::new(r"(?i)[-_](x86_64|amd64|aarch64|arm64|i386|linux)$").expect("regex"));
    let nombre = nombre_de(ruta);
    let base = ext.replace(&nombre, "").into_owned();
    let base = version.split(&base).next().unwrap_or("").to_string();
    let base = arq.replace(&base, "").into_owned();
    format!("appimage-{}", registro::slug(&base))
}

fn primer_icono(creados: &[PathBuf]) -> String {
    creados
        .iter()
        .find(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("png" | "svg")))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn rutas_json(creados: &[PathBuf]) -> Value {
    json!(creados.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>())
}

pub fn instalar_appimage(
    ctx: &Contexto,
    ruta: &Path,
    reemplazar: bool,
    control: &Control,
) -> Resultado<registro::Entrada> {
    let id = id_appimage(ruta);
    let previo = registro::buscar(ctx, &id);
    let mut destino = ctx.rutas.appimages().join(nombre_de(ruta));
    if let Some(p) = &previo
        && !reemplazar
    {
        let abrir = registro::texto_de(p, "abrir");
        return Err(Error::Conflicto {
            ruta: if abrir.is_empty() {
                destino.clone()
            } else {
                PathBuf::from(abrir)
            },
            sugerida: destino,
        });
    }
    let ds = destino.to_string_lossy().into_owned();
    let es_nuestro = previo.as_ref().is_some_and(|p| {
        p.get("rutas")
            .and_then(Value::as_array)
            .is_some_and(|r| r.iter().any(|x| x.as_str() == Some(&ds)))
    });
    if existe_o_enlace(&destino) && !es_nuestro {
        if !reemplazar {
            return Err(Error::Conflicto {
                sugerida: nombre_libre(&destino),
                ruta: destino,
            });
        }
        destino = nombre_libre(&destino); // no es nuestro: no se toca, va al lado
    }
    if previo.is_some() {
        registro::deshacer(ctx, &id)?; // la versión anterior, a la papelera
    }
    if let Some(p) = destino.parent() {
        std::fs::create_dir_all(p)?;
    }
    copiar(ruta, &destino, control)?;
    std::fs::set_permissions(&destino, std::fs::Permissions::from_mode(0o755))?;
    let mut creados = vec![destino.clone()];
    let r = (|| -> Resultado<(registro::Entrada, PathBuf)> {
        let tmp = tempfile::Builder::new().prefix("vortex-appimage-").tempdir()?;
        let (desktop_origen, icono_origen) = metadatos_appimage(ctx, &destino, tmp.path());
        control.avanzar(0.95);
        let mut grupos = match &desktop_origen {
            Some(d) => leer_desktop(&leer_texto(d)?),
            None => Vec::new(),
        };
        static EXT: OnceLock<Regex> = OnceLock::new();
        let ext = EXT.get_or_init(|| Regex::new(r"(?i)\.appimage$").expect("regex"));
        let nombre = {
            let n = valor_entrada(&grupos, "Name");
            if n.is_empty() {
                ext.replace(&nombre_de(ruta), "").into_owned()
            } else {
                n
            }
        };
        let base_icono = registro::slug(
            &desktop_origen
                .as_ref()
                .map_or_else(|| nombre.clone(), |d| raiz_de_nombre(&nombre_de(d)).to_string()),
        );
        let mut icono = String::new();
        if let Some(io) = &icono_origen {
            let (i, archivos) = instalar_icono(ctx, io, &base_icono, &id)?;
            icono = i;
            creados.extend(archivos);
        }
        if grupos.is_empty() {
            grupos = vec![(
                "Desktop Entry".into(),
                vec![
                    ("Name".into(), nombre.clone()),
                    ("Categories".into(), "Utility;".into()),
                ],
            )];
        }
        let prefijo = if ctx.hay_fuse2() {
            ""
        } else {
            "env APPIMAGE_EXTRACT_AND_RUN=1 "
        };
        preparar_desktop(
            &mut grupos,
            &nombre,
            &destino.to_string_lossy(),
            Some(if icono.is_empty() {
                "application-x-executable"
            } else {
                &icono
            }),
            &id,
            prefijo,
        );
        let nombre_desktop = desktop_origen
            .as_ref()
            .map_or_else(|| format!("{base_icono}.desktop"), |d| nombre_de(d));
        let desktop = desktop_libre(ctx, &nombre_desktop, &id);
        if let Some(p) = desktop.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&desktop, escribir_desktop(&grupos))?;
        creados.push(desktop);
        let script = ctx.rutas.desinstaladores().join(format!("{id}.sh"));
        creados.push(script.clone());
        escribir_desinstalador(ctx, &script, &nombre, &creados)?;
        let entrada = Map::from_iter([
            ("id".to_string(), json!(id)),
            ("tipo".into(), json!("appimage")),
            ("nombre".into(), json!(nombre)),
            ("rutas".into(), rutas_json(&creados)),
            ("icono".into(), json!(primer_icono(&creados))),
            ("archivo".into(), json!(nombre_de(ruta))),
            ("abrir".into(), json!(destino.to_string_lossy())),
            ("desinstalador".into(), json!(script.to_string_lossy())),
        ]);
        Ok((entrada, script))
    })();
    let (entrada, _) = match r {
        Ok(x) => x,
        Err(e) => {
            for p in &creados {
                // a medias no se queda nada
                if p.is_file() || p.is_symlink() {
                    let _ = std::fs::remove_file(p);
                }
            }
            return Err(e);
        }
    };
    registro::refrescar_lanzador(ctx);
    let entrada = registro::agregar(ctx, entrada)?;
    control.avanzar(1.0);
    Ok(entrada)
}

/// «Solo abrir»: le da permiso de ejecución y lo corre, sin instalar nada.
pub fn abrir_appimage(ctx: &Contexto, ruta: &Path) -> Resultado<()> {
    let m = std::fs::metadata(ruta)?;
    std::fs::set_permissions(
        ruta,
        std::fs::Permissions::from_mode(m.permissions().mode() | 0o100),
    )?;
    ctx.lanzar(
        &[ruta.to_string_lossy().into_owned()],
        &ctx.entorno_appimage(),
        ruta.parent(),
    )
}

// ------------------------------------------------------------------------------------ programas .tar.gz

fn es_programa(p: &Path) -> bool {
    let ok = p.is_file() && !p.is_symlink() && crate::util::es_ejecutable(p);
    if !ok {
        return false;
    }
    let mut cabeza = Vec::new();
    let Ok(f) = std::fs::File::open(p) else {
        return false;
    };
    let _ = f.take(4).read_to_end(&mut cabeza);
    let nombre = nombre_de(p).to_lowercase();
    if nombre.contains(".so") || nombre.ends_with(".desktop") {
        return false;
    }
    cabeza == b"\x7fELF" || cabeza.starts_with(b"#!")
}

const INSTALADORES: [&str; 6] = [
    "instalar.sh",
    "install.sh",
    "desinstalar.sh",
    "uninstall.sh",
    "installer.sh",
    "setup.sh",
];

fn caminar(raiz: &Path, carpeta: &Path, candidatos: &mut Vec<PathBuf>) {
    let hondo = carpeta.strip_prefix(raiz).map_or(0, |r| r.components().count());
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut hijos: Vec<(PathBuf, std::fs::Metadata)> = lista
        .flatten()
        .filter_map(|e| Some((e.path(), std::fs::symlink_metadata(e.path()).ok()?)))
        .collect();
    hijos.sort_by(|a, b| a.0.cmp(&b.0));
    for (p, m) in &hijos {
        if !m.is_dir() && !INSTALADORES.contains(&nombre_de(p).to_lowercase().as_str()) && es_programa(p) {
            candidatos.push(p.clone());
        }
    }
    if hondo >= 3 {
        return;
    }
    for (p, m) in &hijos {
        if m.is_dir() && !["_internal", "lib", "lib64", "share", "resources"].contains(&nombre_de(p).as_str())
        {
            caminar(raiz, p, candidatos);
        }
    }
}

fn solo_letras_y_numeros(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        .collect()
}

pub fn buscar_ejecutable(raiz: &Path, grupos: &Grupos, nombre_desktop: &str) -> Option<PathBuf> {
    let mut candidatos = Vec::new();
    caminar(raiz, raiz, &mut candidatos);
    if candidatos.is_empty() {
        return None;
    }
    let mut partes = partes_exec(&valor_entrada(grupos, "Exec"));
    while partes
        .first()
        .is_some_and(|p| p == "env" || (p.contains('=') && !p.starts_with('/')))
    {
        partes.remove(0);
    }
    let token = partes.first().cloned().unwrap_or_default();
    if !token.is_empty() && !token.starts_with('/') && !token.contains('@') {
        let directo = raiz.join(&token);
        if candidatos.contains(&directo) {
            return Some(directo);
        }
    }
    let pistas: Vec<String> = [
        basename(&token).to_string(),
        basename(&valor_entrada(grupos, "TryExec")).to_string(),
        raiz_de_nombre(nombre_desktop).to_string(),
        valor_entrada(grupos, "StartupWMClass"),
    ]
    .into_iter()
    .filter(|p| !p.is_empty() && !p.contains('@'))
    .map(|p| p.to_lowercase())
    .collect();
    let por_hondura = |c: &PathBuf| c.components().count();
    for pista in &pistas {
        let mut ordenados: Vec<&PathBuf> = candidatos.iter().collect();
        ordenados.sort_by_key(|c| por_hondura(c));
        if let Some(c) = ordenados
            .into_iter()
            .find(|c| nombre_de(c).to_lowercase() == *pista)
        {
            return Some(c.clone());
        }
    }
    let nombre = solo_letras_y_numeros(&valor_entrada(grupos, "Name"));
    for c in &candidatos {
        if !nombre.is_empty() && solo_letras_y_numeros(&nombre_de(c)) == nombre {
            return Some(c.clone());
        }
    }
    let mut poco_hondos = candidatos.clone();
    poco_hondos.sort_by_key(por_hondura);
    if poco_hondos.len() == 1 || por_hondura(&poco_hondos[0]) < por_hondura(&poco_hondos[1]) {
        return Some(poco_hondos[0].clone());
    }
    None
}

fn buscar_icono(raiz: &Path, icono: &str) -> Option<PathBuf> {
    if icono.is_empty() {
        return None;
    }
    if icono.contains('/') {
        if icono.starts_with('/') {
            return None;
        }
        let p = dentro_de(&raiz.join(icono.trim_start_matches('/')), raiz)?;
        return (!tipo_imagen(&p).is_empty()).then_some(p);
    }
    let mut mejores: Vec<(u32, PathBuf)> = Vec::new();
    fn recorrer(carpeta: &Path, raiz: &Path, icono: &str, mejores: &mut Vec<(u32, PathBuf)>) {
        let Ok(lista) = std::fs::read_dir(carpeta) else {
            return;
        };
        let mut hijos: Vec<PathBuf> = lista.flatten().map(|e| e.path()).collect();
        hijos.sort();
        for p in hijos {
            let Ok(m) = std::fs::symlink_metadata(&p) else {
                continue;
            };
            if m.is_dir() {
                recorrer(&p, raiz, icono, mejores);
                continue;
            }
            let n = nombre_de(&p);
            let ext = crate::util::sufijo(&n).to_lowercase();
            if raiz_de_nombre(&n) == icono && [".svg", ".png", ".xpm"].contains(&ext.as_str()) {
                if dentro_de(&p, raiz).is_none() {
                    continue;
                }
                let puntos = match ext.as_str() {
                    ".svg" => 100_000,
                    ".png" => tamano_png(&p),
                    _ => 1,
                };
                mejores.push((puntos, p));
            }
        }
    }
    recorrer(raiz, raiz, icono, &mut mejores);
    let mut mejor: Option<(u32, PathBuf)> = None;
    for (puntos, p) in mejores {
        if mejor.as_ref().is_none_or(|(m, _)| puntos > *m) {
            mejor = Some((puntos, p));
        }
    }
    mejor.map(|(_, p)| p)
}

/// (nombre de la carpeta en ~/.local/opt, nombre del .desktop que trae).
pub fn plan_programa(entradas: &[Entrada], archivo: &Path) -> (String, String) {
    let mut desktops: Vec<&Entrada> = entradas
        .iter()
        .filter(|e| e.tipo == Tipo::Archivo && e.nombre.ends_with(".desktop") && e.profundidad() <= 3)
        .collect();
    desktops.sort_by_key(|e| e.profundidad());
    let nombre_desktop = desktops
        .first()
        .map(|e| partes(&e.nombre).last().copied().unwrap_or("").to_string())
        .unwrap_or_default();
    let carpeta = registro::slug(&if nombre_desktop.is_empty() {
        comprimidos::nombre_sin_extension(&nombre_de(archivo))
    } else {
        raiz_de_nombre(&nombre_desktop).to_string()
    });
    (carpeta, nombre_desktop)
}

fn buscar_en_arbol(carpeta: &Path, nombre: &str, salida: &mut Vec<PathBuf>) {
    let Ok(lista) = std::fs::read_dir(carpeta) else {
        return;
    };
    let mut hijos: Vec<PathBuf> = lista.flatten().map(|e| e.path()).collect();
    hijos.sort();
    for p in hijos {
        if nombre_de(&p) == nombre {
            salida.push(p.clone());
        }
        if std::fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
            buscar_en_arbol(&p, nombre, salida);
        }
    }
}

pub fn instalar_programa(
    ctx: &Contexto,
    archivo: &Path,
    entradas: &[Entrada],
    reemplazar: bool,
    contrasena: Option<&str>,
    control: &Control,
) -> Resultado<registro::Entrada> {
    let (carpeta, nombre_desktop) = plan_programa(entradas, archivo);
    let id = format!("programa-{carpeta}");
    let mut destino = ctx.rutas.opt().join(&carpeta);
    let previo = registro::buscar(ctx, &id);
    if existe_o_enlace(&destino) || previo.is_some() {
        if !reemplazar {
            return Err(Error::Conflicto {
                ruta: destino.clone(),
                sugerida: destino,
            });
        }
        if previo.is_some() {
            registro::deshacer(ctx, &id)?;
        }
        if existe_o_enlace(&destino) {
            destino = nombre_libre(&destino); // una carpeta que no es nuestra: no se toca, va al lado
        }
    }
    std::fs::create_dir_all(ctx.rutas.opt())?;
    comprimidos::extraer(
        ctx,
        archivo,
        &destino,
        Some(entradas),
        contrasena,
        &control.escalado(0.9),
        true,
    )?;
    let mut creados: Vec<PathBuf> = vec![destino.clone()];
    let r = (|| -> Resultado<registro::Entrada> {
        let desktop_origen = if nombre_desktop.is_empty() {
            None
        } else {
            let mut v = Vec::new();
            buscar_en_arbol(&destino, &nombre_desktop, &mut v);
            v.sort_by_key(|p| p.components().count());
            v.into_iter().find(|p| dentro_de(p, &destino).is_some())
        };
        let grupos = match &desktop_origen {
            Some(d) => leer_desktop(&leer_texto(d)?),
            None => Vec::new(),
        };
        let programa = buscar_ejecutable(&destino, &grupos, &nombre_desktop)
            .ok_or_else(|| Error::Mensaje(ctx.tr("programa_sin_ejecutable", &[])))?;
        let nombre = {
            let n = valor_entrada(&grupos, "Name");
            if n.is_empty() { carpeta.clone() } else { n }
        };
        let icono_decl = valor_entrada(&grupos, "Icon");
        let icono_origen = buscar_icono(&destino, &icono_decl);
        let mut icono = if !icono_decl.is_empty() && !icono_decl.contains('/') {
            icono_decl.clone()
        } else {
            String::new()
        };
        if let Some(io) = &icono_origen {
            let base = {
                let s = raiz_de_nombre(&nombre_desktop);
                registro::slug(if s.is_empty() { &carpeta } else { s })
            };
            let (i, archivos) = instalar_icono(ctx, io, &base, &id)?;
            icono = i;
            creados.extend(archivos);
        }
        let mut grupos = grupos;
        if grupos.is_empty() {
            grupos = vec![(
                "Desktop Entry".into(),
                vec![
                    ("Name".into(), nombre.clone()),
                    ("Categories".into(), "Utility;".into()),
                ],
            )];
        }
        preparar_desktop(
            &mut grupos,
            &nombre,
            &programa.to_string_lossy(),
            Some(if icono.is_empty() {
                "application-x-executable"
            } else {
                &icono
            }),
            &id,
            "",
        );
        let nombre_archivo = if nombre_desktop.is_empty() {
            format!("{carpeta}.desktop")
        } else {
            nombre_desktop.clone()
        };
        let desktop = desktop_libre(ctx, &nombre_archivo, &id);
        if let Some(p) = desktop.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(&desktop, escribir_desktop(&grupos))?;
        creados.push(desktop);
        let script = destino.join("desinstalar.sh");
        if script.exists() {
            // el que trae el comprimido es para su propio instalador: se guarda aparte
            let guardado = destino.join("desinstalar.sh.original");
            let a = if existe_o_enlace(&guardado) {
                nombre_libre(&guardado)
            } else {
                guardado
            };
            std::fs::rename(&script, a)?;
        }
        escribir_desinstalador(ctx, &script, &nombre, &creados)?;
        Ok(Map::from_iter([
            ("id".to_string(), json!(id)),
            ("tipo".into(), json!("programa")),
            ("nombre".into(), json!(nombre)),
            ("rutas".into(), rutas_json(&creados)),
            ("icono".into(), json!(primer_icono(&creados))),
            ("archivo".into(), json!(nombre_de(archivo))),
            ("abrir".into(), json!(programa.to_string_lossy())),
            ("desinstalador".into(), json!(script.to_string_lossy())),
        ]))
    })();
    let entrada = match r {
        Ok(e) => e,
        Err(e) => {
            for p in creados.iter().rev() {
                let _ = papelera::mandar(ctx, p);
            }
            return Err(e);
        }
    };
    registro::refrescar_lanzador(ctx);
    let entrada = registro::agregar(ctx, entrada)?;
    control.avanzar(1.0);
    Ok(entrada)
}

// ------------------------------------------------------------------------------------ correr cosas

pub fn abrir_carpeta(ctx: &Contexto, ruta: &Path) -> Resultado<()> {
    let carpeta = if ruta.is_dir() {
        ruta.to_path_buf()
    } else {
        ruta.parent().unwrap_or(Path::new("/")).to_path_buf()
    };
    let c = carpeta.to_string_lossy().into_owned();
    if ctx.which("xdg-open").is_some() {
        return ctx.lanzar(&["xdg-open".into(), c], &[], None);
    }
    if ctx.which("gio").is_some() {
        return ctx.lanzar(&["gio".into(), "open".into(), c], &[], None);
    }
    Err(Error::Mensaje(ctx.tr("no_se_pudo_abrir", &[])))
}

const ADMIN: [&str; 5] = ["sudo", "pkexec", "doas", "su", "run0"];

/// Lo que se muestra de un instalador .sh/.run/.bin antes de correrlo: sus primeras líneas si es texto.
pub fn vista_script(ctx: &Contexto, ruta: &Path, cabeza: &[u8]) -> Map<String, Value> {
    let elf = cabeza.starts_with(b"\x7fELF");
    let mut lineas: Vec<String> = Vec::new();
    if !elf {
        let tope = &cabeza[..cabeza.len().min(16384)];
        for linea in tope.split(|b| *b == b'\n').take(40) {
            let raros = linea.iter().filter(|c| **c < 9 || (13 < **c && **c < 32)).count();
            if linea.contains(&0) || raros > 2 {
                break; // empieza lo binario (un .run trae el programa pegado después del texto)
            }
            lineas.push(String::from_utf8_lossy(linea).trim_end_matches('\r').to_string());
        }
    }
    static ADMIN_RE: OnceLock<Regex> = OnceLock::new();
    let admin = ADMIN_RE
        .get_or_init(|| Regex::new(r"\b(sudo|pkexec|doas)\b|EUID|id -u|as root|como root").expect("regex"));
    let texto = String::from_utf8_lossy(&cabeza[..cabeza.len().min(65536)]).into_owned();
    let comando = comando_script(ctx, ruta, Some(cabeza), false).unwrap_or_default();
    Map::from_iter([
        ("vista".to_string(), json!(lineas.join("\n"))),
        ("binario".into(), json!(elf)),
        ("pide_admin".into(), json!(admin.is_match(&texto))),
        (
            "comando_admin".into(),
            json!(format!(
                "sudo {}",
                comando
                    .iter()
                    .map(|p| comillas_shell(p))
                    .collect::<Vec<_>>()
                    .join(" ")
            )),
        ),
    ])
}

/// Cómo se corre el instalador: con el intérprete de su primera línea (#!), con `sh`, o directo si es binario.
pub fn comando_script(
    ctx: &Contexto,
    ruta: &Path,
    cabeza: Option<&[u8]>,
    sin_admin: bool,
) -> Resultado<Vec<String>> {
    let leida;
    let cabeza = match cabeza {
        Some(c) => c,
        None => {
            let mut v = Vec::new();
            if let Ok(f) = std::fs::File::open(ruta) {
                let _ = f.take(512).read_to_end(&mut v);
            }
            leida = v;
            &leida
        }
    };
    let r = ruta.to_string_lossy().into_owned();
    if cabeza.starts_with(b"\x7fELF") {
        return Ok(vec![r]);
    }
    if let Some(resto) = cabeza.strip_prefix(b"#!") {
        let primera = String::from_utf8_lossy(resto.split(|b| *b == b'\n').next().unwrap_or(&[]))
            .trim()
            .to_string();
        let ps = partes_exec(&primera);
        if !ps.is_empty() {
            if sin_admin && ps.iter().any(|p| ADMIN.contains(&basename(p))) {
                return Err(Error::Mensaje(ctx.tr("script_con_sudo", &[])));
            }
            let mut v = ps;
            v.push(r);
            return Ok(v);
        }
    }
    Ok(vec!["sh".into(), r])
}

/// Abre `args` en una terminal que se queda abierta al terminar (para leer lo que pasó).
pub fn terminal(ctx: &Contexto, args: &[String], titulo: &str, carpeta: &Path) -> Resultado<()> {
    let c = carpeta.to_string_lossy().into_owned();
    let con = |prefijo: Vec<&str>, sep: Option<&str>| -> Vec<String> {
        let mut v: Vec<String> = prefijo.into_iter().map(str::to_string).collect();
        if let Some(s) = sep {
            v.push(s.to_string());
        }
        v.extend(args.iter().cloned());
        v
    };
    let opciones: [(&str, Vec<String>); 4] = [
        (
            "kitty",
            con(
                vec!["kitty", "--hold", "--title", titulo, "--directory", &c],
                Some("--"),
            ),
        ),
        (
            "foot",
            con(
                vec!["foot", "--hold", "--title", titulo, "--working-directory", &c],
                None,
            ),
        ),
        (
            "alacritty",
            con(
                vec![
                    "alacritty",
                    "--hold",
                    "--title",
                    titulo,
                    "--working-directory",
                    &c,
                ],
                Some("-e"),
            ),
        ),
        (
            "konsole",
            con(vec!["konsole", "--hold", "--workdir", &c], Some("-e")),
        ),
    ];
    for (exe, cmd) in opciones {
        if ctx.which(exe).is_some() {
            return ctx.lanzar(&cmd, &[], Some(carpeta));
        }
    }
    Err(Error::Mensaje(ctx.tr("falta_terminal", &[])))
}

pub fn correr_script(ctx: &Contexto, ruta: &Path) -> Resultado<()> {
    let args = comando_script(ctx, ruta, None, true)?;
    if args == [ruta.to_string_lossy().into_owned()] {
        // un binario: necesita permiso de ejecución
        let m = std::fs::metadata(ruta)?;
        std::fs::set_permissions(
            ruta,
            std::fs::Permissions::from_mode(m.permissions().mode() | 0o100),
        )?;
    }
    terminal(
        ctx,
        &args,
        &nombre_de(ruta),
        ruta.parent().unwrap_or(Path::new("/")),
    )
}

/// Con qué se puede abrir un programa de Windows en este equipo: Vortexine's Runner (si lo instalaste desde
/// la Shop, va primero: le da su propio espacio y lo deja en tu biblioteca), Wine, umu (Proton) o el Proton de
/// Steam.
pub fn opciones_windows(ctx: &Contexto) -> Vec<Value> {
    let mut salida = Vec::new();
    if runner(ctx).is_some() {
        salida.push(json!({"id": "runner", "nombre": "Vortexine's Runner"}));
    }
    if ctx.which("wine").is_some() {
        salida.push(json!({"id": "wine", "nombre": "Wine"}));
    }
    if ctx.which("umu-run").is_some() {
        salida.push(json!({"id": "umu", "nombre": "Proton (umu)"}));
    }
    if let Some(p) = proton(ctx) {
        let nombre = p.parent().map(nombre_de).unwrap_or_default();
        salida.push(json!({"id": "proton", "nombre": nombre, "ruta": p.to_string_lossy()}));
    }
    salida
}

/// El lanzador de Vortexine's Runner, donde lo deja «vortex runner instalar».
fn runner(ctx: &Contexto) -> Option<PathBuf> {
    let r = ctx.rutas.datos_xdg().join("vortex-runner/vortex-runner");
    (r.is_file() && crate::util::es_ejecutable(&r)).then_some(r)
}

fn proton(ctx: &Contexto) -> Option<PathBuf> {
    let casa = &ctx.rutas.casa;
    let raices = [
        casa.join(".local/share/Steam"),
        casa.join(".steam/steam"),
        casa.join(".var/app/com.valvesoftware.Steam/.local/share/Steam"),
    ];
    let mut encontrados: Vec<PathBuf> = Vec::new();
    let hijos = |d: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(d)
            .map(|l| l.flatten().map(|e| e.path()).collect())
            .unwrap_or_default()
    };
    for r in &raices {
        for h in hijos(&r.join("steamapps/common")) {
            if nombre_de(&h).starts_with("Proton") && h.join("proton").is_file() {
                encontrados.push(h.join("proton"));
            }
        }
        for h in hijos(&r.join("compatibilitytools.d")) {
            if h.join("proton").is_file() {
                encontrados.push(h.join("proton"));
            }
        }
    }
    encontrados.sort_by_key(|p| p.parent().map(nombre_de).unwrap_or_default());
    encontrados.pop()
}

pub fn abrir_windows(ctx: &Contexto, ruta: &Path, opcion: &str) -> Resultado<()> {
    let msi = nombre_de(ruta).to_lowercase().ends_with(".msi");
    let r = ruta.to_string_lossy().into_owned();
    let mut entorno: Vec<(String, String)> = Vec::new();
    let args: Vec<String> = match opcion {
        "runner" => {
            let Some(runner) = runner(ctx) else {
                return Err(Error::Mensaje(ctx.tr("falta_wine", &[])));
            };
            // Runner pregunta si lo instala o lo agrega a tu biblioteca
            vec![runner.to_string_lossy().into_owned(), r]
        }
        "wine" => {
            if msi {
                vec!["wine".into(), "msiexec".into(), "/i".into(), r]
            } else {
                vec!["wine".into(), r]
            }
        }
        "umu" => {
            if std::env::var_os("WINEPREFIX").is_none() {
                entorno.push((
                    "WINEPREFIX".into(),
                    ctx.rutas.propios().join("windows").to_string_lossy().into_owned(),
                ));
            }
            if msi {
                vec!["umu-run".into(), "msiexec".into(), "/i".into(), r]
            } else {
                vec!["umu-run".into(), r]
            }
        }
        "proton" => {
            let Some(proton) = proton(ctx) else {
                return Err(Error::Mensaje(ctx.tr("falta_wine", &[])));
            };
            let prefijo = ctx.rutas.propios().join("proton");
            std::fs::create_dir_all(&prefijo)?;
            let abuelo = proton.parent().and_then(Path::parent);
            let steam = if abuelo.map(nombre_de).as_deref() == Some("common") {
                abuelo.and_then(Path::parent).and_then(Path::parent)
            } else {
                abuelo.and_then(Path::parent)
            };
            entorno.push((
                "STEAM_COMPAT_DATA_PATH".into(),
                prefijo.to_string_lossy().into_owned(),
            ));
            entorno.push((
                "STEAM_COMPAT_CLIENT_INSTALL_PATH".into(),
                steam
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ));
            let mut v = vec![proton.to_string_lossy().into_owned(), "run".into()];
            if msi {
                v.extend(["msiexec".into(), "/i".into()]);
            }
            v.push(r);
            v
        }
        _ => return Err(Error::Mensaje(ctx.tr("falta_wine", &[]))),
    };
    ctx.lanzar(&args, &entorno, ruta.parent())
}

pub fn abrir_java(ctx: &Contexto, ruta: &Path) -> Resultado<()> {
    if ctx.which("java").is_none() {
        return Err(Error::Mensaje(ctx.tr("falta_java", &[])));
    }
    ctx.lanzar(
        &["java".into(), "-jar".into(), ruta.to_string_lossy().into_owned()],
        &[],
        ruta.parent(),
    )
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn exec_se_reemplaza() {
        assert_eq!(nuevo_exec("\"@EXEC@\" %F", "/o/p", ""), "/o/p %F");
        assert_eq!(nuevo_exec("AppRun %U", "/a b/c", ""), "\"/a b/c\" %U");
        assert_eq!(
            nuevo_exec("env A=1 prog --x \"y z\" %u", "/p", ""),
            "/p --x \"y z\" %u"
        );
        assert_eq!(nuevo_exec("", "/p", "env X=1 "), "env X=1 /p");
        assert_eq!(comillas_exec("a\"b$c"), "\"a\\\"b\\$c\"");
    }

    #[test]
    fn ids_de_appimage() {
        assert_eq!(
            id_appimage(Path::new("/x/Falso-1.0-x86_64.AppImage")),
            "appimage-falso"
        );
        assert_eq!(
            id_appimage(Path::new("BedrockOnLinux-2.2.7-x86_64.AppImage")),
            "appimage-bedrockonlinux"
        );
        assert_eq!(id_appimage(Path::new("Mi App_v2.appimage")), "appimage-mi-app");
        // Igual que Python: «_6» de «x86_64» corta el nombre (cosa rara que se conserva).
        assert_eq!(
            id_appimage(Path::new("tool-x86_64.AppImage")),
            "appimage-tool-x86"
        );
    }

    #[test]
    fn desktop_ida_y_vuelta() {
        let t = "[Desktop Entry]\nName=A\nExec=a %U\n\n[Desktop Action x]\nName=X\nExec=a --x\n";
        let mut g = leer_desktop(t);
        assert_eq!(valor(&g, "Exec", "Desktop Action x"), "a --x");
        poner(&mut g, "Exec", None, "Desktop Action x");
        poner(&mut g, "Icon", Some("i"), "Desktop Entry");
        assert_eq!(
            escribir_desktop(&g),
            "[Desktop Entry]\nName=A\nExec=a %U\nIcon=i\n\n[Desktop Action x]\nName=X\n"
        );
    }
}
