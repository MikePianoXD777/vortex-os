//! `cam`: lo mismo que `puente.py`, sobre el motor `vortex_camera` (ya probado). Mismos nombres de
//! propiedades, señales y slots, para que los QML de la de Python no cambien.
//!
//! Lo que tarda (buscar en la red, revisar qué cámaras contestan, fotos, mover las PTZ) va en hilos aparte y
//! el resultado vuelve al hilo de la interfaz con `qt_thread().queue`. El código de seguridad nunca pasa a
//! QML salvo dentro de la URL del video, que QML pide con `urlVivo`/`urlGrabado` cuando la va a usar (y en
//! `codigoGuardado`, solo para el ojito de «Editar»).
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
        #[qproperty(QVariant, camaras)]
        #[qproperty(QVariant, ventanas)]
        #[qproperty(QVariant, divisiones)]
        #[qproperty(QVariant, estados)]
        #[qproperty(QVariant, grabando)]
        #[qproperty(QVariant, encontradas)]
        #[qproperty(bool, buscando)]
        type Puente = super::PuenteRust;

        /// texto, y si es un error
        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: &QString, es_error: bool);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "camarasCambio"]
        fn camaras_cambio(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "ventanasCambio"]
        fn ventanas_cambio(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "estadosCambio"]
        fn estados_cambio(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "busquedaCambio"]
        fn busqueda_cambio(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "grabandoCambio"]
        fn grabando_cambio(self: Pin<&mut Puente>);

        #[qinvokable]
        fn camara(self: &Puente, ident: &QString) -> QVariant;
        #[qinvokable]
        #[cxx_name = "codigoGuardado"]
        fn codigo_guardado(self: &Puente, ident: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "urlVivo"]
        fn url_vivo(self: &Puente, ident: &QString, principal: bool) -> QString;
        #[qinvokable]
        #[cxx_name = "urlGrabado"]
        fn url_grabado(self: &Puente, ident: &QString, fecha: &QString, hora: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "moverHora"]
        fn mover_hora(self: &Puente, fecha: &QString, hora: &QString, minutos: i32) -> QVariant;

        #[qinvokable]
        #[cxx_name = "agregarCamara"]
        fn agregar_camara(self: Pin<&mut Puente>, datos: &QVariant) -> QString;
        #[qinvokable]
        #[cxx_name = "editarCamara"]
        fn editar_camara(self: Pin<&mut Puente>, ident: &QString, datos: &QVariant) -> QString;
        #[qinvokable]
        #[cxx_name = "quitarCamara"]
        fn quitar_camara(self: Pin<&mut Puente>, ident: &QString);

        #[qinvokable]
        #[cxx_name = "buscarEnRed"]
        fn buscar_en_red(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "revisarEstados"]
        fn revisar_estados(self: Pin<&mut Puente>);
        #[qinvokable]
        fn reintentar(self: Pin<&mut Puente>, ident: &QString);

        #[qinvokable]
        fn foto(self: Pin<&mut Puente>, ident: &QString);
        #[qinvokable]
        #[cxx_name = "alternarGrabar"]
        fn alternar_grabar(self: Pin<&mut Puente>, ident: &QString);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: &Puente, tipo: &QString);

        #[qinvokable]
        #[cxx_name = "moverPtz"]
        fn mover_ptz(self: Pin<&mut Puente>, ident: &QString, direccion: &QString);
        #[qinvokable]
        #[cxx_name = "pararPtz"]
        fn parar_ptz(self: Pin<&mut Puente>, ident: &QString, direccion: &QString);
        #[qinvokable]
        #[cxx_name = "probarPtz"]
        fn probar_ptz(self: Pin<&mut Puente>, ident: &QString);

        #[qinvokable]
        #[cxx_name = "nuevaVentana"]
        fn nueva_ventana(self: Pin<&mut Puente>) -> i32;
        #[qinvokable]
        #[cxx_name = "quitarVentana"]
        fn quitar_ventana(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "renombrarVentana"]
        fn renombrar_ventana(self: Pin<&mut Puente>, i: i32, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "ponerDivision"]
        fn poner_division(self: Pin<&mut Puente>, i: i32, division: i32);
        #[qinvokable]
        #[cxx_name = "ponerCelda"]
        fn poner_celda(self: Pin<&mut Puente>, i: i32, pos: i32, ident: &QString);

        /// Cierra las grabaciones (ffmpeg recibe «q») y para los relojes. Se llama al salir.
        #[qinvokable]
        fn cerrar(self: Pin<&mut Puente>);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use crate::arranque::ffi::{json_a_variante, variante_a_json};
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;
use vortex_camera::camaras::{Almacen, Camara, DIVISIONES, limpiar_serie};
use vortex_camera::descubrir::{self, Prueba};
use vortex_camera::fecha::FechaHora;
use vortex_camera::grabar::{self, Grabacion};
use vortex_camera::{instancia, ptz};

