//! El cerebro de Miku con cualquier IA compatible con OpenAI (la local con Ollama, Gemini, ChatGPT…).
//!
//! Hace lo que con Claude hace `claude -p`: lleva la conversación, levanta los mismos servidores MCP (la
//! vista de vortex-mcp —solo ver_pantalla, ventana_activa, ventanas y estado_sistema— y las acciones de
//! Miku, que piden tu permiso en la ventana) y corre las herramientas que pida la IA, hasta 8 vueltas por
//! pregunta. La IA no tiene nada más: ni comandos, ni archivos, ni internet. Las capturas le llegan como
//! imagen (si el modelo no ve imágenes, se le dice). La conversación vive solo en memoria.

use crate::cerebro::{HERRAMIENTAS_PERMITIDAS, encontrar_aux, encontrar_mcp, leer_persona};
use crate::config::Config;
use crate::eventos::{AlEvento, Cerebro as CerebroT, Evento};
use crate::fecha::{self, DIAS, MESES};
use crate::http::Cortador;
use crate::ia::{ErrorIA, Ia, Proveedor};
use crate::mcp::ServidorMcp;
use crate::rutas::Rutas;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const VUELTAS: usize = 8;
pub const MENSAJES_MAXIMOS: usize = 40;
pub const TIEMPO_LECTURA_LOCAL: u64 = 120;
pub const TIEMPO_LECTURA: u64 = 60;
pub const TIEMPO_HERRAMIENTA: u64 = 300;
pub const SIN_IMAGENES: &str =
    "(No puedo ver imágenes con este modelo: la imagen no me llegó. Dilo así y no inventes lo que hay.)";

/// (nombre, comando, entorno) de un servidor MCP.
pub type DefServidor = (String, Vec<String>, Vec<(String, String)>);

