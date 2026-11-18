//! El cerebro de la ventana, sin Qt: qué es el archivo, qué se le ofrece a la persona y hacerlo (`motor.py`).
//!
//! ```text
//! let info = motor::analizar(&ctx, ruta, None);          // detectar + lo que tarda (Flathub, instancias…)
//! let vista = motor::para_mostrar(&ctx, &info);          // JSON listo para la interfaz (textos ya traducidos)
//! let r = motor::ejecutar(&ctx, &info, "descomprimir", &opciones, &control)?;
//! ```
//!
//! [`ejecutar`] da [`Error::Conflicto`] (el destino ya existe: la interfaz pregunta y vuelve a llamar con
//! `{"nombre": …}` o `{"reemplazar": true}`) y [`Error::Contrasena`] (vuelve con `{"contrasena": …}`);
//! cualquier otro problema es un [`Error::Problema`] con el mensaje para la persona.

use crate::comprimidos::{self, Entrada, Tipo};
use crate::detectar::{self, Info};
use crate::util::nombre_de;
use crate::{Contexto, Control, Error, Resultado, flathub, flatpak, instalar, mods, registro};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// Las que se muestran en la lista de contenido.
pub const MAX_ENTRADAS: usize = 400;

/// Las opciones de una acción (lo que manda la interfaz: `nombre`, `reemplazar`, `carpeta`, `contrasena`…).
pub type Opciones = Map<String, Value>;

fn problema(mensaje: String) -> Error {
    Error::Problema {
        mensaje,
        comando: String::new(),
    }
}

pub fn tamano_legible(ctx: &Contexto, n: u64) -> String {
    if n < 1024 {
        return ctx.tr("bytes", &[("n", &n.to_string())]);
    }
    let mut x = n as f64;
    for unidad in ["KB", "MB", "GB", "TB"] {
        x /= 1024.0;
        if x < 1024.0 || unidad == "TB" {
            let mut texto = if x >= 100.0 { format!("{x:.0}") } else { format!("{x:.1}") };
            if texto.ends_with(".0") {
                texto.truncate(texto.len() - 2);
            }
            if ctx.idioma == crate::textos::Idioma::Es {
                texto = texto.replace('.', ",");
            }
            return format!("{texto} {unidad}");
        }
    }
    n.to_string()
}

// ------------------------------------------------------------------------------------ analizar

/// [`detectar::detectar`] más lo que necesita red o comandos: el programa en Flathub, las instancias de
/// Minecraft, con qué abrir un .exe. Puede tardar unos segundos (se corre fuera de la ventana).
pub fn analizar(ctx: &Contexto, ruta: &Path, contrasena: Option<&str>) -> Info {
    let mut info = detectar::detectar(ctx, ruta, contrasena);
    if matches!(info.tipo.as_str(), "deb" | "rpm" | "windows") {
        let nombre = info.extra_texto("nombre_programa");
        if nombre.is_empty() {
            info.extra.insert("flathub".into(), Value::Null);
        } else {
            match flathub::buscar(ctx, &flathub::para_buscar(&nombre), 8) {
                Ok(r) => {
                    let m = flathub::mejor(&nombre, &r).map(flathub::App::a_json);
                    info.extra.insert("flathub".into(), m.unwrap_or(Value::Null));
                }
                Err(_) => {
                    info.extra.insert("flathub".into(), Value::Null);
                    info.extra.insert("flathub_sin_red".into(), json!(true));
                }
            }
        }
    }
    if info.tipo == "windows" {
        info.extra.insert("opciones_windows".into(), Value::Array(instalar::opciones_windows(ctx)));
    }
    if info.tipo == "mod" {
        info.extra
            .insert("instancias".into(), mods::instancias(ctx).map_or(Value::Null, Value::Array));
    }
    if info.tipo == "flatpakref" && info.extra_texto("nombre_programa").is_empty() {
        let stem = crate::util::raiz_de_nombre(&info.nombre).to_string();
        info.extra.insert("nombre_programa".into(), json!(stem));
    }
    info
}

// ------------------------------------------------------------------------------------ qué mostrar

/// Tipo de archivo → clave del texto que lo nombra (las pruebas revisan que existan).
pub const TIPOS: [(&str, &str); 13] = [
    ("comprimido", "tipo_comprimido"),
    ("programa", "tipo_programa"),
    ("appimage", "tipo_appimage"),
    ("deb", "tipo_deb"),
    ("rpm", "tipo_rpm"),
    ("flatpakref", "tipo_flatpak"),
    ("flatpak", "tipo_flatpak"),
    ("windows", "tipo_windows"),
    ("mod", "tipo_mod"),
    ("jar", "tipo_jar"),
    ("desconocido", "tipo_desconocido"),
    ("carpeta", "tipo_carpeta"),
    ("falta", "tipo_falta"),
];

