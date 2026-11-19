//! `chat`: las conversaciones y los mensajes, que llegan mientras el modelo escribe. Lo mismo que la clase
//! `Chat` de `puente.py`: mismos nombres de propiedades, señales y slots.
//!
//! El modelo contesta en un hilo aparte; los trozos se juntan y se pintan cada 60 ms, para que la ventana no se
//! trabe aunque escriba muy rápido.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, actual, READ, NOTIFY = actual_cambio)]
        #[qproperty(QString, titulo, READ, NOTIFY = actual_cambio)]
        #[qproperty(QString, modelo, READ, WRITE = escribir_modelo, NOTIFY = modelo_cambio)]
        #[qproperty(bool, generando, READ, NOTIFY = generando_cambio)]
        #[qproperty(bool, usar_vault, cxx_name = "usarVault", READ, WRITE = escribir_usar_vault, NOTIFY = usar_vault_cambio)]
        #[qproperty(QVariant, lista, READ, CONSTANT)]
        #[qproperty(QVariant, lista_mensajes, cxx_name = "listaMensajes", READ, CONSTANT)]
        type Chat = super::ChatRust;

        #[qsignal]
        #[cxx_name = "actualCambio"]
        fn actual_cambio(self: Pin<&mut Chat>);
        #[qsignal]
        #[cxx_name = "modeloCambio"]
        fn modelo_cambio(self: Pin<&mut Chat>);
        #[qsignal]
        #[cxx_name = "generandoCambio"]
        fn generando_cambio(self: Pin<&mut Chat>);
        #[qsignal]
        #[cxx_name = "usarVaultCambio"]
        fn usar_vault_cambio(self: Pin<&mut Chat>);

        #[cxx_name = "escribirModelo"]
        fn escribir_modelo(self: Pin<&mut Chat>, nombre: &QString);
        #[cxx_name = "escribirUsarVault"]
        fn escribir_usar_vault(self: Pin<&mut Chat>, valor: bool);

        #[qinvokable]
        fn nuevo(self: Pin<&mut Chat>);
        #[qinvokable]
        fn abrir(self: Pin<&mut Chat>, ident: &QString);
        #[qinvokable]
        fn borrar(self: Pin<&mut Chat>, ident: &QString);
        #[qinvokable]
        fn enviar(self: Pin<&mut Chat>, texto: &QString);
        /// Vuelve a pedir la última respuesta (con el modelo elegido ahora).
        #[qinvokable]
        fn regenerar(self: Pin<&mut Chat>);
        #[qinvokable]
        fn detener(self: Pin<&mut Chat>);
        #[qinvokable]
        #[cxx_name = "guardarEnVault"]
        fn guardar_en_vault(self: Pin<&mut Chat>);
        #[qinvokable]
        #[cxx_name = "recargarLista"]
        fn recargar_lista(self: Pin<&mut Chat>);
    }

    impl cxx_qt::Initialize for Chat {}
    impl cxx_qt::Threading for Chat {}
}

use crate::comun::{
    avisar, comun, en_boveda, en_general, msg_io, msg_ollama, msg_vault, nueva_memoria, registrar_chat,
};
use crate::lista::Lista;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_local_ai::conversaciones;
use vortex_local_ai::http::Cancelar;
use vortex_local_ai::memoria::Fragmento;
use vortex_local_ai::ollama::Estadisticas;
use vortex_local_ai::texto;
use vortex_local_ai::tiempo;
use vortex_local_ai::vault::Vault;

pub struct ChatRust {
    actual: QString,
    titulo: QString,
    modelo: QString,
    generando: bool,
    usar_vault: bool,
    lista: QVariant,
    lista_mensajes: QVariant,
    conversaciones: Lista,
    mensajes: Lista,
    conv: Value,
    cancelar: Cancelar,
    /// Respuesta y pensamiento que llegaron y todavía no se pintan.
    trozos: Arc<Mutex<(String, String)>>,
    /// Para parar el reloj de 60 ms que pinta los trozos.
    pintando: Option<Arc<AtomicBool>>,
}