const REVISAR_CADA: Duration = Duration::from_secs(30);
const VIGILAR_CADA: Duration = Duration::from_secs(3);

/// Un trabajo para el hilo de las PTZ: en orden, como el ThreadPoolExecutor de un solo hilo de Python
/// (mover y luego parar).
enum TrabajoPtz {
    Mover(Camara, String),
    Parar(Camara, String),
}

pub struct PuenteRust {
    almacen: Almacen,
    estados_mapa: HashMap<String, String>,
    grabaciones: HashMap<String, Grabacion>,
    encontradas_lista: Vec<Value>,
    revisar: bool,
    vivo: Arc<AtomicBool>,
    ptz: Option<mpsc::Sender<TrabajoPtz>>,
    // Las propiedades que ve QML.
    camaras: QVariant,
    ventanas: QVariant,
    divisiones: QVariant,
    estados: QVariant,
    grabando: QVariant,
    encontradas: QVariant,
    buscando: bool,
}

impl Default for PuenteRust {
    fn default() -> Self {
        PuenteRust {
            almacen: Almacen::new(None),
            estados_mapa: HashMap::new(),
            grabaciones: HashMap::new(),
            encontradas_lista: Vec::new(),
            // VORTEX_CAMERA_SIN_REVISAR=1: sin revisar ni buscar solo (el `revisar=False` de las pruebas).
            revisar: std::env::var_os("VORTEX_CAMERA_SIN_REVISAR").is_none_or(|v| v.is_empty()),
            vivo: Arc::new(AtomicBool::new(true)),
            ptz: None,
            camaras: QVariant::default(),
            ventanas: QVariant::default(),
            divisiones: QVariant::default(),
            estados: QVariant::default(),
            grabando: QVariant::default(),
            encontradas: QVariant::default(),
            buscando: false,
        }
    }
}

fn q(s: &str) -> QString {
    QString::from(s)
}

fn variante(v: &Value) -> QVariant {
    json_a_variante(&v.to_string())
}

/// Lo que manda QML (un objeto de JavaScript) como objeto JSON.
fn objeto(v: &QVariant) -> Map<String, Value> {
    serde_json::from_str::<Value>(&variante_a_json(v))
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

/// IPv4, IPv6 o nombre de Tailscale (casa-juanita).
fn direccion_valida(ip: &str) -> bool {
    !ip.is_empty()
        && ip
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-:[]".contains(c))
}

fn verdadero(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(_) => true,
    }
}

fn texto_de(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(otro) => otro.to_string(),
    }
}