fn accion(id: &str, texto: &str, tipo: &str, icono: &str, confirmar: Option<Value>, carpeta: bool, datos: Option<Value>) -> Value {
    json!({"id": id, "texto": texto, "tipo": tipo, "icono": icono, "confirmar": confirmar.unwrap_or_else(|| json!({})),
           "elegirCarpeta": carpeta, "datos": datos.unwrap_or_else(|| json!({}))})
}

fn confirmar(titulo: &str, texto: &str, aceptar: &str) -> Value {
    json!({"titulo": titulo, "texto": texto, "aceptar": aceptar})
}

pub fn tipo_texto(ctx: &Contexto, info: &Info) -> String {
    if info.tipo == "script" {
        return if info.extra_si("binario") {
            ctx.tr("tipo_programa_suelto", &[])
        } else {
            ctx.tr("tipo_script", &[])
        };
    }
    let clave = TIPOS
        .iter()
        .find(|(t, _)| *t == info.tipo)
        .map_or("tipo_desconocido", |(_, c)| c);
    ctx.tr(clave, &[])
}

fn texto_de(v: &Value, clave: &str) -> String {
    v.get(clave).and_then(Value::as_str).unwrap_or("").to_string()
}

/// Lo que la interfaz necesita para dibujar el archivo: textos, avisos, lista de contenido y botones.
pub fn para_mostrar(ctx: &Contexto, info: &Info) -> Value {
    let ruta = Path::new(&info.ruta);
    let nombre_corto = ruta.parent().map(nombre_de).unwrap_or_default();
    let mut vista = json!({
        "ruta": info.ruta, "nombre": info.nombre, "tipo": info.tipo, "tipoTexto": tipo_texto(ctx, info),
        "tamano": tamano_legible(ctx, info.tamano), "origen": "", "aviso": "", "descripcion": "", "nota": "",
        "error": info.error, "comando": info.falta_comando, "entradas": [], "cuantas": 0, "contenido": "",
        "vista": "", "comandoAdmin": "", "flathub": {}, "instancias": [], "acciones": [],
        "pideContrasena": info.extra_si("pide_contrasena"),
    });
    let mut acciones: Vec<Value> = Vec::new();
    let sitio = detectar::sitio(&info.origen);
    vista["origen"] = if !sitio.is_empty() {
        json!(ctx.tr("origen_sitio", &[("sitio", &sitio)]))
    } else if info.de_internet {
        json!(ctx.tr("origen_descargas", &[]))
    } else {
        json!(ctx.tr("origen_carpeta", &[("carpeta", if nombre_corto.is_empty() { "/" } else { &nombre_corto })]))
    };
    if info.ejecuta_codigo() && info.de_internet {
        vista["aviso"] = json!(ctx.tr("aviso_internet", &[]));
    }
    let aviso = !vista["aviso"].as_str().unwrap_or("").is_empty();
    let t = info.tipo.as_str();
    match t {
        "comprimido" | "programa" => {
            let entradas = &info.entradas;
            vista["cuantas"] = json!(entradas.len());
            vista["contenido"] = json!(ctx.tr(
                "contenido_resumen",
                &[
                    ("n", &entradas.len().to_string()),
                    ("tam", &tamano_legible(ctx, comprimidos::tamano_total(entradas))),
                ]
            ));
            vista["entradas"] = Value::Array(
                entradas
                    .iter()
                    .take(MAX_ENTRADAS)
                    .map(|e| {
                        json!({"nombre": e.nombre, "carpeta": e.tipo == Tipo::Carpeta,
                               "enlace": matches!(e.tipo, Tipo::Enlace | Tipo::Duro),
                               "tamano": if e.tipo == Tipo::Archivo { tamano_legible(ctx, e.tamano) } else { String::new() },
                               "peligrosa": comprimidos::peligrosa(e)})
                    })
                    .collect(),
            );
            let peligrosas = entradas.iter().filter(|e| comprimidos::peligrosa(e)).count();
            if peligrosas > 0 {
                vista["nota"] = json!(ctx.tr("nota_peligrosas", &[("n", &peligrosas.to_string())]));
            }
            let padre = ruta.parent().unwrap_or(Path::new("/"));
            let destino = if entradas.is_empty() {
                padre.join(comprimidos::nombre_sin_extension(&info.nombre))
            } else {
                comprimidos::destino_sugerido(ruta, entradas, padre)
            };
            if t == "programa" {
                let (carpeta, _) = instalar::plan_programa(entradas, ruta);
                vista["descripcion"] = json!(ctx.tr("desc_programa", &[("carpeta", &format!("~/.local/opt/{carpeta}"))]));
                acciones.push(accion("instalar_programa", &ctx.tr("boton_instalar", &[]), "primario", "descargar", None, false, None));
                acciones.push(accion("descomprimir", &ctx.tr("boton_descomprimir_aqui", &[]), "secundario", "caja", None, false, None));
                acciones.push(accion("descomprimir", &ctx.tr("boton_descomprimir_en", &[]), "secundario", "carpeta", None, true, None));
            } else if info.extra_si("pide_contrasena") || !info.error.is_empty() || !entradas.is_empty() {
                vista["descripcion"] = if entradas.is_empty() {
                    json!(ctx.tr("desc_comprimido_sin_lista", &[]))
                } else {
                    json!(ctx.tr("desc_comprimido", &[("destino", &nombre_de(&destino))]))
                };
                if info.error.is_empty() {
                    acciones.push(accion("descomprimir", &ctx.tr("boton_descomprimir", &[]), "primario", "caja", None, false, None));
                    acciones.push(accion("descomprimir", &ctx.tr("boton_descomprimir_en", &[]), "secundario", "carpeta", None, true, None));
                }
            } else {
                vista["descripcion"] = json!(ctx.tr("desc_comprimido_vacio", &[]));
            }
        }
        "appimage" => {
            vista["descripcion"] = json!(ctx.tr("desc_appimage", &[]));
            let conf = aviso.then(|| {
                confirmar(
                    &ctx.tr("conf_instalar_titulo", &[("nombre", &info.nombre)]),
                    &ctx.tr("conf_instalar_appimage", &[]),
                    &ctx.tr("boton_instalar", &[]),
                )
            });
            acciones.push(accion("instalar_appimage", &ctx.tr("boton_instalar", &[]), "primario", "descargar", conf, false, None));
            acciones.push(accion(
                "abrir_appimage",
                &ctx.tr("boton_solo_abrir", &[]),
                "secundario",
                "abrir",
                Some(confirmar(
                    &ctx.tr("conf_abrir_titulo", &[("nombre", &info.nombre)]),
                    &ctx.tr("conf_abrir_appimage", &[]),
                    &ctx.tr("boton_abrir", &[]),
                )),
                false,
                None,
            ));
        }
        "script" => {
            vista["vista"] = json!(info.extra_texto("vista"));
            vista["descripcion"] = json!(if info.extra_si("binario") {
                ctx.tr("desc_binario", &[])
            } else {
                ctx.tr("desc_script", &[])
            });
            if info.extra_si("pide_admin") {
                vista["comandoAdmin"] = json!(info.extra_texto("comando_admin"));
            }
            acciones.push(accion(
                "correr",
                &ctx.tr("boton_abrir_terminal", &[]),
                "primario",
                "terminal",
                Some(confirmar(
                    &ctx.tr("conf_correr_titulo", &[("nombre", &info.nombre)]),
                    &ctx.tr("conf_correr", &[]),
                    &ctx.tr("boton_abrir_terminal", &[]),
                )),
                false,
                None,
            ));
        }
        "deb" | "rpm" => {
            let app = info.extra.get("flathub").filter(|v| v.is_object());
            let base = if t == "deb" { ctx.tr("desc_deb", &[]) } else { ctx.tr("desc_rpm", &[]) };
            let nombre = {
                let n = info.extra_texto("nombre_programa");
                if n.is_empty() { info.nombre.clone() } else { n }
            };
            if let Some(app) = app {
                vista["flathub"] = app.clone();
                vista["descripcion"] = json!(format!(
                    "{base} {}",
                    ctx.tr("desc_en_flathub", &[("nombre", &texto_de(app, "nombre"))])
                ));
                acciones.push(accion(
                    "flathub",
                    &ctx.tr("boton_instalar_flathub", &[]),
                    "primario",
                    "descargar",
                    None,
                    false,
                    Some(json!({"app": texto_de(app, "id"), "nombre": texto_de(app, "nombre")})),
                ));
            } else if info.extra_si("flathub_sin_red") {
                vista["descripcion"] = json!(format!("{base} {}", ctx.tr("desc_flathub_sin_red", &[])));
            } else {
                vista["descripcion"] = json!(format!("{base} {}", ctx.tr("desc_no_flathub", &[("nombre", &nombre)])));
            }
            acciones.push(accion(
                "ver_contenido",
                &ctx.tr("boton_ver_contenido", &[]),
                if app.is_none() { "primario" } else { "secundario" },
                "caja",
                None,
                false,
                None,
            ));
        }
        "flatpakref" | "flatpak" => {
            if !flatpak::hay_flatpak(ctx) {
                vista["descripcion"] = json!(ctx.tr("falta_flatpak", &[]));
                vista["comando"] = json!("sudo pacman -S flatpak");
            } else {
                let nombre = {
                    let n = info.extra_texto("nombre_programa");
                    if n.is_empty() { crate::util::raiz_de_nombre(&info.nombre).to_string() } else { n }
                };
                vista["descripcion"] = json!(if t == "flatpakref" {
                    ctx.tr("desc_flatpakref", &[("nombre", &nombre)])
                } else {
                    ctx.tr("desc_flatpak", &[])
                });
                acciones.push(accion("instalar_flatpak", &ctx.tr("boton_instalar", &[]), "primario", "descargar", None, false, None));
            }
        }
        "windows" => {
            let opciones: Vec<Value> = info
                .extra
                .get("opciones_windows")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let app = info.extra.get("flathub").filter(|v| v.is_object());
            let mut texto = match opciones.first() {
                Some(o) => ctx.tr("desc_windows_con", &[("programa", &texto_de(o, "nombre"))]),
                None => ctx.tr("desc_windows_sin", &[]),
            };
            if let Some(app) = app {
                vista["flathub"] = app.clone();
                texto.push(' ');
                texto.push_str(&ctx.tr("desc_windows_flathub", &[("nombre", &texto_de(app, "nombre"))]));
                acciones.push(accion(
                    "flathub",
                    &ctx.tr("boton_instalar_flathub_linux", &[]),
                    "primario",
                    "descargar",
                    None,
                    false,
                    Some(json!({"app": texto_de(app, "id"), "nombre": texto_de(app, "nombre")})),
                ));
            }
            vista["descripcion"] = json!(texto);
            for o in &opciones {
                let tipo = if app.is_some() || !acciones.is_empty() { "secundario" } else { "primario" };
                acciones.push(accion(
                    "windows",
                    &ctx.tr("boton_abrir_con", &[("programa", &texto_de(o, "nombre"))]),
                    tipo,
                    "abrir",
                    Some(confirmar(
                        &ctx.tr("conf_abrir_titulo", &[("nombre", &info.nombre)]),
                        &ctx.tr("conf_windows", &[]),
                        &ctx.tr("boton_abrir", &[]),
                    )),
                    false,
                    Some(json!({"opcion": texto_de(o, "id")})),
                ));
            }
            if opciones.is_empty() {
                vista["comando"] = json!("sudo pacman -S wine");
            }
        }
        "mod" => {
            let m = info.extra.get("mod").cloned().unwrap_or_else(|| json!({}));
            let loader = match texto_de(&m, "loader").as_str() {
                "fabric" => "Fabric",
                "quilt" => "Quilt",
                "forge" => "Forge",
                "neoforge" => "NeoForge",
                _ => "",
            };
            let nombre = {
                let n = texto_de(&m, "nombre");
                if n.is_empty() { info.nombre.clone() } else { n }
            };
            vista["descripcion"] = json!(ctx.tr("desc_mod", &[("nombre", &nombre), ("loader", loader)]));
            vista["mod"] = m.clone();
            let instancias = info.extra.get("instancias");
            let hay = instancias.and_then(Value::as_array).is_some_and(|l| !l.is_empty());
            match instancias {
                Some(Value::Array(l)) if !l.is_empty() => {
                    let mod_loader = texto_de(&m, "loader");
                    vista["instancias"] = Value::Array(
                        l.iter()
                            .map(|i| {
                                let il = texto_de(i, "loader");
                                let compatible = il.is_empty()
                                    || il == mod_loader
                                    || (mod_loader == "fabric" && il == "quilt");
                                let mut o = i.as_object().cloned().unwrap_or_default();
                                o.insert("compatible".into(), json!(compatible));
                                Value::Object(o)
                            })
                            .collect(),
                    );
                    vista["nota"] = json!(ctx.tr("nota_mod_instancias", &[]));
                }
                Some(Value::Null) | None => vista["nota"] = json!(ctx.tr("nota_mod_sin_comando", &[])),
                _ => vista["nota"] = json!(ctx.tr("nota_mod_sin_instancias", &[])),
            }
            acciones.push(accion(
                "mod_carpeta",
                &ctx.tr("boton_elegir_carpeta", &[]),
                if hay { "secundario" } else { "primario" },
                "carpeta",
                None,
                true,
                None,
            ));
        }
        "jar" => {
            vista["descripcion"] = json!(ctx.tr("desc_jar", &[]));
            acciones.push(accion(
                "java",
                &ctx.tr("boton_abrir_java", &[]),
                "primario",
                "abrir",
                Some(confirmar(
                    &ctx.tr("conf_abrir_titulo", &[("nombre", &info.nombre)]),
                    &ctx.tr("conf_java", &[]),
                    &ctx.tr("boton_abrir", &[]),
                )),
                false,
                None,
            ));
            if ctx.which("java").is_none() {
                vista["comando"] = json!("sudo pacman -S jre-openjdk");
                acciones.clear();
            }
        }
        "carpeta" => vista["descripcion"] = json!(ctx.tr("desc_carpeta", &[])),
        "falta" => vista["descripcion"] = json!(ctx.tr("desc_falta", &[])),
        _ => vista["descripcion"] = json!(ctx.tr("desc_desconocido", &[])),
    }
    if t != "falta" {
        let tipo = if acciones.is_empty() { "primario" } else { "fantasma" };
        acciones.push(accion("abrir_carpeta", &ctx.tr("boton_abrir_carpeta", &[]), tipo, "carpeta", None, false, None));
    }
    vista["acciones"] = Value::Array(acciones);
    vista
}