impl Default for ChatRust {
    fn default() -> Self {
        let c = comun();
        let (modelo, usar_vault) = {
            let cfg = c.config.lock().unwrap();
            (cfg.texto("modelo"), cfg.booleano("usar_vault", false))
        };
        let conversaciones = Lista::nueva(&[
            ("clave", json!("")),
            ("titulo", json!("")),
            ("fecha", json!("")),
            ("vault", json!(false)),
        ]);
        let mensajes = Lista::nueva(&[
            ("rol", json!("")),
            ("contenido", json!("")),
            ("pensamiento", json!("")),
            ("citas", json!([])),
            ("error", json!("")),
            ("escribiendo", json!(false)),
            ("pensando", json!(false)),
            ("modelo", json!("")),
            ("datos", json!("")),
        ]);
        let conv = conversaciones::nueva(&modelo, "");
        ChatRust {
            actual: QString::from(conv["id"].as_str().unwrap_or("")),
            titulo: QString::default(),
            modelo: QString::from(modelo.as_str()),
            generando: false,
            usar_vault,
            lista: conversaciones.variante(),
            lista_mensajes: mensajes.variante(),
            conversaciones,
            mensajes,
            conv,
            cancelar: Cancelar::new(),
            trozos: Arc::new(Mutex::new((String::new(), String::new()))),
            pintando: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::Chat {
    fn initialize(self: Pin<&mut Self>) {
        registrar_chat(self.qt_thread());
    }
}

fn texto_de<'a>(v: &'a Value, clave: &str) -> &'a str {
    v.get(clave).and_then(Value::as_str).unwrap_or("")
}

fn stem(ruta: &str) -> String {
    Path::new(ruta)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Las notas citadas, sin repetir: `[{"ruta", "titulo"}]`. Acepta rutas sueltas o `{ruta, titulo}`.
fn citas_de(rutas: &Value) -> Vec<Value> {
    let mut vistas = HashSet::new();
    let mut salida = Vec::new();
    for r in rutas.as_array().into_iter().flatten() {
        let ruta = match r {
            Value::Object(_) => texto_de(r, "ruta").to_string(),
            Value::String(s) => s.clone(),
            otro => otro.to_string(),
        };
        if ruta.is_empty() || !vistas.insert(ruta.clone()) {
            continue;
        }
        let titulo = match r {
            Value::Object(_) if !texto_de(r, "titulo").is_empty() => texto_de(r, "titulo").to_string(),
            _ => stem(&ruta),
        };
        salida.push(json!({"ruta": ruta, "titulo": titulo}));
    }
    salida
}

fn citas_de_fragmentos(f: &[Fragmento]) -> Vec<Value> {
    citas_de(&Value::Array(
        f.iter()
            .map(|x| json!({"ruta": x.ruta, "titulo": x.titulo}))
            .collect(),
    ))
}

/// `{"role", "content"}` de los mensajes del usuario y de la IA que tienen texto.
fn historial_de(mensajes: &[Value]) -> Vec<Value> {
    mensajes
        .iter()
        .filter(|m| matches!(texto_de(m, "role"), "user" | "assistant") && !texto_de(m, "content").is_empty())
        .map(|m| json!({"role": texto_de(m, "role"), "content": texto_de(m, "content")}))
        .collect()
}

impl qobject::Chat {
    pub fn arrancar(self: Pin<&mut Self>) {
        self.recargar_lista();
    }

    fn recargar_lista(self: Pin<&mut Self>) {
        let convs = conversaciones::lista(&comun().rutas.chats());
        let filas = convs
            .iter()
            .map(|c| {
                let titulo = texto_de(c, "titulo");
                json!({
                    "clave": texto_de(c, "id"),
                    "titulo": if titulo.is_empty() { "Chat nuevo" } else { titulo },
                    "fecha": texto::fecha_corta(c.get("modificado").and_then(Value::as_f64).unwrap_or(0.0)),
                    "vault": c.get("vault").is_some_and(|v| v.as_bool().unwrap_or(false)),
                })
            })
            .collect();
        self.rust().conversaciones.poner(filas);
    }

    /// Si el modelo del chat ya no está instalado, se elige otro (el recomendado, o el primero).
    pub fn revisar_modelo(mut self: Pin<&mut Self>) {
        let (nombres, recomendado) = {
            let c = comun().compartido.lock().unwrap();
            (c.nombres_chat.clone(), c.recomendado.clone())
        };
        let actual = self.rust().modelo.to_string();
        if !nombres.is_empty() && !nombres.contains(&actual) {
            let elegido = nombres
                .iter()
                .find(|n| **n == recomendado || n.starts_with(&format!("{recomendado}:")))
                .unwrap_or(&nombres[0])
                .clone();
            self.poner_modelo(&elegido);
        } else if nombres.is_empty() && !actual.is_empty() {
            // Sin modelos: el chat pide bajar uno (la preferencia guardada se queda).
            self.as_mut().rust_mut().modelo = QString::default();
            self.as_mut().modelo_cambio();
        }
    }

    fn poner_modelo(mut self: Pin<&mut Self>, nombre: &str) {
        if self.rust().modelo.to_string() != nombre {
            self.as_mut().rust_mut().modelo = QString::from(nombre);
            comun().config.lock().unwrap().poner("modelo", json!(nombre));
            self.as_mut().modelo_cambio();
        }
    }

    fn escribir_modelo(self: Pin<&mut Self>, nombre: &QString) {
        self.poner_modelo(&nombre.to_string());
    }

    fn poner_usar_vault(mut self: Pin<&mut Self>, valor: bool) {
        if self.rust().usar_vault != valor {
            self.as_mut().rust_mut().usar_vault = valor;
            comun().config.lock().unwrap().poner("usar_vault", json!(valor));
            self.as_mut().usar_vault_cambio();
        }
    }

    fn escribir_usar_vault(self: Pin<&mut Self>, valor: bool) {
        self.poner_usar_vault(valor);
    }

    fn poner_generando(mut self: Pin<&mut Self>, valor: bool) {
        if self.rust().generando != valor {
            self.as_mut().rust_mut().generando = valor;
            self.as_mut().generando_cambio();
        }
    }

    /// `actual` y `titulo` salen de la conversación; se avisa a QML.
    fn sincronizar(mut self: Pin<&mut Self>) {
        let (id, titulo) = {
            let c = &self.rust().conv;
            (texto_de(c, "id").to_string(), texto_de(c, "titulo").to_string())
        };
        {
            let mut r = self.as_mut().rust_mut();
            r.actual = QString::from(id.as_str());
            r.titulo = QString::from(titulo.as_str());
        }
        self.as_mut().actual_cambio();
    }

    // --- conversaciones ---

    fn nuevo(mut self: Pin<&mut Self>) {
        if self.rust().generando {
            self.as_mut().detener();
        }
        let modelo = self.rust().modelo.to_string();
        self.as_mut().rust_mut().conv = conversaciones::nueva(&modelo, "");
        self.rust().mensajes.poner(vec![]);
        self.sincronizar();
    }

    fn abrir(mut self: Pin<&mut Self>, ident: &QString) {
        if self.rust().generando {
            self.as_mut().detener();
        }
        let id = ident.to_string();
        let Some(conv) = conversaciones::abrir(&comun().rutas.chats(), &id) else {
            avisar("No encontré ese chat.", "error");
            self.recargar_lista();
            return;
        };
        let filas: Vec<Value> = conv["mensajes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| matches!(texto_de(m, "role"), "user" | "assistant"))
            .map(|m| {
                json!({
                    "rol": texto_de(m, "role"), "contenido": texto_de(m, "content"),
                    "pensamiento": texto_de(m, "pensamiento"),
                    "citas": citas_de(m.get("citas").unwrap_or(&Value::Null)),
                    "modelo": texto_de(m, "modelo"),
                })
            })
            .collect();
        self.rust().mensajes.poner(filas);
        let modelo = texto_de(&conv, "modelo").to_string();
        let usar_vault = match conv.get("vault") {
            Some(Value::Bool(b)) => *b,
            Some(Value::Null) | None => self.rust().usar_vault,
            Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0),
            Some(Value::String(s)) => !s.is_empty(),
            Some(_) => true,
        };
        self.as_mut().rust_mut().conv = conv;
        if comun().compartido.lock().unwrap().nombres_chat.contains(&modelo) {
            self.as_mut().poner_modelo(&modelo);
        }
        self.as_mut().poner_usar_vault(usar_vault);
        self.sincronizar();
    }

    fn borrar(mut self: Pin<&mut Self>, ident: &QString) {
        let id = ident.to_string();
        if let Err(e) = conversaciones::borrar(&comun().rutas.chats(), &id) {
            avisar(e, "error");
        }
        if texto_de(&self.rust().conv, "id") == id {
            self.as_mut().nuevo();
        }
        self.recargar_lista();
    }

    fn guardar(mut self: Pin<&mut Self>) {
        let vacio = self.rust().conv["mensajes"].as_array().is_none_or(Vec::is_empty);
        if vacio {
            return;
        }
        if texto_de(&self.rust().conv, "titulo").is_empty() {
            let primero = self.rust().conv["mensajes"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|m| texto_de(m, "role") == "user")
                .map_or("", |m| texto_de(m, "content"))
                .to_string();
            self.as_mut().rust_mut().conv["titulo"] = json!(conversaciones::titulo_automatico(&primero));
        }
        let carpeta = comun().rutas.chats();
        let r = conversaciones::guardar(&carpeta, &mut self.as_mut().rust_mut().conv);
        match r {
            Ok(id) => {
                self.as_mut().rust_mut().conv["id"] = json!(id);
                self.as_mut().recargar_lista();
                self.sincronizar();
            }
            Err(e) => avisar(format!("No pude guardar el chat: {}", msg_io(&e)), "error"),
        }
    }

    // --- mandar y recibir ---

    fn enviar(mut self: Pin<&mut Self>, texto: &QString) {
        let texto = texto.to_string().trim().to_string();
        if texto.is_empty() || self.rust().generando {
            return;
        }
        if self.rust().modelo.is_empty() {
            avisar("Primero baja un modelo: te llevo a Modelos.", "info");
            en_general(|g| g.ir_a(&QString::from("modelos")));
            return;
        }
        let historial = historial_de(self.rust().conv["mensajes"].as_array().map_or(&[], Vec::as_slice));
        let modelo = self.rust().modelo.to_string();
        let usar_vault = self.rust().usar_vault;
        {
            let conv = &mut self.as_mut().rust_mut().conv;
            if !conv["mensajes"].is_array() {
                conv["mensajes"] = json!([]);
            }
            conv["mensajes"].as_array_mut().unwrap().push(json!({
                "role": "user", "content": texto, "fecha": tiempo::ahora(),
            }));
            conv["modelo"] = json!(modelo);
            if usar_vault {
                conv["vault"] = json!(true);
            }
        }
        self.rust()
            .mensajes
            .agregar(json!({"rol": "user", "contenido": texto}));
        self.generar(historial, texto);
    }

    fn regenerar(mut self: Pin<&mut Self>) {
        if self.rust().generando {
            return;
        }
        let (historial, pregunta) = {
            let conv = &mut self.as_mut().rust_mut().conv;
            let Some(msgs) = conv["mensajes"].as_array_mut() else {
                return;
            };
            let Some(i_user) = msgs.iter().rposition(|m| texto_de(m, "role") == "user") else {
                return;
            };
            let pregunta = texto_de(&msgs[i_user], "content").to_string();
            msgs.truncate(i_user + 1);
            (historial_de(&msgs[..i_user]), pregunta)
        };
        let filas = self.rust().mensajes.filas();
        let ultima_user = filas
            .iter()
            .rposition(|f| f.get("rol").and_then(Value::as_str) == Some("user"))
            .map_or(-1, |i| i as i32);
        self.rust().mensajes.quitar_desde(ultima_user + 1);
        self.generar(historial, pregunta);
    }

    fn detener(self: Pin<&mut Self>) {
        self.rust().cancelar.set();
    }

    fn generar(mut self: Pin<&mut Self>, historial: Vec<Value>, pregunta: String) {
        let modelo = self.rust().modelo.to_string();
        let usar_vault = self.rust().usar_vault;
        let cancelar = Cancelar::new();
        let trozos = self.rust().trozos.clone();
        {
            let mut r = self.as_mut().rust_mut();
            r.cancelar = cancelar.clone();
            *r.trozos.lock().unwrap() = (String::new(), String::new());
        }
        self.rust()
            .mensajes
            .agregar(json!({"rol": "assistant", "escribiendo": true, "modelo": modelo}));
        let fila = self.rust().mensajes.cuenta() - 1;
        self.as_mut().poner_generando(true);
        // El reloj de 60 ms que pinta lo que va llegando.
        let parar = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().pintando = Some(parar.clone());
        let hilo = self.qt_thread();
        let reloj = hilo.clone();
        std::thread::spawn(move || {
            while !parar.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(60));
                if parar.load(Ordering::SeqCst) || reloj.queue(|c| c.volcar()).is_err() {
                    break;
                }
            }
        });

        std::thread::spawn(move || {
            let o = comun().ollama.clone();
            let mut citas: Vec<Value> = Vec::new();
            let mut aviso = String::new();
            let r: Result<(String, Estadisticas), String> = (|| {
                let mensajes: Vec<Value> = if usar_vault {
                    let mut mem = nueva_memoria().map_err(|e| msg_vault(&e))?;
                    let m = mem
                        .mensajes_con_contexto(&pregunta, &historial)
                        .map_err(|e| msg_ollama(&e))?;
                    citas = citas_de_fragmentos(&mem.ultimo_contexto);
                    aviso = mem.aviso();
                    let c = citas.clone();
                    let _ = hilo.queue(move |ch| ch.rust().mensajes.cambiar(fila, json!({"citas": c})));
                    m
                } else {
                    let mut m = historial.clone();
                    m.push(json!({"role": "user", "content": pregunta}));
                    m
                };
                let (t1, t2) = (trozos.clone(), trozos.clone());
                let mut al_trozo = move |t: &str| t1.lock().unwrap().0.push_str(t);
                let mut al_pensar = move |t: &str| t2.lock().unwrap().1.push_str(t);
                let mut res = o.chat(
                    &modelo,
                    &mensajes,
                    Some(&mut al_trozo),
                    Some(&cancelar),
                    None,
                    Some(&mut al_pensar),
                );
                // Ollama se cayó: se intenta levantar y se vuelve a pedir una vez.
                if res.as_ref().is_err_and(|e| e.clave == "conexion")
                    && o.asegurar_servidor(Duration::from_secs(20))
                {
                    en_general(|g| g.revisar_ollama(false));
                    res = o.chat(
                        &modelo,
                        &mensajes,
                        Some(&mut al_trozo),
                        Some(&cancelar),
                        None,
                        Some(&mut al_pensar),
                    );
                }
                res.map(|r| (r, o.ultimas_estadisticas()))
                    .map_err(|e| msg_ollama(&e))
            })();
            let cancelada = cancelar.is_set();
            let _ = hilo.queue(move |c| c.terminar(fila, modelo, cancelada, r, citas, aviso));
        });
    }