/// Los mensajes con las imágenes cambiadas por un texto (para el historial, o si el modelo no ve).
pub fn sin_imagenes(mensajes: &[Value]) -> Vec<Value> {
    mensajes
        .iter()
        .map(|m| {
            if let Some(partes) = m["content"].as_array() {
                let mut textos: Vec<String> = partes
                    .iter()
                    .filter(|c| c["type"] == "text")
                    .map(|c| c["text"].as_str().unwrap_or("").to_string())
                    .collect();
                if partes.iter().any(|c| c["type"] == "image_url") {
                    textos.push("[imagen]".into());
                }
                let mut m = m.clone();
                m["content"] = json!(
                    textos
                        .into_iter()
                        .filter(|t| !t.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                m
            } else {
                m.clone()
            }
        })
        .collect()
}

/// La fecha y la hora, que Claude Code ya sabe y estas IA no («¿qué día es hoy?»).
pub fn hoy() -> String {
    let t = fecha::ahora();
    format!(
        "\n\nHoy es {} {} de {} de {}, y son las {}:{:02}.",
        DIAS[t.dia_semana()],
        t.dia,
        MESES[t.mes as usize - 1],
        t.anio,
        t.hora,
        t.minuto
    )
}

/// Los últimos mensajes, empezando siempre en una pregunta (sin cortar una llamada a medias).
pub fn recortar(historial: Vec<Value>) -> Vec<Value> {
    if historial.len() <= MENSAJES_MAXIMOS {
        return historial;
    }
    let mut resto = historial[historial.len() - MENSAJES_MAXIMOS..].to_vec();
    while resto.first().is_some_and(|m| m["role"] != "user") {
        resto.remove(0);
    }
    resto
}

struct Herramientas {
    lista: Vec<Value>,
    /// nombre expuesto a la IA → (índice del servidor, herramienta, nombre mcp__…)
    rutas: BTreeMap<String, (usize, String, String)>,
}

pub struct CerebroAbierto {
    pub avatar: bool,
    al_evento: AlEvento,
    pub proveedor: Proveedor,
    ia: Arc<Ia>,
    persona: Mutex<String>,
    sentimientos: Mutex<String>,
    autorizar: Mutex<Option<String>>,
    ocupado: Arc<AtomicBool>,
    pub historial: Mutex<Vec<Value>>,
    servidores_fijos: Option<Vec<DefServidor>>,
    servidores: Mutex<Vec<Arc<ServidorMcp>>>,
    cortador: Mutex<Option<Cortador>>,
    token: Mutex<Option<Arc<AtomicBool>>>,
    mcp: Option<PathBuf>,
    aux: PathBuf,
}

impl CerebroAbierto {
    pub fn nuevo(
        rutas: &Rutas,
        ia: Arc<Ia>,
        avatar: bool,
        al_evento: AlEvento,
        cfg: &Config,
        servidores: Option<Vec<DefServidor>>,
    ) -> Self {
        let mcp = encontrar_mcp();
        let mut persona = leer_persona(rutas, avatar);
        if mcp.is_none() && servidores.is_none() {
            persona += "\n\nEn esta sesión no tienes acceso a la pantalla: si te piden mirarla, explica que no puedes.";
        }
        Self {
            avatar,
            al_evento,
            proveedor: ia.elegido(cfg),
            ia,
            persona: Mutex::new(persona),
            sentimientos: Mutex::new(String::new()),
            autorizar: Mutex::new(None),
            ocupado: Arc::new(AtomicBool::new(false)),
            historial: Mutex::new(vec![]),
            servidores_fijos: servidores,
            servidores: Mutex::new(vec![]),
            cortador: Mutex::new(None),
            token: Mutex::new(None),
            mcp,
            aux: encontrar_aux(),
        }
    }

    /// Las herramientas en formato OpenAI y de dónde sale cada una.
    fn herramientas(&self) -> Result<(Vec<Arc<ServidorMcp>>, Herramientas), String> {
        {
            let mut s = self.servidores.lock().unwrap();
            if s.is_empty() || !s.iter().all(|x| x.vivo()) {
                s.clear();
                let defs: Vec<DefServidor> = match &self.servidores_fijos {
                    Some(f) => f.clone(),
                    None => {
                        let mut d = vec![];
                        if let Some(m) = &self.mcp {
                            d.push(("vortex".into(), vec![m.to_string_lossy().into_owned()], vec![]));
                        }
                        if let Some(a) = self.autorizar.lock().unwrap().clone() {
                            d.push((
                                "miku".into(),
                                vec![self.aux.to_string_lossy().into_owned(), "mcp".into()],
                                vec![("VORTEX_MIKU_AUTORIZAR".into(), a)],
                            ));
                        }
                        d
                    }
                };
                for (n, c, e) in defs {
                    s.push(Arc::new(ServidorMcp::arrancar(&n, &c, &e)?));
                }
            }
        }
        let servidores = self.servidores.lock().unwrap().clone();
        let permitidas: Vec<&str> = HERRAMIENTAS_PERMITIDAS
            .iter()
            .map(|h| h.trim_start_matches("mcp__vortex__"))
            .collect();
        let (mut lista, mut rutas) = (vec![], BTreeMap::new());
        for (i, s) in servidores.iter().enumerate() {
            for h in &s.herramientas {
                let nombre_h = h["name"].as_str().unwrap_or("");
                if s.nombre == "vortex" && !permitidas.contains(&nombre_h) {
                    continue; // portapapeles e historial de la terminal: nunca
                }
                let nombre = format!("{}__{nombre_h}", s.nombre);
                rutas.insert(
                    nombre.clone(),
                    (i, nombre_h.to_string(), format!("mcp__{}__{nombre_h}", s.nombre)),
                );
                lista.push(json!({"type": "function", "function": {
                    "name": nombre,
                    "description": h["description"].as_str().unwrap_or(""),
                    "parameters": if h["inputSchema"].is_null() { json!({"type": "object", "properties": {}}) } else { h["inputSchema"].clone() }}}));
            }
        }
        Ok((servidores, Herramientas { lista, rutas }))
    }

    /// Corre una herramienta. (texto para la IA, [(mime, base64), …] de imágenes).
    fn correr(
        &self,
        servidores: &[Arc<ServidorMcp>],
        h: &Herramientas,
        nombre: &str,
        argumentos: &str,
    ) -> (String, Vec<(String, String)>) {
        let Some((i, herramienta, _)) = h.rutas.get(nombre) else {
            return (format!("No existe la herramienta {nombre}."), vec![]);
        };
        let args: Value = if argumentos.trim().is_empty() {
            json!({})
        } else {
            match serde_json::from_str(argumentos) {
                Ok(v) => v,
                Err(_) => return ("Los argumentos no eran JSON válido.".into(), vec![]),
            }
        };
        let args = if args.is_object() { args } else { json!({}) };
        let res = match servidores[*i].pedir(
            "tools/call",
            json!({"name": herramienta, "arguments": args}),
            Duration::from_secs(TIEMPO_HERRAMIENTA),
        ) {
            Ok(r) => r,
            Err(e) => return (format!("La herramienta falló: {e}"), vec![]),
        };
        let (mut textos, mut imagenes) = (vec![], vec![]);
        for c in res["content"].as_array().cloned().unwrap_or_default() {
            match c["type"].as_str() {
                Some("text") => textos.push(c["text"].as_str().unwrap_or("").to_string()),
                Some("image") if c["data"].as_str().is_some_and(|d| !d.is_empty()) => {
                    imagenes.push((
                        c["mimeType"].as_str().unwrap_or("image/png").to_string(),
                        c["data"].as_str().unwrap().to_string(),
                    ));
                }
                _ => {}
            }
        }
        let t: Vec<String> = textos.into_iter().filter(|t| !t.is_empty()).collect();
        let mut texto = if t.is_empty() {
            if imagenes.is_empty() {
                "Listo.".to_string()
            } else {
                "Listo: te mando la imagen.".to_string()
            }
        } else {
            t.join("\n")
        };
        if res["isError"].as_bool().unwrap_or(false) {
            texto = format!("Error: {texto}");
        }
        (texto, imagenes)
    }

    /// Una respuesta de la IA en flujo: avisa del texto y devuelve (texto, llamadas a herramientas).
    fn una_vuelta(
        &self,
        mensajes: &[Value],
        herramientas: &[Value],
        avisar: &dyn Fn(Evento),
        token: &AtomicBool,
    ) -> Result<(String, Vec<Value>), ErrorIA> {
        let mut cuerpo = json!({"messages": mensajes, "stream": true});
        if !herramientas.is_empty() {
            cuerpo["tools"] = json!(herramientas);
        }
        let inactivo = if self.proveedor.id == "local" {
            TIEMPO_LECTURA_LOCAL
        } else {
            TIEMPO_LECTURA
        };
        let guardar = |c: Cortador| *self.cortador.lock().unwrap() = Some(c);
        let mut r = self
            .ia
            .pedir(&self.proveedor, &cuerpo, inactivo, Some(&guardar))?;
        let (mut contenido, mut llamadas): (String, BTreeMap<i64, Value>) = (String::new(), BTreeMap::new());
        let mut cortada = false;
        while let Some(l) = r.linea() {
            if token.load(Ordering::SeqCst) {
                cortada = true;
                break;
            }
            let l = String::from_utf8_lossy(&l).trim().to_string();
            let Some(dato) = l.strip_prefix("data:") else {
                continue;
            };
            let dato = dato.trim();
            if dato == "[DONE]" {
                break;
            }
            let Ok(ev) = serde_json::from_str::<Value>(dato) else {
                continue;
            };
            if !ev["error"].is_null() && ev["error"] != json!(false) {
                let m = ev["error"]["message"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| ev["error"].to_string());
                *self.cortador.lock().unwrap() = None;
                return Err(ErrorIA::nuevo(
                    "otro",
                    format!("{} contestó con un error: {m}", self.proveedor.nombre),
                ));
            }
            let delta = &ev["choices"][0]["delta"];
            if let Some(c) = delta["content"].as_str().filter(|c| !c.is_empty()) {
                contenido += c;
                avisar(Evento::Texto(c.into()));
            }
            for t in delta["tool_calls"].as_array().cloned().unwrap_or_default() {
                let i = t["index"].as_i64().unwrap_or(llamadas.len() as i64);
                let ll = llamadas.entry(i).or_insert_with(
                    || json!({"id": "", "type": "function", "function": {"name": "", "arguments": ""}}),
                );
                if let Some(id) = t["id"].as_str().filter(|s| !s.is_empty()) {
                    ll["id"] = json!(id);
                }
                let f = &t["function"];
                let nombre = format!(
                    "{}{}",
                    ll["function"]["name"].as_str().unwrap_or(""),
                    f["name"].as_str().unwrap_or("")
                );
                ll["function"]["name"] = json!(nombre);
                let a = match &f["arguments"] {
                    Value::String(s) => s.clone(),
                    Value::Null | Value::Bool(false) => String::new(),
                    otro => otro.to_string(),
                };
                let args = format!("{}{a}", ll["function"]["arguments"].as_str().unwrap_or(""));
                ll["function"]["arguments"] = json!(args);
            }
        }
        if !cortada
            && !token.load(Ordering::SeqCst)
            && r.fallo().is_some()
            && contenido.is_empty()
            && llamadas.is_empty()
        {
            *self.cortador.lock().unwrap() = None;
            return Err(ErrorIA::nuevo(
                "internet",
                format!("Se cortó la respuesta de {}.", self.proveedor.nombre),
            ));
        }
        *self.cortador.lock().unwrap() = None;
        if !contenido.is_empty() {
            avisar(Evento::BloqueFin);
        }
        let mut lista = vec![];
        for (i, mut ll) in llamadas {
            if !ll["function"]["name"].as_str().unwrap_or("").is_empty() {
                if ll["id"].as_str().unwrap_or("").is_empty() {
                    ll["id"] = json!(format!("llamada_{i}"));
                }
                lista.push(ll);
            }
        }
        Ok((contenido, lista))
    }
}

impl CerebroT for CerebroAbierto {
    fn comprobar(&self) -> (String, String) {
        self.ia.comprobar(&self.proveedor)
    }

    fn cancelar(&self) {
        if let Some(t) = &*self.token.lock().unwrap() {
            t.store(true, Ordering::SeqCst);
        }
        if let Some(c) = &*self.cortador.lock().unwrap() {
            c.cortar();
        }
    }

    fn olvidar(&self) {
        self.cancelar();
        self.historial.lock().unwrap().clear();
        for s in self.servidores.lock().unwrap().drain(..) {
            s.cerrar();
        }
    }

    fn preguntar(&self, texto: &str, etiqueta: u64) {
        let token = Arc::new(AtomicBool::new(false));
        *self.token.lock().unwrap() = Some(token.clone());
        let avisar = |ev: Evento| {
            if !token.load(Ordering::SeqCst) {
                (self.al_evento)(etiqueta, ev);
            }
        };
        let (clave, mensaje) = self.comprobar();
        if clave != "ok" {
            avisar(Evento::Error { clave, mensaje });
            return;
        }
        // Sin herramientas también se puede platicar.
        let (servidores, herr) = self.herramientas().unwrap_or((
            vec![],
            Herramientas {
                lista: vec![],
                rutas: BTreeMap::new(),
            },
        ));
        let mut nuevos: Vec<Value> = vec![json!({"role": "user", "content": texto})];
        avisar(Evento::Inicio);
        let (mut texto_total, mut ve_imagenes) = (String::new(), true);
        let mut terminado = false;
        for _ in 0..VUELTAS {
            let sistema = format!(
                "{}{}{}",
                self.persona.lock().unwrap(),
                self.sentimientos.lock().unwrap(),
                hoy()
            );
            let mut mensajes = vec![json!({"role": "system", "content": sistema})];
            mensajes.extend(self.historial.lock().unwrap().iter().cloned());
            mensajes.extend(nuevos.iter().cloned());
            if !ve_imagenes {
                mensajes = sin_imagenes(&mensajes);
            }
            let (contenido, llamadas) = match self.una_vuelta(&mensajes, &herr.lista, &avisar, &token) {
                Ok(r) => r,
                Err(e) => {
                    let hay_imagenes = nuevos.iter().any(|m| m["content"].is_array());
                    if ve_imagenes && hay_imagenes && e.clave == "otro" {
                        ve_imagenes = false; // el modelo no ve imágenes: otra vez, sin ellas
                        for m in nuevos.iter_mut().filter(|m| m["content"].is_array()) {
                            m["content"] = json!(SIN_IMAGENES);
                        }
                        continue;
                    }
                    avisar(Evento::Error {
                        clave: e.clave,
                        mensaje: e.mensaje,
                    });
                    return;
                }
            };
            if token.load(Ordering::SeqCst) {
                return;
            }
            texto_total += &contenido;
            if llamadas.is_empty() {
                nuevos.push(json!({"role": "assistant", "content": contenido}));
                terminado = true;
                break;
            }
            nuevos.push(json!({"role": "assistant", "content": if contenido.is_empty() { Value::Null } else { json!(contenido) }, "tool_calls": llamadas}));
            let mut imagenes = vec![];
            for ll in &llamadas {
                let nombre = ll["function"]["name"].as_str().unwrap_or("");
                let completo = herr
                    .rutas
                    .get(nombre)
                    .map(|r| r.2.clone())
                    .unwrap_or_else(|| nombre.to_string());
                avisar(Evento::Herramienta(completo));
                let args = ll["function"]["arguments"]
                    .as_str()
                    .filter(|a| !a.is_empty())
                    .unwrap_or("{}");
                let (resultado, imgs) = self.correr(&servidores, &herr, nombre, args);
                if token.load(Ordering::SeqCst) {
                    return;
                }
                nuevos.push(json!({"role": "tool", "tool_call_id": ll["id"], "content": resultado}));
                imagenes.extend(imgs);
            }
            if !imagenes.is_empty() {
                let mut partes =
                    vec![json!({"type": "text", "text": "La imagen que pediste (captura o foto):"})];
                partes.extend(imagenes.iter().map(|(mime, datos)| json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{datos}")}})));
                nuevos.push(json!({"role": "user", "content": if ve_imagenes { json!(partes) } else { json!(SIN_IMAGENES) }}));
            }
        }
        if !terminado && texto_total.is_empty() {
            texto_total = "Me enredé con las herramientas y no terminé. ¿Me lo pides otra vez?".into();
            avisar(Evento::Texto(texto_total.clone()));
            avisar(Evento::BloqueFin);
        }
        if token.load(Ordering::SeqCst) {
            return;
        }
        {
            let mut h = self.historial.lock().unwrap();
            let mut todo = std::mem::take(&mut *h);
            todo.extend(sin_imagenes(&nuevos));
            *h = recortar(todo);
        }
        avisar(Evento::Listo(texto_total));
    }

    fn poner_sentimientos(&self, texto: &str) {
        *self.sentimientos.lock().unwrap() = texto.into();
    }
    fn poner_autorizar(&self, ruta: &str) {
        *self.autorizar.lock().unwrap() = Some(ruta.into());
    }
    fn poner_internet(&self, _si: bool) {} // la búsqueda web es de Claude: aquí no hay
    fn ocupado(&self) -> Arc<AtomicBool> {
        self.ocupado.clone()
    }
    fn agregar_persona(&self, texto: &str) {
        let mut p = self.persona.lock().unwrap();
        p.push_str("\n\n");
        p.push_str(texto);
    }
}