// ------------------------------------------------------------------------------------ hacer

fn resultado(titulo: &str, texto: &str, carpeta: &str, abrir: &str, deshacible: bool) -> Map<String, Value> {
    Map::from_iter([
        ("titulo".to_string(), json!(titulo)),
        ("texto".into(), json!(texto)),
        ("carpeta".into(), json!(carpeta)),
        ("abrir".into(), json!(abrir)),
        ("deshacible".into(), json!(deshacible)),
    ])
}

/// «/home/mike/Descargas/x» → «~/Descargas/x».
fn corto(ctx: &Contexto, ruta: &Path) -> String {
    let casa = ctx.rutas.casa.to_string_lossy().into_owned();
    let s = ruta.to_string_lossy().into_owned();
    if s == casa || s.starts_with(&format!("{casa}/")) {
        format!("~{}", &s[casa.len()..])
    } else {
        s
    }
}

fn opcion_texto(o: &Opciones, clave: &str) -> String {
    match o.get(clave) {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

fn opcion_si(o: &Opciones, clave: &str) -> bool {
    match o.get(clave) {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        _ => false,
    }
}

/// Lo que dejó un error: lo que se ve tal cual pasa, lo demás se vuelve un `Problema` para la persona.
fn traducir(ctx: &Contexto, e: Error) -> Error {
    match e {
        Error::Conflicto { .. } | Error::Contrasena { .. } | Error::Cancelado | Error::Problema { .. } => e,
        Error::FaltaPrograma { mensaje, comando, .. } => Error::Problema { mensaje, comando },
        Error::Mensaje(m) => problema(m),
        Error::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => problema(ctx.tr("sin_permiso", &[])),
        Error::Io(e) if e.raw_os_error() == Some(28) => problema(ctx.tr("sin_espacio", &[])),
        Error::Io(e) => problema(ctx.tr("error_general", &[("detalle", &Error::texto_io(&e))])),
    }
}

pub fn ejecutar(ctx: &Contexto, info: &Info, accion: &str, opciones: &Opciones, control: &Control) -> Resultado<Value> {
    hacer(ctx, info, accion, opciones, control).map_err(|e| traducir(ctx, e))
}

fn hacer(ctx: &Contexto, info: &Info, accion: &str, opciones: &Opciones, control: &Control) -> Resultado<Value> {
    let ruta = PathBuf::from(&info.ruta);
    let contrasena = Some(opcion_texto(opciones, "contrasena")).filter(|c| !c.is_empty());
    let contrasena = contrasena.as_deref();
    let hecho = |m: Map<String, Value>| Ok(Value::Object(m));
    match accion {
        "descomprimir" => descomprimir(ctx, info, &ruta, opciones, contrasena, control),
        "instalar_programa" => {
            let entradas = if info.entradas.is_empty() {
                comprimidos::listar(ctx, &ruta, contrasena)?
            } else {
                info.entradas.clone()
            };
            let e = instalar::instalar_programa(ctx, &ruta, &entradas, opcion_si(opciones, "reemplazar"), contrasena, control)?;
            let nombre = registro::texto_de(&e, "nombre");
            let mut r = resultado(
                &ctx.tr("hecho_instalado", &[("nombre", nombre)]),
                &ctx.tr("hecho_en_lanzador", &[]),
                "",
                registro::texto_de(&e, "abrir"),
                true,
            );
            r.insert("id".into(), json!(registro::texto_de(&e, "id")));
            hecho(r)
        }
        "instalar_appimage" => {
            let e = instalar::instalar_appimage(ctx, &ruta, opcion_si(opciones, "reemplazar"), control)?;
            let nombre = registro::texto_de(&e, "nombre");
            let mut r = resultado(
                &ctx.tr("hecho_instalado", &[("nombre", nombre)]),
                &ctx.tr("hecho_en_lanzador", &[]),
                "",
                registro::texto_de(&e, "abrir"),
                true,
            );
            r.insert("id".into(), json!(registro::texto_de(&e, "id")));
            hecho(r)
        }
        "abrir_appimage" => {
            instalar::abrir_appimage(ctx, &ruta)?;
            hecho(resultado(
                &ctx.tr("hecho_abierto", &[("nombre", &info.nombre)]),
                &ctx.tr("hecho_abierto_appimage", &[]),
                "",
                "",
                false,
            ))
        }
        "correr" => {
            instalar::correr_script(ctx, &ruta)?;
            hecho(resultado(&ctx.tr("hecho_terminal", &[]), &ctx.tr("hecho_terminal_texto", &[]), "", "", false))
        }
        "flathub" => {
            let de_info = info.extra.get("flathub").filter(|v| v.is_object());
            let mut app_id = opcion_texto(opciones, "app");
            if app_id.is_empty() {
                app_id = de_info.map(|a| texto_de(a, "id")).unwrap_or_default();
            }
            let mut nombre = opcion_texto(opciones, "nombre");
            if nombre.is_empty() {
                nombre = de_info.map(|a| texto_de(a, "nombre")).unwrap_or_default();
            }
            if nombre.is_empty() {
                nombre = app_id.clone();
            }
            if app_id.is_empty() {
                return Err(problema(ctx.tr("flatpak_fallo", &[])));
            }
            flatpak::instalar_flathub(ctx, &app_id, control)?;
            let e = registro::agregar(
                ctx,
                Map::from_iter([
                    ("id".to_string(), json!(format!("flatpak-{app_id}"))),
                    ("tipo".into(), json!("flatpak")),
                    ("nombre".into(), json!(nombre)),
                    ("flatpak".into(), json!(app_id)),
                    ("archivo".into(), json!(info.nombre)),
                    ("rutas".into(), json!([])),
                ]),
            )?;
            let mut r = resultado(
                &ctx.tr("hecho_instalado", &[("nombre", &nombre)]),
                &ctx.tr("hecho_en_lanzador", &[]),
                "",
                "",
                true,
            );
            r.insert("flatpak".into(), json!(app_id));
            r.insert("id".into(), json!(registro::texto_de(&e, "id")));
            hecho(r)
        }
        "instalar_flatpak" => {
            let app_id = flatpak::instalar_archivo(ctx, &ruta, &info.tipo, control)?;
            let nombre = [info.extra_texto("nombre_programa"), app_id.clone().unwrap_or_default(), info.nombre.clone()]
                .into_iter()
                .find(|n| !n.is_empty())
                .unwrap_or_default();
            if let Some(id) = &app_id {
                registro::agregar(
                    ctx,
                    Map::from_iter([
                        ("id".to_string(), json!(format!("flatpak-{id}"))),
                        ("tipo".into(), json!("flatpak")),
                        ("nombre".into(), json!(nombre)),
                        ("flatpak".into(), json!(id)),
                        ("archivo".into(), json!(info.nombre)),
                        ("rutas".into(), json!([])),
                    ]),
                )?;
            }
            let mut r = resultado(
                &ctx.tr("hecho_instalado", &[("nombre", &nombre)]),
                &ctx.tr("hecho_en_lanzador", &[]),
                "",
                "",
                app_id.is_some(),
            );
            r.insert("flatpak".into(), json!(app_id.unwrap_or_default()));
            hecho(r)
        }
        "ver_contenido" => ver_contenido(ctx, info, &ruta, opciones, control),
        "windows" => {
            instalar::abrir_windows(ctx, &ruta, &opcion_texto(opciones, "opcion"))?;
            hecho(resultado(
                &ctx.tr("hecho_abierto", &[("nombre", &info.nombre)]),
                &ctx.tr("hecho_windows", &[]),
                "",
                "",
                false,
            ))
        }
        "java" => {
            instalar::abrir_java(ctx, &ruta)?;
            hecho(resultado(
                &ctx.tr("hecho_abierto", &[("nombre", &info.nombre)]),
                &ctx.tr("hecho_java", &[]),
                "",
                "",
                false,
            ))
        }
        "mod_instancia" | "mod_carpeta" => mod_(ctx, info, &ruta, accion, opciones),
        "abrir_carpeta" => {
            instalar::abrir_carpeta(ctx, &ruta)?;
            Ok(json!({}))
        }
        _ => Err(problema(ctx.tr("error_general", &[("detalle", accion)]))),
    }
}

fn registrar_descomprimido(ctx: &Contexto, info: &Info, destino: &Path) -> Resultado<()> {
    let nombre = nombre_de(destino);
    registro::agregar(
        ctx,
        Map::from_iter([
            ("id".to_string(), json!(registro::id_libre(ctx, "descomprimido", &nombre))),
            ("tipo".into(), json!("descomprimido")),
            ("nombre".into(), json!(nombre)),
            ("rutas".into(), json!([destino.to_string_lossy()])),
            ("archivo".into(), json!(info.nombre)),
            ("abrir".into(), json!(destino.to_string_lossy())),
        ]),
    )?;
    Ok(())
}

fn descomprimir(
    ctx: &Contexto,
    info: &Info,
    ruta: &Path,
    opciones: &Opciones,
    contrasena: Option<&str>,
    control: &Control,
) -> Resultado<Value> {
    let entradas: Vec<Entrada> = if info.entradas.is_empty() || contrasena.is_some() {
        comprimidos::listar(ctx, ruta, contrasena)?
    } else {
        info.entradas.clone()
    };
    let carpeta = opcion_texto(opciones, "carpeta");
    let padre = if carpeta.is_empty() {
        ruta.parent().unwrap_or(Path::new("/")).to_path_buf()
    } else {
        PathBuf::from(carpeta)
    };
    let nombre = opcion_texto(opciones, "nombre");
    let destino = if nombre.is_empty() {
        comprimidos::destino_sugerido(ruta, &entradas, &padre)
    } else {
        padre.join(nombre)
    };
    let r = comprimidos::extraer(ctx, ruta, &destino, Some(&entradas), contrasena, control, true)?;
    registrar_descomprimido(ctx, info, &r.destino)?;
    let mut texto = ctx.tr("hecho_en_carpeta", &[("carpeta", &corto(ctx, &r.destino))]);
    if !r.saltados.is_empty() {
        texto.push(' ');
        texto.push_str(&ctx.tr("hecho_saltados", &[("n", &r.saltados.len().to_string())]));
    }
    let mut m = resultado(&ctx.tr("hecho_descomprimido", &[]), &texto, &r.destino.to_string_lossy(), "", true);
    m.insert("saltados".into(), json!(r.saltados.len()));
    Ok(Value::Object(m))
}

/// Saca lo que trae un .deb (su data.tar) o un .rpm a una carpeta, solo para verlo.
fn ver_contenido(ctx: &Contexto, info: &Info, ruta: &Path, opciones: &Opciones, control: &Control) -> Resultado<Value> {
    let carpeta = opcion_texto(opciones, "carpeta");
    let padre = if carpeta.is_empty() {
        ruta.parent().unwrap_or(Path::new("/")).to_path_buf()
    } else {
        PathBuf::from(carpeta)
    };
    let nombre = opcion_texto(opciones, "nombre");
    let destino = padre.join(if nombre.is_empty() {
        comprimidos::nombre_sin_extension(&info.nombre)
    } else {
        nombre
    });
    if crate::util::existe_o_enlace(&destino) {
        return Err(Error::Conflicto {
            sugerida: comprimidos::nombre_libre(&destino),
            ruta: destino,
        });
    }
    let res = if info.tipo == "deb" {
        let tmp = tempfile::Builder::new().prefix("vortex-deb-").tempdir()?;
        let datos = comprimidos::listar(ctx, ruta, None)?
            .into_iter()
            .map(|e| e.nombre)
            .find(|n| n.starts_with("data.tar"))
            .ok_or_else(|| problema(ctx.tr("comprimido_danado", &[])))?;
        let r = comprimidos::correr(
            ctx,
            &comprimidos::bsdtar(ctx)?,
            &["-xf".into(), ruta.to_string_lossy().into_owned(), "-C".into(), tmp.path().to_string_lossy().into_owned(), datos.clone()],
            Some(std::time::Duration::from_secs(600)),
        )?;
        if r.codigo != Some(0) {
            return Err(problema(ctx.tr("comprimido_danado", &[])));
        }
        comprimidos::extraer(ctx, &tmp.path().join(&datos), &destino, None, None, control, false)?
    } else {
        comprimidos::extraer(ctx, ruta, &destino, None, None, control, false)?
    };
    registrar_descomprimido(ctx, info, &res.destino)?;
    Ok(Value::Object(resultado(
        &ctx.tr("hecho_descomprimido", &[]),
        &ctx.tr("hecho_en_carpeta", &[("carpeta", &corto(ctx, &res.destino))]),
        &res.destino.to_string_lossy(),
        "",
        true,
    )))
}

fn mod_(ctx: &Contexto, info: &Info, ruta: &Path, accion: &str, opciones: &Opciones) -> Resultado<Value> {
    let nombre_mod = info
        .extra
        .get("mod")
        .map(|m| texto_de(m, "nombre"))
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| info.nombre.clone());
    let (destino, donde) = if accion == "mod_instancia" {
        let r = opcion_texto(opciones, "ruta");
        let destino = mods::mover(ctx, ruta, &r)?;
        let inst = opcion_texto(opciones, "instancia");
        (destino, if inst.is_empty() { nombre_de(Path::new(&r)) } else { inst })
    } else {
        let nombre = Some(opcion_texto(opciones, "nombre")).filter(|n| !n.is_empty());
        let destino = mods::copiar_a_carpeta(ruta, Path::new(&opcion_texto(opciones, "carpeta")), nombre.as_deref())?;
        let padre = destino.parent().unwrap_or(Path::new("/")).to_path_buf();
        let donde = if nombre_de(&padre) == "mods" {
            padre.parent().map(nombre_de).unwrap_or_default()
        } else {
            nombre_de(&padre)
        };
        (destino, donde)
    };
    let padre = destino.parent().unwrap_or(Path::new("/")).to_path_buf();
    registro::agregar(
        ctx,
        Map::from_iter([
            ("id".to_string(), json!(registro::id_libre(ctx, "mod", &nombre_mod))),
            ("tipo".into(), json!("mod")),
            ("nombre".into(), json!(nombre_mod)),
            ("rutas".into(), json!([destino.to_string_lossy()])),
            ("archivo".into(), json!(info.nombre)),
            ("abrir".into(), json!(padre.to_string_lossy())),
            ("detalle".into(), json!(donde)),
        ]),
    )?;
    Ok(Value::Object(resultado(
        &ctx.tr("hecho_mod", &[("nombre", &nombre_mod), ("donde", &donde)]),
        &ctx.tr("hecho_mod_texto", &[]),
        &padre.to_string_lossy(),
        "",
        true,
    )))
}

// ------------------------------------------------------------------------------------ pestaña «Instalado»

const TIPO_INSTALADO: [(&str, &str); 5] = [
    ("appimage", "instalado_appimage"),
    ("programa", "instalado_programa"),
    ("flatpak", "instalado_flatpak"),
    ("mod", "instalado_mod"),
    ("descomprimido", "instalado_descomprimido"),
];

/// Las claves de texto de «Instalado» (las pruebas revisan que existan).
pub fn claves_de_tipos() -> Vec<&'static str> {
    TIPOS.iter().map(|(_, c)| *c).chain(TIPO_INSTALADO.iter().map(|(_, c)| *c)).collect()
}

