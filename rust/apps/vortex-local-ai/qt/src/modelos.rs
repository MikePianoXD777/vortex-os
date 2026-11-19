//! `modelos`: el catálogo de Vortexine con «te cabe / va justo / no te cabe», bajar y borrar. Lo mismo que la
//! clase `Modelos` de `puente.py`: mismos nombres de propiedades, señales y slots.
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
        #[qproperty(QString, filtro, READ, WRITE = escribir_filtro, NOTIFY = filtro_cambio)]
        #[qproperty(QString, gpu, READ, NOTIFY = gpu_cambio)]
        #[qproperty(QVariant, tipos, READ, CONSTANT)]
        #[qproperty(QVariant, nombres_chat, cxx_name = "nombresChat", READ, NOTIFY = instalados_cambio)]
        #[qproperty(QVariant, nombres_embeddings, cxx_name = "nombresEmbeddings", READ, NOTIFY = instalados_cambio)]
        #[qproperty(bool, hay_instalados, cxx_name = "hayInstalados", READ, NOTIFY = instalados_cambio)]
        #[qproperty(QVariant, lista, READ, CONSTANT)]
        #[qproperty(QVariant, lista_instalados, cxx_name = "listaInstalados", READ, CONSTANT)]
        type Modelos = super::ModelosRust;

        #[qsignal]
        #[cxx_name = "instaladosCambio"]
        fn instalados_cambio(self: Pin<&mut Modelos>);
        #[qsignal]
        #[cxx_name = "filtroCambio"]
        fn filtro_cambio(self: Pin<&mut Modelos>);
        #[qsignal]
        #[cxx_name = "gpuCambio"]
        fn gpu_cambio(self: Pin<&mut Modelos>);

        #[cxx_name = "escribirFiltro"]
        fn escribir_filtro(self: Pin<&mut Modelos>, filtro: &QString);

        #[qinvokable]
        fn recargar(self: Pin<&mut Modelos>);
        #[qinvokable]
        fn bajar(self: Pin<&mut Modelos>, ident: &QString);
        #[qinvokable]
        fn cancelar(self: Pin<&mut Modelos>, ident: &QString);
        #[qinvokable]
        fn borrar(self: Pin<&mut Modelos>, nombre: &QString);
        /// Se llama al salir: corta las descargas que van.
        #[qinvokable]
        #[cxx_name = "cancelarTodo"]
        fn cancelar_todo(self: Pin<&mut Modelos>);
    }

    impl cxx_qt::Initialize for Modelos {}
    impl cxx_qt::Threading for Modelos {}
}

use crate::arranque::ffi::json_a_variante;
use crate::comun::{avisar, comun, en_chat, en_general, msg_ollama, registrar_modelos};
use crate::lista::Lista;
use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use vortex_local_ai::catalogo::{self, Medida, Vram};
use vortex_local_ai::http::Cancelar;
use vortex_local_ai::ollama::{ErrorOllama, Modelo};
use vortex_local_ai::texto;

struct Bajada {
    cancelar: Cancelar,
    progreso: f64,
    estado: String,
}

pub struct ModelosRust {
    filtro: QString,
    gpu: QString,
    tipos: QVariant,
    nombres_chat: QVariant,
    nombres_embeddings: QVariant,
    hay_instalados: bool,
    lista: QVariant,
    lista_instalados: QVariant,
    catalogo: Lista,
    instalados: Lista,
    vram: Option<Vram>,
    recomendado: String,
    nombres: Vec<String>,
    bajando: HashMap<String, Bajada>, // id -> la descarga que va
    cargando: bool,
}

