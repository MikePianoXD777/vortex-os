//! `vs`: lo mismo que `puente.py`, envolviendo el motor de `vortex_shared` (ya probado).
//! Mismos nombres de propiedades, señales y slots, para que los QML de la app de Python no cambien.
//!
//! Lo lento (preparar, mandar, recibir, preguntar a Tailscale) corre en hilos y vuelve a la ventana con
//! `qt_thread().queue`.
#![allow(unsafe_code)] // el código que genera cxx-qt

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qvariant.h");
        type QVariant = cxx_qt_lib::QVariant;
        include!("cxx-qt-lib/qlist.h");
        type QList_QVariant = cxx_qt_lib::QList<cxx_qt_lib::QVariant>;
    }

    unsafe extern "RustQt" {
        #[qobject]
        #[qproperty(QString, usuario)]
        #[qproperty(QString, estado)]
        #[qproperty(f64, progreso)]
        #[qproperty(bool, ocupado)]
        #[qproperty(QString, tono)]
        #[qproperty(QString, codigo)]
        #[qproperty(QList_QVariant, archivos)]
        #[qproperty(QString, total)]
        #[qproperty(QList_QVariant, direcciones)]
        #[qproperty(QList_QVariant, compus)]
        #[qproperty(bool, puerto_cerrado, cxx_name = "puertoCerrado")]
        #[qproperty(QList_QVariant, recientes)]
        type Puente = super::PuenteRust;

        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);

        #[qinvokable]
        #[cxx_name = "actualizarRed"]
        fn actualizar_red(self: Pin<&mut Puente>);
        #[qinvokable]
        fn agregar(self: Pin<&mut Puente>, urls: &QList_QVariant);
        #[qinvokable]
        fn quitar(self: Pin<&mut Puente>, indice: i32);
        #[qinvokable]
        fn mandar(self: Pin<&mut Puente>, host: &QString, usuario: &QString);
        #[qinvokable]
        #[cxx_name = "crearCodigo"]
        fn crear_codigo(self: Pin<&mut Puente>);
        #[qinvokable]
        fn cancelar(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "recibirCodigo"]
        fn recibir_codigo(self: Pin<&mut Puente>, codigo: &QString);
        #[qinvokable]
        #[cxx_name = "abrirPuerto"]
        fn abrir_puerto(self: Pin<&mut Puente>);
        #[qinvokable]
        fn copiar(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: Pin<&mut Puente>);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use core::pin::Pin;
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QList, QMap, QMapPair_QString_QVariant, QString, QVariant};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_shared::motor::{self, Envio, ErrorShared};
use vortex_shared::{Config, instancia};

const MAX_RECIENTES: usize = 5;

/// El envío que sostiene un código (sigue mandando hasta que se cancela o se cierra la ventana).
static ENVIO_CODIGO: Mutex<Option<Envio>> = Mutex::new(None);
/// Los archivos que trae la línea de comandos (clic derecho de Thunar).
static ARCHIVOS_INICIALES: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn poner_archivos_iniciales(archivos: Vec<String>) {
    *ARCHIVOS_INICIALES.lock().unwrap() = archivos;
}

/// Al cerrar la ventana: si había un código, corta el sendme (y limpia lo que dejó).
pub fn cerrar() {
    let envio = ENVIO_CODIGO.lock().unwrap().take();
    if let Some(mut e) = envio {
        e.terminar();
    }
}