pub fn instalados(ctx: &Contexto) -> Vec<Value> {
    registro::cargar(ctx)
        .iter()
        .map(|e| {
            let tipo = registro::texto_de(e, "tipo");
            let mut fecha = registro::texto_de(e, "fecha").chars().take(10).collect::<String>();
            if fecha.len() == 10 {
                let p: Vec<&str> = fecha.split('-').collect();
                if p.len() == 3 {
                    fecha = if ctx.idioma == crate::textos::Idioma::Es {
                        format!("{}/{}/{}", p[2], p[1], p[0])
                    } else {
                        format!("{}-{}-{}", p[0], p[1], p[2])
                    };
                }
            }
            let abrir = registro::texto_de(e, "abrir");
            let id = registro::texto_de(e, "id");
            let tipo_texto = if tipo == "descomprimido" && !abrir.is_empty() && Path::new(abrir).is_file() {
                ctx.tr("instalado_descomprimido_archivo", &[])
            } else {
                let clave = TIPO_INSTALADO.iter().find(|(t, _)| *t == tipo).map_or("instalado_programa", |(_, c)| c);
                ctx.tr(clave, &[])
            };
            let detalle = {
                let d = registro::texto_de(e, "detalle");
                if d.is_empty() { registro::texto_de(e, "archivo") } else { d }
            };
            let nombre = {
                let n = registro::texto_de(e, "nombre");
                if n.is_empty() { id.to_string() } else { n.to_string() }
            };
            json!({"id": registro::texto_de(e, "id"), "tipo": tipo, "nombre": nombre, "tipoTexto": tipo_texto,
                   "fecha": fecha, "icono": registro::texto_de(e, "icono"), "abrir": abrir, "detalle": detalle,
                   "boton": if matches!(tipo, "appimage" | "programa" | "flatpak") {
                       ctx.tr("boton_desinstalar", &[])
                   } else {
                       ctx.tr("boton_quitar", &[])
                   }})
        })
        .collect()
}