fn nombre_archivo(r: &std::path::Path) -> String {
    r.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn abrir(ruta: &std::path::Path) {
    // Como QDesktopServices.openUrl: el administrador de archivos de la persona.
    let _ = std::process::Command::new("xdg-open")
        .arg(ruta)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Corre `trabajo` en otro hilo y manda el resultado a `listo`, en el hilo de la interfaz.
fn lanzar<T: Send + 'static>(
    hilo: CxxQtThread<qobject::Puente>,
    trabajo: impl FnOnce() -> T + Send + 'static,
    listo: impl FnOnce(Pin<&mut qobject::Puente>, T) + Send + 'static,
) {
    std::thread::spawn(move || {
        let valor = trabajo();
        let _ = hilo.queue(move |obj| listo(obj, valor));
    });
}

impl qobject::Puente {
    // --- sincronizar lo que ve QML ---

    fn poner_camaras(mut self: Pin<&mut Self>) {
        let v = variante(&self.rust().almacen.camaras_publicas());
        self.as_mut().set_camaras(v);
        self.camaras_cambio();
    }

    fn poner_ventanas(mut self: Pin<&mut Self>) {
        let v = variante(&self.rust().almacen.ventanas_publicas());
        self.as_mut().set_ventanas(v);
        self.ventanas_cambio();
    }

    fn poner_estados(mut self: Pin<&mut Self>) {
        let m: Map<String, Value> = self
            .rust()
            .estados_mapa
            .iter()
            .map(|(k, v)| (k.clone(), Value::String(v.clone())))
            .collect();
        self.as_mut().set_estados(variante(&Value::Object(m)));
        self.estados_cambio();
    }

    fn poner_grabando(mut self: Pin<&mut Self>) {
        let m: Map<String, Value> = self
            .rust()
            .grabaciones
            .keys()
            .map(|k| (k.clone(), Value::Bool(true)))
            .collect();
        self.as_mut().set_grabando(variante(&Value::Object(m)));
        self.grabando_cambio();
    }

    fn poner_busqueda(mut self: Pin<&mut Self>, buscando: bool) {
        let v = variante(&Value::Array(self.rust().encontradas_lista.clone()));
        self.as_mut().set_encontradas(v);
        self.as_mut().set_buscando(buscando);
        self.busqueda_cambio();
    }

    fn avisar(self: Pin<&mut Self>, texto: &str, error: bool) {
        self.aviso(&q(texto), error);
    }

    // --- lo que se pregunta ---

    fn camara(&self, ident: &QString) -> QVariant {
        match self.rust().almacen.camara(&ident.to_string()) {
            Some(c) => variante(&c.publica()),
            None => variante(&json!({})),
        }
    }

    fn codigo_guardado(&self, ident: &QString) -> QString {
        self.rust()
            .almacen
            .camara(&ident.to_string())
            .map_or_else(QString::default, |c| q(&c.codigo))
    }

    fn url_vivo(&self, ident: &QString, principal: bool) -> QString {
        self.rust()
            .almacen
            .camara(&ident.to_string())
            .map_or_else(QString::default, |c| q(&c.url_vivo(principal)))
    }

    /// Lo grabado en la microSD desde «AAAA-MM-DD» «HH:MM» y hasta 24 horas después (la cámara manda lo que
    /// tenga y salta los huecos).
    fn url_grabado(&self, ident: &QString, fecha: &QString, hora: &QString) -> QString {
        let c = self.rust().almacen.camara(&ident.to_string());
        let inicio = FechaHora::leer(&fecha.to_string(), &hora.to_string());
        match (c, inicio) {
            (Some(c), Some(i)) => q(&c.url_grabado(&i, &i.mas_minutos(24 * 60))),
            _ => QString::default(),
        }
    }

    /// «2026-10-30» «23:55» + 10 min → ["2026-10-31", "00:05"], para los botones de saltar.
    fn mover_hora(&self, fecha: &QString, hora: &QString, minutos: i32) -> QVariant {
        let inicio = FechaHora::leer(&fecha.to_string(), &hora.to_string()).unwrap_or_else(|| {
            let a = FechaHora::ahora();
            FechaHora { segundo: 0, ..a }
        });
        let n = inicio.mas_minutos(i64::from(minutos));
        variante(&json!([n.dia_texto(), n.hora_texto()]))
    }

    // --- cámaras ---

    fn validar(&self, datos: &Map<String, Value>, nueva: bool) -> (Map<String, Value>, String) {
        let mut limpio: Map<String, Value> = ["nombre", "serie", "codigo", "ip", "puerto", "canal"]
            .iter()
            .filter_map(|k| datos.get(*k).map(|v| ((*k).to_string(), v.clone())))
            .map(|(k, v)| match v {
                Value::String(s) => (k, Value::String(s.trim().to_string())),
                otro => (k, otro),
            })
            .collect();
        if !verdadero(limpio.get("nombre")) {
            return (limpio, "Ponle un nombre a la cámara.".into());
        }
        if nueva && !verdadero(limpio.get("codigo")) {
            return (limpio, "Falta el código de seguridad (viene en la etiqueta) o, en las Imou viejas, la contraseña que se le puso en Imou Life.".into());
        }
        if !nueva && !verdadero(limpio.get("codigo")) {
            limpio.remove("codigo"); // en blanco al editar: se queda el que tenía
        }
        let serie = limpiar_serie(&texto_de(limpio.get("serie")));
        let ip = texto_de(limpio.get("ip"));
        if !verdadero(limpio.get("ip")) && serie.is_empty() {
            return (limpio, "Escribe el número de serie o la IP de la cámara.".into());
        }
        if verdadero(limpio.get("ip")) && !direccion_valida(&ip) {
            return (limpio, "Esa IP no se ve bien (algo como 192.168.1.20).".into());
        }
        let yo = texto_de(datos.get("id"));
        if let Some(otra) = self.rust().almacen.por_serie(&serie)
            && (nueva || otra.id != yo)
        {
            return (
                limpio,
                format!("Esa cámara ya está agregada como «{}».", otra.nombre),
            );
        }
        (limpio, String::new())
    }

    fn agregar_camara(mut self: Pin<&mut Self>, datos: &QVariant) -> QString {
        let (limpio, error) = self.validar(&objeto(datos), true);
        if !error.is_empty() {
            return q(&error);
        }
        let c = self.as_mut().rust_mut().almacen.agregar(&limpio);
        self.as_mut().poner_camaras();
        self.as_mut().poner_ventanas();
        if c.ip.is_empty() {
            self.buscar(false, &c.id);
        } else {
            self.revisar(vec![c.id], true);
        }
        QString::default()
    }

    fn editar_camara(mut self: Pin<&mut Self>, ident: &QString, datos: &QVariant) -> QString {
        let ident = ident.to_string();
        let mut d = objeto(datos);
        d.insert("id".into(), Value::String(ident.clone()));
        let (limpio, error) = self.validar(&d, false);
        if !error.is_empty() {
            return q(&error);
        }
        let Some(c) = self.as_mut().rust_mut().almacen.actualizar(&ident, &limpio) else {
            return q("Esa cámara ya no existe.");
        };
        self.as_mut().poner_camaras();
        self.as_mut().poner_ventanas(); // para que los cuadros vuelvan a pedir su URL
        if c.ip.is_empty() {
            self.buscar(false, &c.id);
        } else {
            self.revisar(vec![c.id], true);
        }
        QString::default()
    }

    fn quitar_camara(mut self: Pin<&mut Self>, ident: &QString) {
        let ident = ident.to_string();
        if self.rust().grabaciones.contains_key(&ident) {
            self.as_mut().detener_grabacion(&ident);
        }
        self.as_mut().rust_mut().almacen.quitar(&ident);
        self.as_mut().rust_mut().estados_mapa.remove(&ident);
        self.as_mut().poner_camaras();
        self.as_mut().poner_ventanas();
        self.poner_estados();
    }

    // --- buscar en la red y revisar quién contesta ---

    fn buscar_en_red(self: Pin<&mut Self>) {
        self.buscar(false, "");
    }

    fn buscar(mut self: Pin<&mut Self>, silencioso: bool, nueva: &str) {
        if self.rust().buscando {
            return;
        }
        self.as_mut().poner_busqueda(true);
        // Al abrir (silencioso) solo hace falta si hay cámaras con número de serie, para corregir su IP.
        let hace_falta = !silencioso || self.rust().almacen.camaras.iter().any(|c| !c.serie.is_empty());
        let nueva = nueva.to_string();
        lanzar(
            self.qt_thread(),
            move || {
                if hace_falta {
                    descubrir::buscar_todo(2.5)
                } else {
                    Vec::new()
                }
            },
            move |mut obj, halladas| {
                let mut cambiaron = Vec::new();
                for h in &halladas {
                    // La cámara cambió de IP (el módem le dio otra): se corrige sola.
                    let Some(c) = obj.rust().almacen.por_serie(&h.serie).cloned() else {
                        continue;
                    };
                    if !h.ip.is_empty() && c.ip != h.ip {
                        let mut d = Map::new();
                        d.insert("ip".into(), Value::String(h.ip.clone()));
                        obj.as_mut().rust_mut().almacen.actualizar(&c.id, &d);
                        cambiaron.push(c.id);
                    }
                }
                let a = &obj.rust().almacen;
                let agregadas: Vec<String> = a
                    .camaras
                    .iter()
                    .flat_map(|c| [c.serie.clone(), c.ip.clone()])
                    .filter(|x| !x.is_empty())
                    .collect();
                let lista: Vec<Value> = halladas
                    .iter()
                    .map(|h| {
                        let mut m = h.a_json();
                        let ya = agregadas.contains(&h.serie) || agregadas.contains(&h.ip);
                        m.insert("agregada".into(), Value::Bool(ya));
                        Value::Object(m)
                    })
                    .collect();
                obj.as_mut().rust_mut().encontradas_lista = lista;
                obj.as_mut().poner_busqueda(false);
                if !cambiaron.is_empty() {
                    obj.as_mut().poner_camaras();
                    obj.as_mut().poner_ventanas();
                    obj.as_mut().revisar(cambiaron, true);
                }
                if !nueva.is_empty() {
                    if let Some(c) = obj.rust().almacen.camara(&nueva).cloned()
                        && c.ip.is_empty()
                    {
                        obj.as_mut()
                            .rust_mut()
                            .estados_mapa
                            .insert(nueva, "sin_ip".into());
                        obj.as_mut().poner_estados();
                        obj.avisar(
                            &format!("No encontré «{}» en tu red. Revisa que esté prendida y conectada, o escribe su IP en Editar.", c.nombre),
                            true,
                        );
                    }
                } else if !silencioso && halladas.is_empty() {
                    obj.avisar("No encontré cámaras en tu red.", true);
                }
            },
        );
    }

    fn revisar_estados(self: Pin<&mut Self>) {
        let ids = self.rust().almacen.camaras.iter().map(|c| c.id.clone()).collect();
        self.revisar(ids, false);
    }

    /// «Volver a intentar» a mano, aunque no haya aceptado el código.
    fn reintentar(self: Pin<&mut Self>, ident: &QString) {
        self.revisar(vec![ident.to_string()], true);
    }

    /// ¿Contesta y acepta el código? Las Dahua/Imou bloquean el usuario tras varios intentos fallidos: a una
    /// que ya no aceptó el código no se le vuelve a preguntar sola (solo al editarla o con «Volver a intentar»).
    fn revisar(mut self: Pin<&mut Self>, ids: Vec<String>, forzar: bool) {
        let camaras: Vec<Camara> = ids
            .iter()
            .filter_map(|i| self.rust().almacen.camara(i).cloned())
            .filter(|c| forzar || self.rust().estados_mapa.get(&c.id).map(String::as_str) != Some("codigo"))
            .collect();
        if camaras.is_empty() {
            return;
        }
        for c in &camaras {
            let actual = self.rust().estados_mapa.get(&c.id).cloned();
            if c.ip.is_empty() {
                self.as_mut()
                    .rust_mut()
                    .estados_mapa
                    .insert(c.id.clone(), "sin_ip".into());
            } else if forzar || actual.is_none() || actual.as_deref() == Some("sin_ip") {
                self.as_mut()
                    .rust_mut()
                    .estados_mapa
                    .insert(c.id.clone(), "revisando".into());
            }
        }
        self.as_mut().poner_estados();
        let con_ip: Vec<Camara> = camaras.into_iter().filter(|c| !c.ip.is_empty()).collect();
        if con_ip.is_empty() {
            return;
        }
        lanzar(
            self.qt_thread(),
            move || {
                let mut res = Vec::new();
                for grupo in con_ip.chunks(16) {
                    let hilos: Vec<_> = grupo
                        .iter()
                        .cloned()
                        .map(|c| {
                            std::thread::spawn(move || {
                                let ruta = format!("/cam/realmonitor?channel={}&subtype=1", c.canal);
                                let r = descubrir::probar_rtsp(
                                    &c.ip,
                                    c.puerto,
                                    &ruta,
                                    &c.usuario,
                                    &c.codigo,
                                    Duration::from_secs(4),
                                );
                                let e = match r {
                                    Prueba::Ok | Prueba::Error => "en_linea",
                                    Prueba::Codigo => "codigo",
                                    Prueba::SinConexion => "sin_conexion",
                                };
                                (c.id, e.to_string())
                            })
                        })
                        .collect();
                    res.extend(hilos.into_iter().filter_map(|h| h.join().ok()));
                }
                res
            },
            |mut obj, resultado: Vec<(String, String)>| {
                for (ident, estado) in resultado {
                    if obj.rust().almacen.camara(&ident).is_some() {
                        obj.as_mut().rust_mut().estados_mapa.insert(ident, estado);
                    }
                }
                obj.poner_estados();
            },
        );
    }

    // --- fotos y videos ---

    fn foto(self: Pin<&mut Self>, ident: &QString) {
        let Some(c) = self.rust().almacen.camara(&ident.to_string()).cloned() else {
            return self.avisar("Esa cámara no tiene IP todavía.", true);
        };
        if c.ip.is_empty() {
            return self.avisar("Esa cámara no tiene IP todavía.", true);
        }
        let url = c.url_vivo(true);
        lanzar(
            self.qt_thread(),
            move || grabar::foto(&url, &grabar::carpeta_fotos(), &c.nombre, Duration::from_secs(20)),
            |obj, r| match r {
                Ok(ruta) => obj.avisar(&format!("Foto guardada: {}", nombre_archivo(&ruta)), false),
                Err(e) => obj.avisar(&e, true),
            },
        );
    }

    fn alternar_grabar(mut self: Pin<&mut Self>, ident: &QString) {
        let ident = ident.to_string();
        if self.rust().grabaciones.contains_key(&ident) {
            return self.detener_grabacion(&ident);
        }
        let Some(c) = self
            .rust()
            .almacen
            .camara(&ident)
            .cloned()
            .filter(|c| !c.ip.is_empty())
        else {
            return self.avisar("Esa cámara no tiene IP todavía.", true);
        };
        match Grabacion::new(&c.url_vivo(true), &grabar::carpeta_videos(), &c.nombre) {
            Ok(g) => {
                self.as_mut().rust_mut().grabaciones.insert(ident, g);
                self.poner_grabando();
            }
            Err(e) => self.avisar(&e, true),
        }
    }

    fn detener_grabacion(mut self: Pin<&mut Self>, ident: &str) {
        let g = self.as_mut().rust_mut().grabaciones.remove(ident);
        self.as_mut().poner_grabando();
        let Some(g) = g else { return };
        lanzar(
            self.qt_thread(),
            move || g.detener(Duration::from_secs(8)),
            |obj, ruta| match ruta {
                Some(r) => obj.avisar(&format!("Video guardado: {}", nombre_archivo(&r)), false),
                None => obj.avisar("No se grabó nada.", true),
            },
        );
    }

    /// Una grabación que se cortó sola (la cámara se cayó).
    fn vigilar_grabaciones(mut self: Pin<&mut Self>) {
        let cortadas: Vec<String> = self
            .as_mut()
            .rust_mut()
            .grabaciones
            .iter_mut()
            .filter_map(|(k, g)| (!g.activa()).then(|| k.clone()))
            .collect();
        for ident in cortadas {
            let Some(mut g) = self.as_mut().rust_mut().grabaciones.remove(&ident) else {
                continue;
            };
            self.as_mut().poner_grabando();
            let motivo = g.error();
            let ruta = g.detener(Duration::from_secs(8));
            let nombre = self
                .rust()
                .almacen
                .camara(&ident)
                .map_or_else(|| "la cámara".to_string(), |c| c.nombre.clone());
            let mut texto = format!("Se cortó la grabación de «{nombre}»");
            if motivo.is_empty() {
                texto.push('.');
            } else {
                texto.push_str(&format!(": {motivo}"));
            }
            if let Some(r) = ruta {
                texto.push_str(&format!(" Lo grabado quedó en {}.", nombre_archivo(&r)));
            }
            self.as_mut().avisar(&texto, true);
        }
    }

    fn abrir_carpeta(&self, tipo: &QString) {
        let carpeta = if tipo.to_string() == "videos" {
            grabar::carpeta_videos()
        } else {
            grabar::carpeta_fotos()
        };
        let _ = std::fs::create_dir_all(&carpeta);
        abrir(&carpeta);
    }

    // --- mover las PTZ ---

    fn mover_ptz(self: Pin<&mut Self>, ident: &QString, direccion: &QString) {
        let (ident, dir) = (ident.to_string(), direccion.to_string());
        let Some(c) = self.rust().almacen.camara(&ident).cloned() else {
            return;
        };
        if c.ip.is_empty() || ptz::direccion(&dir).is_none() {
            return;
        }
        if let Some(tx) = &self.rust().ptz {
            let _ = tx.send(TrabajoPtz::Mover(c, dir));
        }
    }

    fn parar_ptz(self: Pin<&mut Self>, ident: &QString, direccion: &QString) {
        let Some(c) = self.rust().almacen.camara(&ident.to_string()).cloned() else {
            return;
        };
        if let Some(tx) = &self.rust().ptz {
            let _ = tx.send(TrabajoPtz::Parar(c, direccion.to_string()));
        }
    }

    /// «Volver a probar» las flechas de una cámara marcada como que no se mueve.
    fn probar_ptz(mut self: Pin<&mut Self>, ident: &QString) {
        let ident = ident.to_string();
        if self.rust().almacen.camara(&ident).is_some() {
            let mut d = Map::new();
            d.insert("ptz".into(), Value::String(String::new()));
            self.as_mut().rust_mut().almacen.actualizar(&ident, &d);
            self.poner_camaras();
        }
    }

    /// Lo que contestó el hilo de las PTZ al mover: guardar cómo se mueve, o esconder las flechas.
    fn ptz_movio(mut self: Pin<&mut Self>, ident: &str, antes: &str, r: Result<String, String>) {
        let mut d = Map::new();
        match r {
            Ok(metodo) => {
                if antes != metodo {
                    d.insert("ptz".into(), Value::String(metodo));
                    self.as_mut().rust_mut().almacen.actualizar(ident, &d);
                    self.poner_camaras();
                }
            }
            Err(_) if antes.is_empty() => {
                // nunca había funcionado: se marca para esconder las flechas
                d.insert("ptz".into(), Value::String("no".into()));
                let nombre = self.rust().almacen.camara(ident).map(|c| c.nombre.clone());
                self.as_mut().rust_mut().almacen.actualizar(ident, &d);
                self.as_mut().poner_camaras();
                if let Some(n) = nombre {
                    self.avisar(&format!("«{n}» no se deja mover desde la PC."), true);
                }
            }
            Err(_) => self.avisar("No se pudo mover la cámara.", true),
        }
    }

    // --- ventanas del mosaico ---

    fn nueva_ventana(mut self: Pin<&mut Self>) -> i32 {
        let i = self.as_mut().rust_mut().almacen.nueva_ventana();
        self.poner_ventanas();
        i as i32
    }

    fn quitar_ventana(mut self: Pin<&mut Self>, i: i32) {
        if let Ok(i) = usize::try_from(i) {
            self.as_mut().rust_mut().almacen.quitar_ventana(i);
        }
        self.poner_ventanas();
    }

    fn renombrar_ventana(mut self: Pin<&mut Self>, i: i32, nombre: &QString) {
        if let Ok(i) = usize::try_from(i) {
            self.as_mut()
                .rust_mut()
                .almacen
                .renombrar_ventana(i, &nombre.to_string());
        }
        self.poner_ventanas();
    }

    fn poner_division(mut self: Pin<&mut Self>, i: i32, division: i32) {
        if let (Ok(i), Ok(d)) = (usize::try_from(i), usize::try_from(division)) {
            self.as_mut().rust_mut().almacen.poner_division(i, d);
        }
        self.poner_ventanas();
    }

    fn poner_celda(mut self: Pin<&mut Self>, i: i32, pos: i32, ident: &QString) {
        if let (Ok(i), Ok(p)) = (usize::try_from(i), usize::try_from(pos)) {
            self.as_mut()
                .rust_mut()
                .almacen
                .poner_celda(i, p, &ident.to_string());
        }
        self.poner_ventanas();
    }

    fn cerrar(mut self: Pin<&mut Self>) {
        self.rust().vivo.store(false, Ordering::Relaxed);
        let grabaciones: Vec<Grabacion> = self
            .as_mut()
            .rust_mut()
            .grabaciones
            .drain()
            .map(|(_, g)| g)
            .collect();
        for g in grabaciones {
            let _ = g.detener(Duration::from_secs(8));
        }
        self.as_mut().rust_mut().ptz = None; // el hilo de las PTZ termina solo
    }
}

/// El hilo de las PTZ: mueve y para en el orden en que llegan, y avisa a la interfaz cómo le fue.
fn hilo_ptz(rx: mpsc::Receiver<TrabajoPtz>, hilo: CxxQtThread<qobject::Puente>) {
    // Cómo se movió cada cámara en esta sesión, para que el «parar» que viene detrás sepa cómo.
    let mut metodos: HashMap<String, String> = HashMap::new();
    for t in rx {
        match t {
            TrabajoPtz::Mover(c, dir) => {
                let antes = c.ptz.clone();
                metodos.remove(&c.id);
                let r = ptz::mover(&c, &dir).map_err(|e| e.to_string());
                if let Ok(m) = &r {
                    metodos.insert(c.id.clone(), m.clone());
                }
                let id = c.id.clone();
                let _ = hilo.queue(move |obj| obj.ptz_movio(&id, &antes, r));
            }
            TrabajoPtz::Parar(mut c, dir) => {
                if !matches!(c.ptz.as_str(), "onvif" | "cgi")
                    && let Some(m) = metodos.get(&c.id)
                {
                    c.ptz = m.clone();
                }
                if matches!(c.ptz.as_str(), "onvif" | "cgi") {
                    let _ = ptz::parar(&c, &dir);
                }
            }
        }
    }
}

impl cxx_qt::Initialize for qobject::Puente {
    fn initialize(mut self: Pin<&mut Self>) {
        let divisiones = variante(&json!(DIVISIONES));
        self.as_mut().set_divisiones(divisiones);
        self.as_mut().poner_camaras();
        self.as_mut().poner_ventanas();
        self.as_mut().poner_estados();
        self.as_mut().poner_grabando();
        self.as_mut().poner_busqueda(false);

        let (tx, rx) = mpsc::channel();
        self.as_mut().rust_mut().ptz = Some(tx);
        let hilo = self.qt_thread();
        std::thread::spawn(move || hilo_ptz(rx, hilo));

        // Los relojes: cada 3 s las grabaciones que se cortaron; cada 30 s quién contesta.
        let vivo = self.rust().vivo.clone();
        let revisar = self.rust().revisar;
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            let mut desde_revisar = Duration::ZERO;
            while vivo.load(Ordering::Relaxed) {
                std::thread::sleep(VIGILAR_CADA);
                desde_revisar += VIGILAR_CADA;
                let toca = revisar && desde_revisar >= REVISAR_CADA;
                if toca {
                    desde_revisar = Duration::ZERO;
                }
                let r = hilo.queue(move |mut obj| {
                    obj.as_mut().vigilar_grabaciones();
                    if toca {
                        obj.revisar_estados();
                    }
                });
                if r.is_err() {
                    break;
                }
            }
        });
        if revisar {
            let _ = self.qt_thread().queue(|mut obj| {
                obj.as_mut().revisar_estados();
                obj.buscar(true, "");
            });
        }

        // Una sola ventana: si otra copia pide «mostrar», se avisa a QML (Main.qml escucha mostrarVentana).
        let Ok(escucha) = instancia::Escucha::new(&instancia::ruta(&instancia::nombre())) else {
            return;
        };
        let hilo = self.qt_thread();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(250));
                for _ in 0..escucha.revisar() {
                    if hilo.queue(|obj| obj.mostrar_ventana()).is_err() {
                        return;
                    }
                }
            }
        });
    }
}