impl Default for ModelosRust {
    fn default() -> Self {
        let catalogo = Lista::nueva(&[
            ("ident", json!("")),
            ("nombre", json!("")),
            ("descripcion", json!("")),
            ("para", json!("")),
            ("tipo", json!("")),
            ("gb", json!(0.0)),
            ("vram_gb", json!(0.0)),
            ("tamano", json!("")),
            ("cabe", json!("si")),
            ("instalado", json!(false)),
            ("bajando", json!(false)),
            ("progreso", json!(-1.0)),
            ("estado", json!("")),
            ("recomendado", json!(false)),
            ("contexto", json!("")),
            ("piensa", json!(false)),
        ]);
        let instalados = Lista::nueva(&[
            ("nombre", json!("")),
            ("bonito", json!("")),
            ("bytes", json!(0.0)),
            ("tamano", json!("")),
            ("detalles", json!("")),
            ("embeddings", json!(false)),
        ]);
        let mut tipos = vec![json!({"id": "todos", "nombre": "Todos"})];
        tipos.extend(
            catalogo::TIPOS
                .iter()
                .map(|(id, n)| json!({"id": id, "nombre": n})),
        );
        ModelosRust {
            filtro: QString::from("todos"),
            gpu: QString::default(),
            tipos: json_a_variante(&Value::Array(tipos).to_string()),
            nombres_chat: json_a_variante("[]"),
            nombres_embeddings: json_a_variante("[]"),
            hay_instalados: false,
            lista: catalogo.variante(),
            lista_instalados: instalados.variante(),
            catalogo,
            instalados,
            vram: None,
            recomendado: String::new(),
            nombres: Vec::new(),
            bajando: HashMap::new(),
            cargando: false,
        }
    }
}

impl cxx_qt::Initialize for qobject::Modelos {
    fn initialize(self: Pin<&mut Self>) {
        registrar_modelos(self.qt_thread());
    }
}

/// «NVIDIA GeForce RTX 3080» → «RTX 3080».
fn nombre_corto(nombre: &str) -> String {
    let mut n = nombre;
    for p in ["NVIDIA ", "AMD ", "Intel(R) "] {
        if let Some(r) = n.strip_prefix(p) {
            n = r;
            break;
        }
    }
    for p in ["GeForce ", "Radeon "] {
        if let Some(r) = n.strip_prefix(p) {
            n = r;
            break;
        }
    }
    n.trim().to_string()
}