/// Deshace una entrada de «Instalado» y dice qué se hizo.
pub fn deshacer(ctx: &Contexto, id: &str) -> Result<String, Error> {
    let e = match registro::deshacer(ctx, id) {
        Ok(Some(e)) => e,
        Ok(None) => return Err(problema(ctx.tr("ya_no_esta", &[]))),
        Err(Error::Mensaje(m)) => return Err(problema(m)),
        Err(Error::Io(e)) => {
            return Err(problema(ctx.tr("error_general", &[("detalle", &Error::texto_io(&e))])));
        }
        Err(e) => return Err(e),
    };
    let tipo = registro::texto_de(&e, "tipo");
    let clave = if matches!(tipo, "appimage" | "programa" | "flatpak") { "deshecho_app" } else { "deshecho_otro" };
    let nombre = {
        let n = registro::texto_de(&e, "nombre");
        if n.is_empty() { id } else { n }
    };
    Ok(ctx.tr(clave, &[("nombre", nombre)]))
}

pub fn abrir_instalado(ctx: &Contexto, id: &str) -> Result<(), Error> {
    let Some(e) = registro::buscar(ctx, id) else {
        return Err(problema(ctx.tr("ya_no_esta", &[])));
    };
    let abrir = registro::texto_de(&e, "abrir").to_string();
    match registro::texto_de(&e, "tipo") {
        "flatpak" => flatpak::abrir(ctx, registro::texto_de(&e, "flatpak")),
        "appimage" => ctx.lanzar(&[abrir], &ctx.entorno_appimage(), None),
        "programa" => {
            let carpeta = Path::new(&abrir).parent().map(Path::to_path_buf);
            ctx.lanzar(&[abrir], &[], carpeta.as_deref())
        }
        _ => instalar::abrir_carpeta(ctx, Path::new(&abrir)),
    }
}