#[derive(Clone)]
struct Archivo {
    ruta: String,
    nombre: String,
    tam: u64,
    texto: String,
    carpeta: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Reciente {
    host: String,
    usuario: String,
}

pub struct PuenteRust {
    usuario: QString,
    estado: QString,
    progreso: f64,
    ocupado: bool,
    tono: QString,
    codigo: QString,
    archivos: QList<QVariant>,
    total: QString,
    direcciones: QList<QVariant>,
    compus: QList<QVariant>,
    puerto_cerrado: bool,
    recientes: QList<QVariant>,
    // --- lo de adentro (no se ve en QML) ---
    cfg: Config,
    lista: Vec<Archivo>,
    recientes_lista: Vec<Reciente>,
    /// cada operación trae su bandera: cancelar una no afecta a la siguiente
    cancelar: Arc<AtomicBool>,
    red_cuando: Option<Instant>,
}

impl Default for PuenteRust {
    fn default() -> Self {
        let cfg = Config::del_entorno();
        PuenteRust {
            usuario: QString::from(cfg.usuario.as_str()),
            estado: QString::default(),
            progreso: -1.0,
            ocupado: false,
            tono: QString::default(),
            codigo: QString::default(),
            archivos: QList::default(),
            total: QString::from(motor::humano(0).as_str()),
            direcciones: QList::default(),
            compus: QList::default(),
            puerto_cerrado: false,
            recientes: QList::default(),
            cfg,
            lista: Vec::new(),
            recientes_lista: Vec::new(),
            cancelar: Arc::new(AtomicBool::new(false)),
            red_cuando: None,
        }
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn mapa(pares: Vec<(&str, QVariant)>) -> QVariant {
    let mut m = QMap::<QMapPair_QString_QVariant>::default();
    for (k, v) in pares {
        m.insert(q(k), v);
    }
    QVariant::from(&m)
}

fn texto(s: &str) -> QVariant {
    QVariant::from(&q(s))
}

fn lista_de(v: Vec<QVariant>) -> QList<QVariant> {
    let mut l = QList::<QVariant>::default();
    for x in v {
        l.append(x);
    }
    l
}

/// `file:///ruta%20con%20espacios` → `/ruta con espacios` (lo que hace `QUrl(...).toLocalFile()`).
fn ruta_de_url(url: &str) -> Option<String> {
    let resto = url.strip_prefix("file://")?;
    let resto = resto.strip_prefix("localhost").unwrap_or(resto);
    if !resto.starts_with('/') {
        return None;
    }
    let b = resto.as_bytes();
    let mut fuera = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(h) = resto
                .get(i + 1..i + 3)
                .and_then(|x| u8::from_str_radix(x, 16).ok())
        {
            fuera.push(h);
            i += 3;
        } else {
            fuera.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(fuera).ok()
}

fn ruta_recientes(cfg: &Config) -> PathBuf {
    cfg.config.join("vortex").join("shared.json")
}

fn leer_recientes(cfg: &Config) -> Vec<Reciente> {
    let Ok(t) = std::fs::read_to_string(ruta_recientes(cfg)) else {
        return Vec::new();
    };
    let Ok(v) = serde_json::from_str::<Value>(&t) else {
        return Vec::new();
    };
    v["recientes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            Some(Reciente {
                host: r["host"].as_str()?.to_string(),
                usuario: r["usuario"].as_str()?.to_string(),
            })
        })
        .take(MAX_RECIENTES)
        .collect()
}

impl qobject::Puente {
    /// Estado, progreso, si está ocupado y el tono (lo que muestra la franja de abajo).
    fn poner(mut self: Pin<&mut Self>, texto: &str, progreso: f64, ocupado: bool, tono: &str) {
        self.as_mut().set_estado(q(texto));
        self.as_mut().set_progreso(progreso);
        self.as_mut().set_ocupado(ocupado);
        self.as_mut().set_tono(q(tono));
    }

    fn poner_archivos(mut self: Pin<&mut Self>) {
        let lista = self.as_ref().rust().lista.clone();
        let total: u64 = lista.iter().map(|a| a.tam).sum();
        let v = lista
            .iter()
            .map(|a| {
                mapa(vec![
                    ("ruta", texto(&a.ruta)),
                    ("nombre", texto(&a.nombre)),
                    ("tam", QVariant::from(&(a.tam as f64))),
                    ("texto", texto(&a.texto)),
                    ("carpeta", QVariant::from(&a.carpeta)),
                ])
            })
            .collect();
        self.as_mut().set_archivos(lista_de(v));
        self.as_mut().set_total(q(&motor::humano(total)));
    }