    /// Pinta lo que llegó desde la última vez.
    fn volcar(self: Pin<&mut Self>) {
        let (respuesta, pensamiento) = {
            let mut t = self.rust().trozos.lock().unwrap();
            std::mem::take(&mut *t)
        };
        if respuesta.is_empty() && pensamiento.is_empty() {
            return;
        }
        let fila = self.rust().mensajes.cuenta() - 1;
        let actual = self.rust().mensajes.fila(fila);
        let contenido = actual
            .get("contenido")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let pensado = actual
            .get("pensamiento")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let pensando = !pensamiento.is_empty() && contenido.is_empty() && respuesta.is_empty();
        self.rust().mensajes.cambiar(
            fila,
            json!({
                "contenido": contenido + &respuesta, "pensamiento": pensado + &pensamiento, "pensando": pensando,
            }),
        );
    }

    fn terminar(
        mut self: Pin<&mut Self>,
        fila: i32,
        modelo: String,
        cancelada: bool,
        r: Result<(String, Estadisticas), String>,
        citas: Vec<Value>,
        aviso: String,
    ) {
        if let Some(p) = self.as_mut().rust_mut().pintando.take() {
            p.store(true, Ordering::SeqCst);
        }
        self.as_mut().volcar();
        let actual = self.rust().mensajes.fila(fila);
        let (error, respuesta, datos) = match r {
            Ok((respuesta, datos)) => (String::new(), respuesta, datos),
            Err(e) => (e, String::new(), Estadisticas::default()),
        };
        let exito = error.is_empty();
        let contenido = if respuesta.is_empty() {
            actual
                .get("contenido")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        } else {
            respuesta
        };
        let mut texto_datos = String::new();
        if exito && datos.tokens_por_segundo != 0.0 {
            texto_datos = format!("{:.0} palabras/s", datos.tokens_por_segundo); // tokens, pero así se entiende
        }
        if cancelada && error.is_empty() {
            texto_datos = if texto_datos.is_empty() {
                "Detenido".into()
            } else {
                format!("Detenido · {texto_datos}")
            };
        }
        let citas_nuevas = citas_de(&Value::Array(if exito { citas } else { vec![] }));
        let citas_fila = if citas_nuevas.is_empty() {
            actual.get("citas").cloned().unwrap_or(json!([]))
        } else {
            Value::Array(citas_nuevas)
        };
        self.rust().mensajes.cambiar(
            fila,
            json!({
                "contenido": contenido, "escribiendo": false, "pensando": false, "error": error,
                "datos": texto_datos, "citas": citas_fila,
            }),
        );
        let pensamiento = actual.get("pensamiento").and_then(Value::as_str).unwrap_or("");
        if !contenido.is_empty() || !pensamiento.is_empty() {
            let ahora = self.rust().mensajes.fila(fila);
            let rutas: Vec<Value> = ahora
                .get("citas")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| json!(texto_de(c, "ruta")))
                .collect();
            let conv = &mut self.as_mut().rust_mut().conv;
            if !conv["mensajes"].is_array() {
                conv["mensajes"] = json!([]);
            }
            conv["mensajes"].as_array_mut().unwrap().push(json!({
                "role": "assistant", "content": contenido,
                "pensamiento": ahora.get("pensamiento").and_then(Value::as_str).unwrap_or(""),
                "citas": rutas, "modelo": modelo, "fecha": tiempo::ahora(),
            }));
        }
        self.as_mut().poner_generando(false);
        if !aviso.is_empty() {
            avisar(aviso, "info");
        }
        self.guardar();
    }

    // --- al vault ---

    fn guardar_en_vault(mut self: Pin<&mut Self>) {
        let vacio = self.rust().conv["mensajes"].as_array().is_none_or(Vec::is_empty);
        if vacio {
            avisar("Este chat está vacío: escribe algo primero.", "info");
            return;
        }
        if self.rust().generando {
            avisar("Espera a que termine de escribir.", "info");
            return;
        }
        self.as_mut().guardar();
        let conv = &self.rust().conv;
        let titulo = match texto_de(conv, "titulo") {
            "" => "Chat",
            t => t,
        }
        .to_string();
        let nota = conversaciones::a_nota(conv);
        let raiz = comun().compartido.lock().unwrap().raiz.clone();
        let r = Vault::new(&raiz).and_then(|v| {
            if !v.existe("Chats") {
                let _ = v.crear_carpeta("Chats");
            }
            let ruta = v.crear(&titulo, "Chats")?;
            v.guardar(&ruta, &nota)?;
            Ok(ruta)
        });
        match r {
            Ok(ruta) => {
                en_boveda(|mut b| {
                    b.as_mut().recargar();
                    b.pedir_grafo();
                });
                avisar(format!("Guardé el chat en tu vault: {ruta}"), "exito");
            }
            Err(e) => avisar(
                format!("No pude guardarlo en el vault: {}", msg_vault(&e)),
                "error",
            ),
        }
    }
}

#[allow(dead_code)]
type _Fila = Map<String, Value>;