fn capitalizar(t: &str) -> String {
    let mut c = t.chars();
    match c.next() {
        Some(p) => p.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

fn lista_de_textos(v: &[String]) -> QVariant {
    json_a_variante(&serde_json::to_string(v).unwrap_or_default())
}

impl qobject::Modelos {
    pub fn arrancar(self: Pin<&mut Self>) {
        // Sin saber la memoria de video no se puede decir «te cabe»: se busca en un hilo aparte.
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let v = catalogo::vram_disponible();
            let _ = hilo.queue(move |m| m.poner_vram(v));
        });
        self.recargar();
    }

    fn poner_vram(mut self: Pin<&mut Self>, vram: Vram) {
        let recomendado = catalogo::recomendado(&vram).id.to_string();
        let gb = vram.total_mb as f64 / 1024.0;
        let texto = if vram.de_entrada {
            format!("Tu gráfica se queda corta · usarás el procesador y tu RAM ({gb:.0} GB)")
        } else if vram.tipo == "cpu" {
            format!("Sin gráfica dedicada · usarás tu RAM ({gb:.0} GB)")
        } else {
            let nombre = nombre_corto(&vram.nombre);
            let nombre = if nombre.is_empty() { "sin nombre" } else { &nombre };
            format!("Tu gráfica: {nombre} · {gb:.0} GB")
        };
        comun().compartido.lock().unwrap().recomendado = recomendado.clone();
        {
            let mut r = self.as_mut().rust_mut();
            r.vram = Some(vram);
            r.recomendado = recomendado;
            r.gpu = QString::from(texto.as_str());
        }
        self.as_mut().gpu_cambio();
        let corto = texto.replace("Tu gráfica: ", "");
        en_general(move |g| g.poner_gpu(&corto));
        self.llenar_catalogo();
    }

    fn instalado(&self, ident: &str) -> bool {
        let n = &self.rust().nombres;
        n.iter().any(|x| x == ident) || (!ident.contains(':') && n.contains(&format!("{ident}:latest")))
    }

    fn llenar_catalogo(self: Pin<&mut Self>) {
        let filtro = self.rust().filtro.to_string();
        let mut filas: Vec<(bool, u8, Value)> = Vec::new();
        for m in catalogo::todos() {
            if filtro != "todos" && filtro != m.para {
                continue;
            }
            let cabe = self
                .rust()
                .vram
                .as_ref()
                .map_or("si", |v| catalogo::cabe(Medida::Entrada(m), v));
            let b = self.rust().bajando.get(m.id);
            let recomendado = m.id == self.rust().recomendado;
            let tipo = catalogo::nombre_de_tipo(m.para)
                .map(str::to_string)
                .unwrap_or_else(|| capitalizar(m.para));
            let orden = match cabe {
                "si" => 0,
                "justo" => 1,
                "no" => 2,
                _ => 3,
            };
            filas.push((
                !recomendado,
                orden,
                json!({
                    "ident": m.id, "nombre": m.nombre, "descripcion": m.descripcion, "para": m.para,
                    "tipo": tipo, "gb": m.gb, "vram_gb": m.vram_gb, "tamano": format!("{:.1} GB", m.gb),
                    "cabe": cabe, "instalado": self.instalado(m.id), "bajando": b.is_some(),
                    "progreso": b.map_or(-1.0, |b| b.progreso),
                    "estado": b.map_or("", |b| b.estado.as_str()),
                    "recomendado": recomendado, "contexto": m.contexto, "piensa": m.piensa,
                }),
            ));
        }
        filas.sort_by_key(|f| (f.0, f.1));
        self.rust()
            .catalogo
            .poner(filas.into_iter().map(|f| f.2).collect());
    }

    pub fn recargar(mut self: Pin<&mut Self>) {
        if self.rust().cargando {
            return;
        }
        self.as_mut().rust_mut().cargando = true;
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let r = comun().ollama.modelos();
            let _ = hilo.queue(move |m| m.recargado(r));
        });
    }

    fn recargado(mut self: Pin<&mut Self>, r: Result<Vec<Modelo>, ErrorOllama>) {
        self.as_mut().rust_mut().cargando = false;
        match r {
            Ok(mut modelos) => {
                modelos.sort_by(|a, b| a.nombre.cmp(&b.nombre));
                let filas: Vec<Value> = modelos
                    .iter()
                    .map(|m| {
                        let del_catalogo = catalogo::todos()
                            .find(|c| c.id == m.nombre || format!("{}:latest", c.id) == m.nombre);
                        let detalles = [&m.familia, &m.parametros, &m.cuantizacion]
                            .iter()
                            .filter(|x| !x.is_empty())
                            .map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(" · ");
                        json!({
                            "nombre": m.nombre, "bonito": del_catalogo.map_or("", |c| c.nombre),
                            "bytes": m.bytes as f64, "tamano": texto::tamano(m.bytes as f64),
                            "detalles": detalles, "embeddings": m.embeddings,
                        })
                    })
                    .collect();
                self.rust().instalados.poner(filas);
                self.as_mut().rust_mut().nombres = modelos.iter().map(|m| m.nombre.clone()).collect();
                self.as_mut().llenar_catalogo();
                self.avisar_instalados();
            }
            Err(_) => {
                if !self.rust().nombres.is_empty() {
                    self.rust().instalados.poner(vec![]);
                    self.as_mut().rust_mut().nombres.clear();
                    self.as_mut().llenar_catalogo();
                    self.avisar_instalados();
                }
            }
        }
    }

    /// Los modelos instalados cambiaron: se actualizan las propiedades y se avisa (a QML y al chat).
    fn avisar_instalados(mut self: Pin<&mut Self>) {
        let filas = self.rust().instalados.filas();
        let nombres = |emb: bool| -> Vec<String> {
            filas
                .iter()
                .filter(|f| f.get("embeddings").and_then(Value::as_bool).unwrap_or(false) == emb)
                .map(|f| f.get("nombre").and_then(Value::as_str).unwrap_or("").to_string())
                .collect()
        };
        let (chat, emb) = (nombres(false), nombres(true));
        comun().compartido.lock().unwrap().nombres_chat = chat.clone();
        {
            let mut r = self.as_mut().rust_mut();
            r.hay_instalados = !chat.is_empty();
            r.nombres_chat = lista_de_textos(&chat);
            r.nombres_embeddings = lista_de_textos(&emb);
        }
        self.as_mut().instalados_cambio();
        en_chat(|c| c.revisar_modelo());
    }

    fn bajar(mut self: Pin<&mut Self>, ident: &QString) {
        let id = ident.to_string();
        if self.rust().bajando.contains_key(&id) {
            return;
        }
        let cancelar = Cancelar::new();
        self.as_mut().rust_mut().bajando.insert(
            id.clone(),
            Bajada {
                cancelar: cancelar.clone(),
                progreso: -1.0,
                estado: "Empezando…".into(),
            },
        );
        self.as_mut().marcar(&id);
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let o = comun().ollama.clone();
            // Cuándo y con qué fracción se avisó la última vez: no se avisa más de ~6 veces por segundo.
            let mut ultimo: (Option<Instant>, f64) = (None, -1.0);
            let (h2, id2) = (hilo.clone(), id.clone());
            let mut al_progreso = move |completado: i64, total: i64, estado: &str| {
                let fraccion = if total != 0 {
                    completado as f64 / total as f64
                } else {
                    -1.0
                };
                let ahora = Instant::now();
                let toca = ultimo
                    .0
                    .is_none_or(|t| ahora.duration_since(t) >= Duration::from_millis(150))
                    || (fraccion - ultimo.1).abs() >= 0.01
                    || estado == "Listo";
                if toca {
                    ultimo = (Some(ahora), fraccion);
                    let (id, estado) = (id2.clone(), estado.to_string());
                    let _ = h2.queue(move |m| m.al_progreso(&id, fraccion, &estado));
                }
            };
            let r = if !o.disponible() && !o.asegurar_servidor(Duration::from_secs(20)) {
                Err(ErrorOllama::otro(
                    "Ollama no está corriendo y no lo pude arrancar. Ve a Ajustes y toca «Arrancar».",
                ))
            } else {
                o.bajar(&id, Some(&mut al_progreso), Some(&cancelar))
            };
            let _ = hilo.queue(move |m| m.bajado(&id, r));
        });
    }

    fn bajado(mut self: Pin<&mut Self>, id: &str, r: Result<(), ErrorOllama>) {
        self.as_mut().rust_mut().bajando.remove(id);
        match r {
            Ok(()) => {
                let nombre = catalogo::por_id(id).map_or(id, |e| e.nombre);
                avisar(format!("Listo: {nombre} ya está instalado."), "exito");
                self.as_mut().marcar(id);
                self.recargar();
            }
            Err(e) => {
                self.as_mut().marcar(id);
                if e.es_cancelado() {
                    avisar(
                        "Cancelé la descarga. Lo que ya bajó se retoma la próxima vez.",
                        "info",
                    );
                } else {
                    avisar(msg_ollama(&e), "error");
                }
            }
        }
    }

    fn cancelar(mut self: Pin<&mut Self>, ident: &QString) {
        let id = ident.to_string();
        if let Some(b) = self.as_mut().rust_mut().bajando.get_mut(&id) {
            b.cancelar.set();
            b.estado = "Cancelando…".into();
            self.marcar(&id);
        }
    }

    fn cancelar_todo(self: Pin<&mut Self>) {
        for b in self.rust().bajando.values() {
            b.cancelar.set();
        }
    }

    fn al_progreso(mut self: Pin<&mut Self>, id: &str, fraccion: f64, estado: &str) {
        if let Some(b) = self.as_mut().rust_mut().bajando.get_mut(id)
            && !b.cancelar.is_set()
        {
            b.progreso = fraccion;
            b.estado = estado.to_string();
            self.marcar(id);
        }
    }

    fn marcar(self: Pin<&mut Self>, id: &str) {
        let i = self.rust().catalogo.buscar("ident", &json!(id));
        let b = self.rust().bajando.get(id);
        let campos = json!({
            "bajando": b.is_some(), "progreso": b.map_or(-1.0, |b| b.progreso),
            "estado": b.map_or("", |b| b.estado.as_str()), "instalado": self.instalado(id),
        });
        self.rust().catalogo.cambiar(i, campos);
    }

    fn borrar(self: Pin<&mut Self>, nombre: &QString) {
        let nombre = nombre.to_string();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let r = comun().ollama.borrar(&nombre);
            let _ = hilo.queue(move |m| match r {
                Ok(()) => {
                    avisar(
                        format!("Borré {nombre}. Lo puedes volver a bajar cuando quieras."),
                        "exito",
                    );
                    m.recargar();
                }
                Err(e) => avisar(msg_ollama(&e), "error"),
            });
        });
    }

    fn escribir_filtro(mut self: Pin<&mut Self>, filtro: &QString) {
        if self.rust().filtro != *filtro {
            self.as_mut().rust_mut().filtro = filtro.clone();
            self.as_mut().filtro_cambio();
            self.llenar_catalogo();
        }
    }
}