    fn poner_recientes(mut self: Pin<&mut Self>) {
        let v = self
            .as_ref()
            .rust()
            .recientes_lista
            .iter()
            .map(|r| mapa(vec![("host", texto(&r.host)), ("usuario", texto(&r.usuario))]))
            .collect();
        self.as_mut().set_recientes(lista_de(v));
    }

    fn agregar_rutas(mut self: Pin<&mut Self>, rutas: &[String]) {
        for ruta in rutas {
            let p = Path::new(ruta);
            if !p.exists() || self.as_ref().rust().lista.iter().any(|a| &a.ruta == ruta) {
                continue;
            }
            let Ok(tam) = motor::tamano(p) else { continue };
            let nombre = Path::new(ruta.trim_end_matches('/'))
                .file_name()
                .map_or_else(|| ruta.clone(), |n| n.to_string_lossy().into_owned());
            self.as_mut().rust_mut().lista.push(Archivo {
                ruta: ruta.clone(),
                nombre,
                tam,
                texto: motor::humano(tam),
                carpeta: p.is_dir(),
            });
        }
        self.poner_archivos();
    }

    fn rutas(self: &Pin<&mut Self>) -> (Vec<PathBuf>, u64) {
        let aqui = self.as_ref();
        let r = aqui.rust();
        (
            r.lista.iter().map(|a| PathBuf::from(&a.ruta)).collect(),
            r.lista.iter().map(|a| a.tam).sum(),
        )
    }

    /// Una operación nueva: bandera nueva, sin código, ocupado.
    fn empezar(mut self: Pin<&mut Self>, texto: &str) -> Arc<AtomicBool> {
        let bandera = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().cancelar = bandera.clone();
        self.as_mut().set_codigo(QString::default());
        self.as_mut().poner(texto, -1.0, true, "");
        bandera
    }

    fn recordar(mut self: Pin<&mut Self>, host: &str, usuario: &str) {
        let mut v = vec![Reciente {
            host: host.to_string(),
            usuario: usuario.to_string(),
        }];
        v.extend(
            self.as_ref()
                .rust()
                .recientes_lista
                .iter()
                .filter(|r| r.host != host)
                .cloned(),
        );
        v.truncate(MAX_RECIENTES);
        let datos = json!({"recientes": v.iter().map(|r| json!({"host": r.host, "usuario": r.usuario})).collect::<Vec<_>>()});
        let ruta = ruta_recientes(&self.as_ref().rust().cfg);
        if let Some(p) = ruta.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let _ = std::fs::write(ruta, datos.to_string());
        self.as_mut().rust_mut().recientes_lista = v;
        self.poner_recientes();
    }

    fn actualizar_red_con(mut self: Pin<&mut Self>, forzar: bool) {
        // Se llama al volver a la ventana: tailscale no se pregunta más de una vez cada 30 s.
        let ahora = Instant::now();
        let reciente = self
            .as_ref()
            .rust()
            .red_cuando
            .is_some_and(|c| ahora.duration_since(c) < Duration::from_secs(30));
        if !forzar && reciente {
            return;
        }
        self.as_mut().rust_mut().red_cuando = Some(ahora);
        let cfg = self.as_ref().rust().cfg.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let estado = motor::tailscale_estado(&cfg);
            let direcciones: Vec<QVariant> = motor::mis_direcciones(&estado, motor::ip_local())
                .iter()
                .map(|d| mapa(vec![("tipo", texto(&d.tipo)), ("ip", texto(&d.ip))]))
                .collect();
            let compus: Vec<QVariant> = motor::compus_tailscale(&estado)
                .iter()
                .map(|c| mapa(vec![("nombre", texto(&c.nombre)), ("ip", texto(&c.ip))]))
                .collect();
            let cerrado = motor::puerto_cerrado(&cfg);
            let _ = hilo.queue(move |mut obj| {
                obj.as_mut().set_direcciones(lista_de(direcciones));
                obj.as_mut().set_compus(lista_de(compus));
                obj.as_mut().set_puerto_cerrado(cerrado);
            });
        });
    }
}

fn mensaje_de(e: &ErrorShared, cancelado: bool) -> String {
    if cancelado { String::new() } else { e.0.clone() }
}

impl qobject::Puente {
    fn actualizar_red(self: Pin<&mut Self>) {
        self.actualizar_red_con(false);
    }

    fn agregar(self: Pin<&mut Self>, urls: &QList<QVariant>) {
        let rutas: Vec<String> = urls
            .iter()
            .filter_map(|v| v.value::<QString>())
            .filter_map(|u| ruta_de_url(&u.to_string()))
            .collect();
        self.agregar_rutas(&rutas);
    }

    fn quitar(mut self: Pin<&mut Self>, indice: i32) {
        let n = self.as_ref().rust().lista.len();
        if let Ok(i) = usize::try_from(indice)
            && i < n
            && !*self.as_ref().ocupado()
        {
            self.as_mut().rust_mut().lista.remove(i);
            self.poner_archivos();
        }
    }

    fn mandar(mut self: Pin<&mut Self>, host: &QString, usuario: &QString) {
        if *self.as_ref().ocupado() {
            return;
        }
        if self.as_ref().rust().lista.is_empty() {
            self.poner("Elige qué mandar.", -1.0, false, "error");
            return;
        }
        let bandera = self.as_mut().empezar("Preparando…");
        let (rutas, total) = self.rutas();
        let (host, usuario) = (
            host.to_string().trim().to_string(),
            usuario.to_string().trim().to_string(),
        );
        let cfg = self.as_ref().rust().cfg.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let resultado = (|| -> Result<(), ErrorShared> {
                let mut envio = Envio::new(&cfg, &rutas)?;
                let r = envio.preparar(Duration::from_secs(900)).and_then(|_| {
                    motor::mandar(
                        &cfg,
                        &host,
                        &usuario,
                        &envio,
                        &mut |fase, n| {
                            let (t, p) = if fase == "esperando" {
                                ("Esperando a que acepten en la otra compu…".to_string(), -1.0)
                            } else {
                                (
                                    format!("Mandando: {} de {}", motor::humano(n), motor::humano(total)),
                                    if total > 0 {
                                        (n as f64 / total as f64).min(1.0)
                                    } else {
                                        -1.0
                                    },
                                )
                            };
                            let _ = hilo.queue(move |obj| obj.poner(&t, p, true, ""));
                        },
                        &|| bandera.load(Ordering::SeqCst),
                        Duration::from_secs(150),
                    )
                });
                envio.terminar();
                r
            })();
            let cancelado = bandera.load(Ordering::SeqCst);
            let _ = hilo.queue(move |mut obj| match resultado {
                Ok(()) => {
                    obj.as_mut().recordar(&host, &usuario);
                    obj.poner("Listo: ya llegó.", 1.0, false, "exito");
                }
                Err(e) => obj.poner(&mensaje_de(&e, cancelado), -1.0, false, "error"),
            });
        });
    }

    fn crear_codigo(mut self: Pin<&mut Self>) {
        if *self.as_ref().ocupado() {
            return;
        }
        if self.as_ref().rust().lista.is_empty() {
            self.poner("Elige qué mandar.", -1.0, false, "error");
            return;
        }
        let bandera = self.as_mut().empezar("Preparando el código…");
        let (rutas, _) = self.rutas();
        let cfg = self.as_ref().rust().cfg.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let preparado = Envio::new(&cfg, &rutas).and_then(|mut e| {
                let t = e.preparar(Duration::from_secs(900));
                t.map(|t| (e, t))
            });
            match preparado {
                Ok((mut envio, ticket)) => {
                    {
                        // la bandera se mira con el candado puesto: así «cancelar» no se cuela entre medias
                        let mut hueco = ENVIO_CODIGO.lock().unwrap();
                        if bandera.load(Ordering::SeqCst) {
                            drop(hueco);
                            envio.terminar();
                            return;
                        }
                        *hueco = Some(envio);
                    }
                    let _ = hilo.queue(move |mut obj| {
                        obj.as_mut().set_codigo(q(&ticket));
                        obj.poner(
                            "Pégalo en la otra compu. Deja esta ventana abierta hasta que lo reciban.",
                            -1.0,
                            true,
                            "",
                        );
                    });
                }
                Err(e) => {
                    let _ = hilo.queue(move |obj| obj.poner(&e.0, -1.0, false, "error"));
                }
            }
        });
    }

    fn cancelar(mut self: Pin<&mut Self>) {
        // Corta lo que esté pasando: un envío por aviso se corta solo en su hilo; un código, aquí.
        self.as_ref().rust().cancelar.store(true, Ordering::SeqCst);
        if !self.as_ref().codigo().is_empty() {
            let envio = ENVIO_CODIGO.lock().unwrap().take();
            if let Some(mut e) = envio {
                std::thread::spawn(move || e.terminar());
            }
        }
        self.as_mut().set_codigo(QString::default());
        self.poner("", -1.0, false, "");
    }

    fn recibir_codigo(mut self: Pin<&mut Self>, codigo: &QString) {
        if *self.as_ref().ocupado() {
            return;
        }
        let bandera = self.as_mut().empezar("Recibiendo…");
        let codigo = codigo.to_string();
        let cfg = self.as_ref().rust().cfg.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let mut avance = |n: u64| {
                let t = format!("Recibiendo: {}", motor::humano(n));
                let _ = hilo.queue(move |obj| obj.poner(&t, -1.0, true, ""));
            };
            let r = motor::recibir(
                &cfg,
                &codigo,
                Some(&mut avance),
                &|| bandera.load(Ordering::SeqCst),
                0,
            );
            let cancelado = bandera.load(Ordering::SeqCst);
            let _ = hilo.queue(move |obj| match r {
                Ok(quedaron) => {
                    let nombre = |p: &PathBuf| {
                        p.file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                    };
                    let t = if quedaron.len() == 1 {
                        nombre(&quedaron[0])
                    } else {
                        format!("{} y {} más", nombre(&quedaron[0]), quedaron.len() - 1)
                    };
                    obj.poner(&format!("Ya llegó: {t}"), 1.0, false, "exito");
                }
                Err(e) => obj.poner(&mensaje_de(&e, cancelado), -1.0, false, "error"),
            });
        });
    }

    fn abrir_puerto(self: Pin<&mut Self>) {
        let cfg = self.as_ref().rust().cfg.clone();
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            motor::abrir_puerto(&cfg);
            let _ = hilo.queue(|obj| obj.actualizar_red_con(true));
        });
    }

    fn copiar(self: Pin<&mut Self>, texto: &QString) {
        crate::arranque::ffi::vortex_shared_copiar(&texto.to_string());
    }

    fn abrir_carpeta(self: Pin<&mut Self>) {
        let cfg = self.as_ref().rust().cfg.clone();
        let carpeta = motor::carpeta_recibidos(&cfg);
        if std::fs::create_dir_all(&carpeta).is_err() {
            return;
        }
        cfg.sistema.abrir(&carpeta.to_string_lossy());
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        let hilo = self.qt_thread();
        let recientes = leer_recientes(&self.as_ref().rust().cfg);
        self.as_mut().rust_mut().recientes_lista = recientes;
        self.as_mut().poner_recientes();
        let iniciales = std::mem::take(&mut *ARCHIVOS_INICIALES.lock().unwrap());
        self.as_mut().agregar_rutas(&iniciales);
        self.as_mut().actualizar_red_con(true);
        // En las pruebas (--prueba-humo, recorrido) no se toca el socket de una ventana de verdad.
        if std::env::var_os("VORTEX_SHARED_RECORRER").is_some()
            || std::env::args().any(|a| a == "--prueba-humo")
        {
            return;
        }
        // Una sola ventana: lo que pida otra copia (con archivos o sin ellos) llega por un socket; se espera
        // sin gastar nada.
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        std::thread::spawn(move || {
            while let Some(rutas) = escucha.esperar() {
                let sigue = hilo.queue(move |mut obj| {
                    obj.as_mut().agregar_rutas(&rutas);
                    obj.mostrar_ventana();
                });
                if sigue.is_err() {
                    break; // la ventana ya se cerró
                }
            }
        });
    }
}
