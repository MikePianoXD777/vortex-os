//! `vp`: lo mismo que `puente.py`, sobre el motor `vortex_player` (ya probado). Mismos nombres de
//! propiedades, señales y botones, para que los QML de la de Python no cambien.
//!
//! Lo que tarda (leer carpetas, bajar, buscar en YouTube, la nube, las letras) va en hilos aparte y vuelve
//! al hilo de la interfaz con `qt_thread().queue`. La base SQLite se comparte entre hilos con un candado.
//! Los relojes de la de Python (QTimer) son hilos que duermen y encolan; los de «esperar un rato después del
//! último cambio» llevan un número de vuelta para que solo cuente el último.
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
        #[qproperty(QVariant, canciones)]
        #[qproperty(QVariant, totales)]
        #[qproperty(QVariant, playlists)]
        #[qproperty(i32, vista_playlist, cxx_name = "vistaPlaylist")]
        #[qproperty(QVariant, descargas_lista, cxx_name = "descargasLista")]
        #[qproperty(i32, descargas_pendientes, cxx_name = "descargasPendientes")]
        #[qproperty(QVariant, bajando_radio, cxx_name = "bajandoRadio")]
        #[qproperty(QVariant, eq)]
        #[qproperty(QVariant, presets)]
        #[qproperty(QVariant, perfiles)]
        #[qproperty(QString, perfil_eq, cxx_name = "perfilEq")]
        #[qproperty(QString, problema_eq, cxx_name = "problemaEq")]
        #[qproperty(QVariant, salida)]
        #[qproperty(QVariant, ajustes)]
        #[qproperty(bool, escaneando)]
        #[qproperty(QVariant, en_linea, cxx_name = "enLinea")]
        #[qproperty(bool, buscando_en_linea, cxx_name = "buscandoEnLinea")]
        #[qproperty(QString, error_en_linea, cxx_name = "errorEnLinea")]
        #[qproperty(QVariant, recomendaciones)]
        #[qproperty(i32, recomendando_para, cxx_name = "recomendandoPara")]
        #[qproperty(bool, cargando_recomendaciones, cxx_name = "cargandoRecomendaciones")]
        #[qproperty(QString, error_recomendaciones, cxx_name = "errorRecomendaciones")]
        #[qproperty(QVariant, para_agregar, cxx_name = "paraAgregar")]
        #[qproperty(QVariant, nube)]
        #[qproperty(QVariant, letra)]
        #[qproperty(QVariant, para_ti, cxx_name = "paraTi")]
        #[qproperty(QVariant, para_ti_de, cxx_name = "paraTiDe")]
        #[qproperty(bool, cargando_para_ti, cxx_name = "cargandoParaTi")]
        #[qproperty(QString, error_para_ti, cxx_name = "errorParaTi")]
        #[qproperty(QVariant, avance)]
        #[qproperty(QVariant, me_gusta, cxx_name = "meGusta")]
        #[qproperty(i32, version_gustos, cxx_name = "versionGustos")]
        #[qproperty(QVariant, letras_guardadas, cxx_name = "letrasGuardadas")]
        #[qproperty(QVariant, resumen)]
        #[qproperty(QVariant, mezcla)]
        #[qproperty(QVariant, seguir)]
        #[qproperty(QVariant, novedades)]
        type Puente = super::PuenteRust;

        #[qsignal]
        fn aviso(self: Pin<&mut Puente>, texto: &QString);
        #[qsignal]
        #[cxx_name = "mostrarVentana"]
        fn mostrar_ventana(self: Pin<&mut Puente>);
        #[qsignal]
        #[cxx_name = "portadaLista"]
        fn portada_lista(self: Pin<&mut Puente>, ruta: &QString);

        // --- canciones y playlists ---
        #[qinvokable]
        fn buscar(self: Pin<&mut Puente>, texto: &QString);
        #[qinvokable]
        fn ordenar(self: Pin<&mut Puente>, orden: &QString);
        #[qinvokable]
        #[cxx_name = "verPlaylist"]
        fn ver_playlist(self: Pin<&mut Puente>, pid: i32);
        #[qinvokable]
        fn tocar(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "tocarAleatorio"]
        fn tocar_aleatorio(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "crearPlaylist"]
        fn crear_playlist(self: Pin<&mut Puente>, nombre: &QString) -> i32;
        #[qinvokable]
        #[cxx_name = "renombrarPlaylist"]
        fn renombrar_playlist(self: Pin<&mut Puente>, pid: i32, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "borrarPlaylist"]
        fn borrar_playlist(self: Pin<&mut Puente>, pid: i32);
        #[qinvokable]
        #[cxx_name = "agregarAPlaylist"]
        fn agregar_a_playlist(self: Pin<&mut Puente>, pid: i32, ruta: &QString);
        #[qinvokable]
        #[cxx_name = "buscarParaPlaylist"]
        fn buscar_para_playlist(self: Pin<&mut Puente>, pid: i32, texto: &QString);
        #[qinvokable]
        #[cxx_name = "quitarDePlaylist"]
        fn quitar_de_playlist(self: Pin<&mut Puente>, pid: i32, ruta: &QString);
        #[qinvokable]
        #[cxx_name = "quitarDeBiblioteca"]
        fn quitar_de_biblioteca(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        #[cxx_name = "abrirCarpeta"]
        fn abrir_carpeta(self: &Puente, ruta: &QString);
        #[qinvokable]
        fn escanear(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "agregarCarpeta"]
        fn agregar_carpeta(self: Pin<&mut Puente>, url: &QString);
        #[qinvokable]
        #[cxx_name = "quitarCarpeta"]
        fn quitar_carpeta(self: Pin<&mut Puente>, carpeta: &QString);
        #[qinvokable]
        #[cxx_name = "agregarArchivos"]
        fn agregar_archivos(self: Pin<&mut Puente>, urls: &QVariant);
        #[qinvokable]
        fn portada(self: Pin<&mut Puente>, ruta: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "colorDe"]
        fn color_de(self: Pin<&mut Puente>, ruta: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "alternarMeGusta"]
        fn alternar_me_gusta(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        #[cxx_name = "leGusta"]
        fn le_gusta(self: &Puente, ruta: &QString, version: i32) -> bool;
        #[qinvokable]
        #[cxx_name = "tocarDespues"]
        fn tocar_despues(self: Pin<&mut Puente>, c: &QVariant);
        #[qinvokable]
        #[cxx_name = "agregarACola"]
        fn agregar_a_cola(self: Pin<&mut Puente>, c: &QVariant);

        // --- buscar en YouTube, recomendaciones, avance y descargas ---
        #[qinvokable]
        #[cxx_name = "buscarEnLinea"]
        fn buscar_en_linea(self: Pin<&mut Puente>, texto: &QString, fuente: &QString);
        #[qinvokable]
        fn recomendar(self: Pin<&mut Puente>, pid: i32, otras: bool);
        #[qinvokable]
        #[cxx_name = "agregarRecomendada"]
        fn agregar_recomendada(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "escucharRecomendada"]
        fn escuchar_recomendada(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "escucharResultado"]
        fn escuchar_resultado(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "pararAvance"]
        fn parar_avance(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "bajarResultado"]
        fn bajar_resultado(self: Pin<&mut Puente>, i: i32, playlist: i32);
        #[qinvokable]
        fn bajar(self: Pin<&mut Puente>, url: &QString);
        #[qinvokable]
        #[cxx_name = "pararDescargas"]
        fn parar_descargas(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "limpiarDescargas"]
        fn limpiar_descargas(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "recomendarParaTi"]
        fn recomendar_para_ti(self: Pin<&mut Puente>, otras: bool);
        #[qinvokable]
        #[cxx_name = "bajarParaTi"]
        fn bajar_para_ti(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "escucharParaTi"]
        fn escuchar_para_ti(self: Pin<&mut Puente>, i: i32);
        #[qinvokable]
        #[cxx_name = "ponerRadio"]
        fn poner_radio(self: Pin<&mut Puente>, activar: bool);
        #[qinvokable]
        #[cxx_name = "iniciarRadio"]
        fn iniciar_radio(self: Pin<&mut Puente>, c: &QVariant);
        #[qinvokable]
        #[cxx_name = "bajarDeLaRadio"]
        fn bajar_de_la_radio(self: Pin<&mut Puente>, ruta: &QString);
        #[qinvokable]
        fn compartir(self: Pin<&mut Puente>, c: &QVariant, como: &QString);
        #[qinvokable]
        #[cxx_name = "compartirPlaylist"]
        fn compartir_playlist(self: Pin<&mut Puente>, como: &QString);
        #[qinvokable]
        #[cxx_name = "armarMezcla"]
        fn armar_mezcla(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "tocarMezcla"]
        fn tocar_mezcla(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "revisarNovedades"]
        fn revisar_novedades(self: Pin<&mut Puente>);
        #[qinvokable]
        fn lanzamiento(self: Pin<&mut Puente>, album: &QString, como: &QString);
        #[qinvokable]
        #[cxx_name = "bajarLetras"]
        fn bajar_letras(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "lineaActual"]
        fn linea_actual(self: &Puente, segundos: f64) -> i32;

        // --- la nube, seguir en el otro aparato y «Tu resumen» ---
        #[qinvokable]
        #[cxx_name = "iniciarSesionGoogle"]
        fn iniciar_sesion_google(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "cerrarSesionGoogle"]
        fn cerrar_sesion_google(self: Pin<&mut Puente>);
        #[qinvokable]
        fn sincronizar(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "revisarOtroAparato"]
        fn revisar_otro_aparato(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "seguirAqui"]
        fn seguir_aqui(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "descartarSeguir"]
        fn descartar_seguir(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "resumenVisto"]
        fn resumen_visto(self: Pin<&mut Puente>);

        // --- ecualizador, salida de audio y ajustes ---
        #[qinvokable]
        #[cxx_name = "ponerBanda"]
        fn poner_banda(self: Pin<&mut Puente>, i: i32, g: f64);
        #[qinvokable]
        #[cxx_name = "ponerPreset"]
        fn poner_preset(self: Pin<&mut Puente>, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "ponerPerfil"]
        fn poner_perfil(self: Pin<&mut Puente>, nombre: &QString);
        #[qinvokable]
        #[cxx_name = "actualizarSalida"]
        fn actualizar_salida(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "forzarLdac"]
        fn forzar_ldac(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "ponerLdacHq"]
        fn poner_ldac_hq(self: Pin<&mut Puente>, activar: bool);
        #[qinvokable]
        #[cxx_name = "ponerNavegador"]
        fn poner_navegador(self: Pin<&mut Puente>, navegador: &QString);
        #[qinvokable]
        #[cxx_name = "guardarVolumen"]
        fn guardar_volumen(self: Pin<&mut Puente>, v: i32);
        #[qinvokable]
        #[cxx_name = "aplicarAudio"]
        fn aplicar_audio(self: Pin<&mut Puente>);
        #[qinvokable]
        #[cxx_name = "ponerDiscord"]
        fn poner_discord(self: Pin<&mut Puente>, activar: bool);
        #[qinvokable]
        #[cxx_name = "ponerFundido"]
        fn poner_fundido(self: Pin<&mut Puente>, segundos: i32);
        #[qinvokable]
        #[cxx_name = "ponerDespertador"]
        fn poner_despertador(self: Pin<&mut Puente>, d: &QVariant);
        #[qinvokable]
        fn despertar(self: Pin<&mut Puente>, playlist: i32);

        /// Al salir: la sesión (el segundo exacto), lo que suena a la nube, mpv. Lo llama el arranque.
        #[qinvokable]
        fn cerrar(self: Pin<&mut Puente>);
    }

    impl cxx_qt::Initialize for Puente {}
    impl cxx_qt::Threading for Puente {}
}

use crate::arranque::ffi::{json_a_variante, variante_a_json};
use crate::motor::{self, con_rep, leer};
use core::pin::Pin;
use cxx_qt::{CxxQtThread, CxxQtType, Threading};
use cxx_qt_lib::{QString, QVariant};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use vortex_player::biblioteca::{Biblioteca, Cancion, Orden, id_de};
use vortex_player::cuenta_google::Cuenta;
use vortex_player::ecualizador::{self, EstadoEq};
use vortex_player::letras::LineaLrc;
use vortex_player::reproductor::Aviso;
use vortex_player::{fecha, rutas};

pub const NOMBRES_PORTADA: [&str; 8] = [
    "cover.jpg", "cover.png", "folder.jpg", "folder.png", "front.jpg", "Cover.jpg", "Folder.jpg", "AlbumArt.jpg",
];
/// Lo que llegó por la línea de comandos (enlaces o archivos): lo recibe el puente al arrancar.
pub static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

pub const NAVEGADORES: [&str; 9] = ["", "firefox", "chrome", "chromium", "brave", "edge", "opera", "vivaldi", "zen"];

/// Todo lo que la de Python guardaba en `self._…`.
pub type ColaDescargas = (VecDeque<(usize, String)>, bool);

pub struct Estado {
    pub bib: Arc<Mutex<Biblioteca>>,
    pub cuenta: Arc<Cuenta>,
    pub vivo: Arc<AtomicBool>,
    /// Sin MPRIS, sin sesión de ventana única, sin relojes de red (las pruebas).
    pub prueba: bool,
    pub sin_red: bool,
    /// Vueltas de los relojes de «esperar un rato» (solo cuenta el último).
    pub relojes: HashMap<&'static str, u64>,

    pub buscar: String,
    pub orden: String,
    pub vista: i64,
    pub canciones: Vec<Value>,
    pub playlists: Vec<Value>,
    pub portadas: HashMap<String, String>,
    pub colores: HashMap<String, String>,
    pub me_gusta: Vec<String>,
    pub gustos: HashSet<String>,
    pub version_gustos: i32,
    pub ids_biblioteca: HashSet<String>,
    pub escaneando: bool,
    pub para_agregar_de: (i64, String),

    pub descargas: Vec<Value>,
    /// Lo que falta bajar y si el hilo de las descargas está trabajando.
    pub cola_urls: Arc<Mutex<ColaDescargas>>,
    pub parar_descargas: Arc<AtomicBool>,

    pub en_linea: Vec<Value>,
    pub buscando_linea: bool,
    pub error_linea: String,
    pub busqueda_linea: u64,
    pub recom: Vec<Value>,
    pub recom_para: i64,
    pub recom_cargando: bool,
    pub recom_error: String,
    pub recom_vuelta: u64,
    pub recom_guardadas: HashMap<i64, Vec<Value>>,

    pub avance: vortex_player::avance::Avance,
    pub seguir_tras_avance: bool,

    pub nube: Map<String, Value>,
    pub nube_pendientes: HashSet<String>,
    pub nube_fallidas: HashSet<String>,
    pub nube_otra_vez: bool,

    pub eq: EstadoEq,
    pub eq_problema: String,
    pub eq_local: f64,
    pub salida: Value,
    pub audio_pendiente: bool,

    pub sesion: Map<String, Value>,
    pub restaurada: bool,
    pub letra_ruta: String,
    pub letra_lineas: Vec<LineaLrc>,
    pub letra_sincronizada: bool,
    pub letra_vuelta: u64,

    pub para_ti: Vec<Value>,
    pub para_ti_de: Vec<String>,
    pub para_ti_cargando: bool,
    pub para_ti_error: String,
    pub para_ti_vuelta: u64,
    pub letras_estado: Value,

    pub radio_vuelta: u64,
    pub bajando_radio: Vec<String>,
    pub radio_para: String,
    pub aviso_datos: bool,
    pub resumen: Value,

    pub mezcla: Map<String, Value>,
    pub mezcla_armando: bool,
    pub seguir: Map<String, Value>,
    pub sonando_subido: i64,
    pub sonando_ultimo: Option<(String, i64, bool)>,
    pub revisado_otro: f64,
    pub novedades: Map<String, Value>,
    pub despertado: String,
}

pub struct PuenteRust {
    pub e: Estado,
    canciones: QVariant,
    totales: QVariant,
    playlists: QVariant,
    vista_playlist: i32,
    descargas_lista: QVariant,
    descargas_pendientes: i32,
    bajando_radio: QVariant,
    eq: QVariant,
    presets: QVariant,
    perfiles: QVariant,
    perfil_eq: QString,
    problema_eq: QString,
    salida: QVariant,
    ajustes: QVariant,
    escaneando: bool,
    en_linea: QVariant,
    buscando_en_linea: bool,
    error_en_linea: QString,
    recomendaciones: QVariant,
    recomendando_para: i32,
    cargando_recomendaciones: bool,
    error_recomendaciones: QString,
    para_agregar: QVariant,
    nube: QVariant,
    letra: QVariant,
    para_ti: QVariant,
    para_ti_de: QVariant,
    cargando_para_ti: bool,
    error_para_ti: QString,
    avance: QVariant,
    me_gusta: QVariant,
    version_gustos: i32,
    letras_guardadas: QVariant,
    resumen: QVariant,
    mezcla: QVariant,
    seguir: QVariant,
    novedades: QVariant,
}

fn env_activa(nombre: &str) -> bool {
    std::env::var_os(nombre).is_some_and(|v| !v.is_empty())
}

impl Default for PuenteRust {
    fn default() -> Self {
        let bib = match Biblioteca::abrir(None, "ffprobe") {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Vortex Player: no pude abrir tu biblioteca: {e}");
                std::process::exit(1);
            }
        };
        let cuenta = Arc::new(Cuenta::new());
        let s = cuenta.sesion();
        let nube = json!({"correo": texto(&s, "correo"), "nombre": texto(&s, "nombre"), "trabajando": false,
                          "texto": "", "ultima": 0.0, "error": ""});
        let vacio = || json_a_variante("{}");
        let lista = || json_a_variante("[]");
        PuenteRust {
            e: Estado {
                bib: Arc::new(Mutex::new(bib)),
                cuenta,
                vivo: Arc::new(AtomicBool::new(true)),
                prueba: env_activa("VORTEX_PLAYER_PRUEBA"),
                sin_red: env_activa("VORTEX_PLAYER_SIN_RED"),
                relojes: HashMap::new(),
                buscar: String::new(),
                orden: "agregada".into(),
                vista: -1,
                canciones: Vec::new(),
                playlists: Vec::new(),
                portadas: HashMap::new(),
                colores: HashMap::new(),
                me_gusta: Vec::new(),
                gustos: HashSet::new(),
                version_gustos: 0,
                ids_biblioteca: HashSet::new(),
                escaneando: false,
                para_agregar_de: (-1, String::new()),
                descargas: Vec::new(),
                cola_urls: Arc::new(Mutex::new((VecDeque::new(), false))),
                parar_descargas: Arc::new(AtomicBool::new(false)),
                en_linea: Vec::new(),
                buscando_linea: false,
                error_linea: String::new(),
                busqueda_linea: 0,
                recom: Vec::new(),
                recom_para: -1,
                recom_cargando: false,
                recom_error: String::new(),
                recom_vuelta: 0,
                recom_guardadas: HashMap::new(),
                avance: vortex_player::avance::Avance::new(|_| {}),
                seguir_tras_avance: false,
                nube: nube.as_object().cloned().unwrap_or_default(),
                nube_pendientes: HashSet::new(),
                nube_fallidas: HashSet::new(),
                nube_otra_vez: false,
                eq: ecualizador::estado(),
                eq_problema: String::new(),
                eq_local: 0.0,
                salida: json!({}),
                audio_pendiente: false,
                sesion: leer_json("sesion.json"),
                restaurada: false,
                letra_ruta: String::new(),
                letra_lineas: Vec::new(),
                letra_sincronizada: false,
                letra_vuelta: 0,
                para_ti: Vec::new(),
                para_ti_de: Vec::new(),
                para_ti_cargando: false,
                para_ti_error: String::new(),
                para_ti_vuelta: 0,
                letras_estado: json!({"con": 0, "total": 0, "trabajando": false}),
                radio_vuelta: 0,
                bajando_radio: Vec::new(),
                radio_para: String::new(),
                aviso_datos: false,
                resumen: json!({}),
                mezcla: leer_json("mezcla.json"),
                mezcla_armando: false,
                seguir: Map::new(),
                sonando_subido: 0,
                sonando_ultimo: None,
                revisado_otro: f64::NEG_INFINITY,
                novedades: leer_json("novedades.json"),
                despertado: String::new(),
            },
            canciones: lista(),
            totales: vacio(),
            playlists: lista(),
            vista_playlist: -1,
            descargas_lista: lista(),
            descargas_pendientes: 0,
            bajando_radio: lista(),
            eq: vacio(),
            presets: lista(),
            perfiles: lista(),
            perfil_eq: QString::default(),
            problema_eq: QString::default(),
            salida: vacio(),
            ajustes: vacio(),
            escaneando: false,
            en_linea: lista(),
            buscando_en_linea: false,
            error_en_linea: QString::default(),
            recomendaciones: lista(),
            recomendando_para: -1,
            cargando_recomendaciones: false,
            error_recomendaciones: QString::default(),
            para_agregar: lista(),
            nube: vacio(),
            letra: vacio(),
            para_ti: lista(),
            para_ti_de: lista(),
            cargando_para_ti: false,
            error_para_ti: QString::default(),
            avance: vacio(),
            me_gusta: lista(),
            version_gustos: 0,
            letras_guardadas: vacio(),
            resumen: vacio(),
            mezcla: vacio(),
            seguir: vacio(),
            novedades: lista(),
        }
    }
}

// --- ayudantes ------------------------------------------------------------------------------------------

pub fn q(s: &str) -> QString {
    QString::from(s)
}

pub fn variante(v: &Value) -> QVariant {
    json_a_variante(&v.to_string())
}

/// Lo que manda QML (un objeto o una lista de JavaScript) como JSON.
pub fn de_qml(v: &QVariant) -> Value {
    serde_json::from_str(&variante_a_json(v)).unwrap_or(Value::Null)
}

pub fn texto(v: &Value, clave: &str) -> String {
    match v.get(clave) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(otro) => otro.to_string(),
    }
}

pub fn numero(v: &Value, clave: &str) -> f64 {
    match v.get(clave) {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0),
        Some(Value::String(s)) => s.trim().parse().unwrap_or(0.0),
        Some(Value::Bool(b)) => f64::from(u8::from(*b)),
        _ => 0.0,
    }
}

pub fn verdadero(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// El nombre del archivo sin extensión (el `Path(ruta).stem` de Python).
pub fn tallo(ruta: &str) -> String {
    Path::new(ruta).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
}

/// `id` de la canción o el de YouTube que trae en el nombre.
pub fn id_o_ruta(c: &Value) -> String {
    let id = texto(c, "id");
    if id.is_empty() { id_de(&texto(c, "ruta")) } else { id }
}

/// «Título · Artista» (o solo el título).
pub fn titulo_y(titulo: &str, artista: &str) -> String {
    if artista.is_empty() { titulo.to_string() } else { format!("{titulo} · {artista}") }
}

/// `file:///…` (con %XX) → ruta; lo demás, igual.
pub fn ruta_de(texto: &str) -> String {
    let Some(resto) = texto.strip_prefix("file://") else {
        return texto.to_string();
    };
    let b = resto.as_bytes();
    let mut salida = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(h) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("x"), 16)
        {
            salida.push(h);
            i += 3;
            continue;
        }
        salida.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&salida).into_owned()
}

/// La URL de un archivo (lo que da `QUrl.fromLocalFile(...).toString()` para QML).
pub fn url_de(ruta: &Path) -> String {
    format!("file://{}", vortex_player::red::escapar_ruta(&ruta.to_string_lossy()))
}

pub fn abrir(destino: &str) {
    let _ = std::process::Command::new("xdg-open")
        .arg(destino)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub fn a_la_papelera(ruta: &str) {
    let _ = std::process::Command::new("gio")
        .args(["trash", ruta])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let jpg = Path::new(ruta).with_extension("jpg");
    if jpg.exists() {
        let _ = std::process::Command::new("gio")
            .arg("trash")
            .arg(&jpg)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }
}

pub fn hay_programa(nombre: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(nombre).is_file()))
}

pub fn leer_json(nombre: &str) -> Map<String, Value> {
    std::fs::read_to_string(rutas::config().join(nombre))
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

pub fn guardar_json(nombre: &str, d: &Map<String, Value>) {
    let carpeta = rutas::config();
    let _ = std::fs::create_dir_all(&carpeta);
    let tmp = carpeta.join(format!("{nombre}.tmp"));
    if std::fs::write(&tmp, Value::Object(d.clone()).to_string()).is_ok() {
        let _ = std::fs::rename(&tmp, carpeta.join(nombre));
    }
}

pub fn guardar_ajuste(clave: &str, valor: Value) {
    let _ = Biblioteca::guardar_ajuste(clave, valor);
}

/// «AAAA-MM-DD» de hoy (hora local).
pub fn hoy() -> String {
    let l = fecha::local(fecha::ahora());
    format!("{:04}-{:02}-{:02}", l.anio, l.mes, l.dia)
}

pub fn cancion_valor(c: &Cancion, portada: &str) -> Value {
    let mut v = serde_json::to_value(c).unwrap_or_else(|_| json!({}));
    v["portada"] = Value::String(portada.to_string());
    v
}

/// Lo de una canción de tu biblioteca que piden las recomendaciones: {id, titulo, artista}.
pub fn semilla(c: &Cancion) -> Value {
    json!({"id": id_de(&c.ruta), "titulo": c.titulo, "artista": c.artista})
}

/// Corre `trabajo` en otro hilo y manda el resultado a `listo`, en el hilo de la interfaz.
pub fn lanzar<T: Send + 'static>(
    hilo: CxxQtThread<qobject::Puente>,
    trabajo: impl FnOnce() -> T + Send + 'static,
    listo: impl FnOnce(Pin<&mut qobject::Puente>, T) + Send + 'static,
) {
    std::thread::spawn(move || {
        let valor = trabajo();
        let _ = hilo.queue(move |obj| listo(obj, valor));
    });
}

/// `f` en el hilo de la interfaz dentro de `ms`.
pub fn despues(hilo: CxxQtThread<qobject::Puente>, ms: u64, f: impl FnOnce(Pin<&mut qobject::Puente>) + Send + 'static) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(ms));
        let _ = hilo.queue(f);
    });
}

/// `f` cada `ms` mientras la app siga abierta.
pub fn cada(hilo: CxxQtThread<qobject::Puente>, vivo: Arc<AtomicBool>, ms: u64, f: fn(Pin<&mut qobject::Puente>)) {
    std::thread::spawn(move || {
        while vivo.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(ms));
            if hilo.queue(f).is_err() {
                break;
            }
        }
    });
}

impl qobject::Puente {
    pub fn em(self: Pin<&mut Self>) -> &mut Estado {
        &mut self.rust_mut().get_mut().e
    }

    pub fn es(&self) -> &Estado {
        &self.rust().e
    }

    pub fn bib(&self) -> MutexGuard<'_, Biblioteca> {
        self.rust().e.bib.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn avisar(self: Pin<&mut Self>, texto: &str) {
        self.aviso(&q(texto));
    }

    /// Como `QTimer(singleShot)` que se reinicia: corre `f` dentro de `ms`, salvo que se vuelva a pedir antes.
    pub fn esperar(mut self: Pin<&mut Self>, nombre: &'static str, ms: u64, f: fn(Pin<&mut Self>)) {
        let vuelta = {
            let v = self.as_mut().em().relojes.entry(nombre).or_insert(0);
            *v += 1;
            *v
        };
        despues(self.qt_thread(), ms, move |obj| {
            if obj.es().relojes.get(nombre) == Some(&vuelta) {
                f(obj);
            }
        });
    }

    // --- sincronizar lo que ve QML --------------------------------------------------------------------------

    pub fn poner_ajustes(mut self: Pin<&mut Self>) {
        let mut a = Biblioteca::ajustes().0;
        a.insert("musica".into(), json!(rutas::musica().to_string_lossy()));
        a.insert("descargas".into(), json!(rutas::descargas().to_string_lossy()));
        a.insert("audioPendiente".into(), json!(self.es().audio_pendiente));
        a.insert("hayEq".into(), json!(ecualizador::disponible()));
        a.insert("hayYtDlp".into(), json!(hay_programa("yt-dlp")));
        a.insert("navegadores".into(), json!(NAVEGADORES));
        self.as_mut().set_ajustes(variante(&Value::Object(a)));
    }

    /// Con su estado: «biblioteca» si ya la tienes, «bajando» si se está bajando.
    pub fn con_estado(&self, lista: &[Value]) -> Vec<Value> {
        let e = self.es();
        let bajando: HashSet<String> = e
            .descargas
            .iter()
            .filter(|d| matches!(d["estado"].as_str(), Some("espera" | "bajando")))
            .map(|d| texto(d, "id"))
            .filter(|i| !i.is_empty())
            .collect();
        lista
            .iter()
            .map(|c| {
                let mut c = c.clone();
                let id = texto(&c, "id");
                c["titulo"] = json!(vortex_player::letras::titulo_limpio(&texto(&c, "titulo"), &texto(&c, "artistas")));
                c["estado"] = json!(if e.ids_biblioteca.contains(&id) {
                    "biblioteca"
                } else if bajando.contains(&id) {
                    "bajando"
                } else {
                    ""
                });
                c
            })
            .collect()
    }

    /// Las recomendadas no repiten lo que ya tienes: al bajarla, se quita de la lista.
    pub fn sin_las_tuyas(&self, lista: &[Value]) -> Vec<Value> {
        self.con_estado(lista).into_iter().filter(|c| c["estado"] != "biblioteca").collect()
    }

    pub fn poner_recomendaciones(mut self: Pin<&mut Self>) {
        let v = variante(&Value::Array(self.sin_las_tuyas(&self.es().recom)));
        self.as_mut().set_recomendaciones(v);
        let (para, cargando, error) = (self.es().recom_para, self.es().recom_cargando, self.es().recom_error.clone());
        self.as_mut().set_recomendando_para(i32::try_from(para).unwrap_or(-1));
        self.as_mut().set_cargando_recomendaciones(cargando);
        self.as_mut().set_error_recomendaciones(q(&error));
    }

    pub fn poner_en_linea(mut self: Pin<&mut Self>) {
        let v = variante(&Value::Array(self.con_estado(&self.es().en_linea)));
        self.as_mut().set_en_linea(v);
        let (b, err) = (self.es().buscando_linea, self.es().error_linea.clone());
        self.as_mut().set_buscando_en_linea(b);
        self.as_mut().set_error_en_linea(q(&err));
        self.poner_recomendaciones();
    }

    pub fn poner_para_ti(mut self: Pin<&mut Self>) {
        let v = variante(&Value::Array(self.sin_las_tuyas(&self.es().para_ti)));
        self.as_mut().set_para_ti(v);
        let de = variante(&json!(self.es().para_ti_de));
        self.as_mut().set_para_ti_de(de);
        let (c, err) = (self.es().para_ti_cargando, self.es().para_ti_error.clone());
        self.as_mut().set_cargando_para_ti(c);
        self.as_mut().set_error_para_ti(q(&err));
    }

    pub fn poner_descargas(mut self: Pin<&mut Self>) {
        let v = variante(&Value::Array(self.es().descargas.clone()));
        self.as_mut().set_descargas_lista(v);
        let pendientes = self
            .es()
            .descargas
            .iter()
            .filter(|d| matches!(d["estado"].as_str(), Some("espera" | "bajando")))
            .count();
        self.as_mut().set_descargas_pendientes(i32::try_from(pendientes).unwrap_or(i32::MAX));
        let r = variante(&json!(self.es().bajando_radio));
        self.as_mut().set_bajando_radio(r);
        self.as_mut().poner_en_linea();
        self.poner_para_ti();
    }

    pub fn poner_nube(mut self: Pin<&mut Self>, cambios: Value) {
        if let Value::Object(c) = cambios {
            self.as_mut().em().nube.extend(c);
        }
        let mut n = self.es().nube.clone();
        n.insert("configurada".into(), json!(self.es().cuenta.configurada()));
        self.as_mut().set_nube(variante(&Value::Object(n)));
    }

    pub fn poner_avance(mut self: Pin<&mut Self>) {
        let a = &self.es().avance;
        let v = json!({"idActual": a.id_actual(), "sonando": a.sonando()});
        self.as_mut().set_avance(variante(&v));
    }

    // --- canciones y playlists ------------------------------------------------------------------------------

    pub fn recargar(mut self: Pin<&mut Self>) {
        let (lista, playlists, me_gusta, ids) = {
            let e = self.es();
            let b = self.bib();
            let lista: Vec<Cancion> = if e.vista >= 0 {
                let mut l = b.canciones_de_playlist(e.vista).unwrap_or_default();
                if !e.buscar.is_empty() {
                    let q = e.buscar.to_lowercase();
                    l.retain(|c| format!("{} {} {}", c.titulo, c.artista, c.album).to_lowercase().contains(&q));
                }
                l
            } else {
                b.canciones(&e.buscar, Orden::desde_str(&e.orden)).unwrap_or_default()
            };
            let playlists: Vec<Value> = b
                .playlists()
                .unwrap_or_default()
                .iter()
                .map(|p| {
                    let mut v = serde_json::to_value(p).unwrap_or_else(|_| json!({}));
                    v["portadas"] = json!(b.primeras_de_playlist(p.id, 4).unwrap_or_default());
                    v
                })
                .collect();
            let me_gusta = b.me_gusta().unwrap_or_default();
            let ids: HashSet<String> = b
                .todas_las_rutas()
                .unwrap_or_default()
                .iter()
                .map(|r| id_de(r))
                .filter(|i| !i.is_empty() && !i.starts_with("http"))
                .collect();
            (lista, playlists, me_gusta, ids)
        };
        let canciones: Vec<Value> = lista
            .iter()
            .map(|c| cancion_valor(c, self.es().portadas.get(&c.ruta).map_or("", String::as_str)))
            .collect();
        let totales = json!({
            "cuantas": lista.len(),
            "duracion": lista.iter().map(|c| c.duracion).sum::<f64>(),
            "hires": lista.iter().filter(|c| c.nivel == "hires").count(),
            "primera": lista.first().map(|c| c.ruta.clone()).unwrap_or_default(),
        });
        {
            let e = self.as_mut().em();
            e.gustos = me_gusta.iter().cloned().collect();
            e.me_gusta = me_gusta;
            e.version_gustos += 1;
            e.ids_biblioteca = ids;
        }
        self.as_mut().set_canciones(variante(&Value::Array(canciones.clone())));
        self.as_mut().set_totales(variante(&totales));
        let vista = self.es().vista;
        self.as_mut().set_vista_playlist(i32::try_from(vista).unwrap_or(-1));
        self.as_mut().set_playlists(variante(&Value::Array(playlists.clone())));
        let mg = variante(&json!(self.es().me_gusta));
        self.as_mut().set_me_gusta(mg);
        let vg = self.es().version_gustos;
        self.as_mut().set_version_gustos(vg);
        {
            let e = self.as_mut().em();
            e.canciones = canciones;
            e.playlists = playlists;
        }
        self.as_mut().poner_en_linea();
        self.poner_para_ti();
    }

    fn buscar(mut self: Pin<&mut Self>, texto_: &QString) {
        self.as_mut().em().buscar = texto_.to_string();
        self.recargar();
    }

    fn ordenar(mut self: Pin<&mut Self>, orden: &QString) {
        self.as_mut().em().orden = orden.to_string();
        self.recargar();
    }

    fn ver_playlist(mut self: Pin<&mut Self>, pid: i32) {
        self.as_mut().em().vista = i64::from(pid);
        self.as_mut().recargar();
        if pid >= 0 {
            self.recomendar(pid, false);
        }
    }

    pub fn origen(&self) -> String {
        let e = self.es();
        e.playlists
            .iter()
            .find(|p| p["id"].as_i64() == Some(e.vista))
            .map_or_else(|| "Tu biblioteca".to_string(), |p| texto(p, "nombre"))
    }

    fn tocar(self: Pin<&mut Self>, i: i32) {
        let origen = self.origen();
        let lista = self.es().canciones.clone();
        con_rep(|r| {
            r.poner_origen(&origen);
            r.tocar(&lista, i64::from(i));
        });
    }

    fn tocar_aleatorio(self: Pin<&mut Self>) {
        let lista = self.es().canciones.clone();
        if lista.is_empty() {
            return;
        }
        let origen = self.origen();
        let i = rand::random_range(0..lista.len());
        con_rep(|r| {
            r.poner_origen(&origen);
            if !r.aleatorio() {
                r.alternar_aleatorio();
            }
            r.tocar(&lista, i64::try_from(i).unwrap_or(0));
        });
    }

    fn crear_playlist(mut self: Pin<&mut Self>, nombre: &QString) -> i32 {
        let pid = self.bib().crear_playlist(&nombre.to_string(), "").unwrap_or(-1);
        self.as_mut().recargar();
        self.cambio_local();
        i32::try_from(pid).unwrap_or(-1)
    }

    fn renombrar_playlist(mut self: Pin<&mut Self>, pid: i32, nombre: &QString) {
        let _ = self.bib().renombrar_playlist(i64::from(pid), &nombre.to_string());
        self.as_mut().recargar();
        self.cambio_local();
    }

    fn borrar_playlist(mut self: Pin<&mut Self>, pid: i32) {
        let _ = self.bib().borrar_playlist(i64::from(pid), true);
        if self.es().vista == i64::from(pid) {
            self.as_mut().em().vista = -1;
        }
        self.as_mut().recargar();
        self.cambio_local();
    }

    fn agregar_a_playlist(mut self: Pin<&mut Self>, pid: i32, ruta: &QString) {
        let _ = self.bib().agregar_a_playlist(i64::from(pid), &ruta.to_string());
        self.as_mut().recargar();
        self.as_mut().cambio_local();
        self.as_mut().avisar("Agregada a la playlist");
        let (p, t) = self.es().para_agregar_de.clone();
        if p == i64::from(pid) {
            self.buscar_para_playlist(pid, &q(&t));
        }
    }

    /// Dentro de una playlist, «Agregar canciones» busca en tu biblioteca (como Spotify).
    fn buscar_para_playlist(mut self: Pin<&mut Self>, pid: i32, texto_: &QString) {
        let t = texto_.to_string();
        self.as_mut().em().para_agregar_de = (i64::from(pid), t.clone());
        let lista = if pid >= 0 { self.bib().para_agregar(i64::from(pid), &t, 60).unwrap_or_default() } else { Vec::new() };
        self.set_para_agregar(variante(&Value::Array(lista)));
    }

    fn quitar_de_playlist(mut self: Pin<&mut Self>, pid: i32, ruta: &QString) {
        let _ = self.bib().quitar_de_playlist(i64::from(pid), &ruta.to_string());
        self.as_mut().recargar();
        self.cambio_local();
    }

    /// La quita de la biblioteca y, si la bajó Vortex Player, también borra el archivo (a la papelera).
    fn quitar_de_biblioteca(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        let c = self.bib().cancion(&ruta).ok().flatten();
        if c.is_some_and(|c| c.origen == "youtube") {
            a_la_papelera(&ruta);
            let _ = self.bib().anotar_cancion_borrada(&id_de(&ruta)); // también se quita de la nube y del cel
        }
        let _ = self.bib().quitar(&ruta);
        con_rep(|r| r.quitar_de_cola(&ruta));
        self.as_mut().recargar();
        self.cambio_local();
    }

    fn abrir_carpeta(&self, ruta: &QString) {
        if let Some(p) = Path::new(&ruta.to_string()).parent() {
            abrir(&p.to_string_lossy());
        }
    }

    // --- agregar música: carpetas y archivos -----------------------------------------------------------------

    pub fn escanear(mut self: Pin<&mut Self>) {
        if self.es().escaneando {
            return;
        }
        self.as_mut().em().escaneando = true;
        self.as_mut().set_escaneando(true);
        let bib = self.es().bib.clone();
        lanzar(
            self.qt_thread(),
            move || {
                let b = bib.lock().unwrap_or_else(|e| e.into_inner());
                b.escanear().map(|r| (r.nuevas, r.total)).unwrap_or((0, 0))
            },
            |obj, (nuevas, total)| obj.fin_escaneo(nuevas, total),
        );
    }

    pub fn fin_escaneo(mut self: Pin<&mut Self>, nuevas: usize, total: usize) {
        self.as_mut().em().escaneando = false;
        self.as_mut().set_escaneando(false);
        self.as_mut().recargar();
        if numero(&Value::Object(self.es().nube.clone()), "ultima") == 0.0 {
            self.as_mut().sincronizar();
        }
        if nuevas > 0 && total > nuevas {
            self.as_mut().avisar(&format!("{nuevas} canciones nuevas en tu biblioteca"));
        }
        despues(self.qt_thread(), 3000, |obj| obj.bajar_letras());
    }

    fn agregar_carpeta(mut self: Pin<&mut Self>, url: &QString) {
        let carpeta = ruta_de(&url.to_string());
        let mut carpetas = Biblioteca::ajustes().carpetas();
        if !carpeta.is_empty() && !carpetas.contains(&carpeta) && Path::new(&carpeta) != rutas::musica() {
            carpetas.push(carpeta);
            guardar_ajuste("carpetas", json!(carpetas));
            self.as_mut().poner_ajustes();
        }
        self.escanear();
    }

    fn quitar_carpeta(mut self: Pin<&mut Self>, carpeta: &QString) {
        let carpeta = carpeta.to_string();
        let quedan: Vec<String> = Biblioteca::ajustes().carpetas().into_iter().filter(|c| *c != carpeta).collect();
        guardar_ajuste("carpetas", json!(quedan));
        {
            let b = self.bib();
            for c in b.canciones("", Orden::Agregada).unwrap_or_default() {
                if Path::new(&c.ruta).starts_with(&carpeta) {
                    let _ = b.quitar(&c.ruta);
                }
            }
        }
        self.as_mut().poner_ajustes();
        self.recargar();
    }

    /// Archivos sueltos (o arrastrados a la ventana): se agregan donde están, sin copiarlos.
    fn agregar_archivos(self: Pin<&mut Self>, urls: &QVariant) {
        let rutas_: Vec<PathBuf> = match de_qml(urls) {
            Value::Array(l) => l.iter().filter_map(|u| u.as_str()).map(|u| PathBuf::from(ruta_de(u))).collect(),
            Value::String(u) => vec![PathBuf::from(ruta_de(&u))],
            _ => Vec::new(),
        };
        self.agregar_rutas(rutas_);
    }

    pub fn agregar_rutas(self: Pin<&mut Self>, rutas_: Vec<PathBuf>) {
        let bib = self.es().bib.clone();
        lanzar(
            self.qt_thread(),
            move || {
                let mut n = 0;
                for r in Biblioteca::archivos_de(&rutas_) {
                    let b = bib.lock().unwrap_or_else(|e| e.into_inner());
                    n += usize::from(b.agregar_archivo(&r, "").ok().flatten().is_some());
                }
                n
            },
            |obj, n| obj.fin_escaneo(n, n + 1),
        );
    }

    // --- portadas --------------------------------------------------------------------------------------------

    /// URL de la portada: la .jpg que dejó yt-dlp, la del disco (cover.jpg…) o la que trae el archivo adentro
    /// (se saca una vez con ffmpeg a la caché). "" si no tiene.
    pub fn portada_de(mut self: Pin<&mut Self>, ruta: &str) -> String {
        if ruta.is_empty() {
            return String::new();
        }
        if let Some(u) = self.es().portadas.get(ruta) {
            return u.clone();
        }
        let p = Path::new(ruta);
        let mut candidatas = vec![p.with_extension("jpg"), p.with_extension("png")];
        if let Some(padre) = p.parent() {
            candidatas.extend(NOMBRES_PORTADA.iter().map(|n| padre.join(n)));
        }
        if let Some(c) = candidatas.into_iter().find(|c| c.is_file()) {
            let u = url_de(&c);
            self.as_mut().em().portadas.insert(ruta.to_string(), u.clone());
            return u;
        }
        let cache = rutas::cache().join("portadas").join(format!("{}.jpg", sha1_hex(ruta.as_bytes())));
        if cache.is_file() {
            let u = url_de(&cache);
            self.as_mut().em().portadas.insert(ruta.to_string(), u.clone());
            return u;
        }
        self.as_mut().em().portadas.insert(ruta.to_string(), String::new());
        let tiene = self.bib().cancion(ruta).ok().flatten().is_some_and(|c| c.portada);
        if tiene && hay_programa("ffmpeg") {
            let ruta = ruta.to_string();
            lanzar(
                self.qt_thread(),
                {
                    let ruta = ruta.clone();
                    let cache = cache.clone();
                    move || {
                        if let Some(c) = cache.parent() {
                            let _ = std::fs::create_dir_all(c);
                        }
                        let ok = std::process::Command::new("ffmpeg")
                            .args(["-v", "quiet", "-y", "-i", &ruta, "-an", "-vf", "scale=600:-2", "-frames:v", "1"])
                            .arg(&cache)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status()
                            .is_ok_and(|s| s.success());
                        ok && cache.is_file()
                    }
                },
                move |mut obj, ok| {
                    if ok {
                        obj.as_mut().em().portadas.insert(ruta.clone(), url_de(&cache));
                        obj.portada_lista(&q(&ruta));
                    }
                },
            );
        }
        String::new()
    }

    fn portada(self: Pin<&mut Self>, ruta: &QString) -> QString {
        q(&self.portada_de(&ruta.to_string()))
    }

    /// El promedio de los pixeles con más color de la portada, oscurecido para que se lea el texto.
    fn color_de(mut self: Pin<&mut Self>, ruta: &QString) -> QString {
        let ruta = ruta.to_string();
        if let Some(c) = self.es().colores.get(&ruta) {
            return q(c);
        }
        let url = self.as_mut().portada_de(&ruta);
        if !url.starts_with("file://") {
            return QString::default();
        }
        let color = crate::arranque::ffi::color_de_imagen(&ruta_de(&url));
        if color.is_empty() {
            return QString::default();
        }
        self.as_mut().em().colores.insert(ruta, color.clone());
        q(&color)
    }

    // --- ❤️ «Tus me gusta» y la cola -------------------------------------------------------------------------

    fn alternar_me_gusta(mut self: Pin<&mut Self>, ruta: &QString) {
        let ruta = ruta.to_string();
        if ruta.is_empty() {
            return;
        }
        let gusta = self.bib().alternar_me_gusta(&ruta).unwrap_or(false);
        self.as_mut().recargar();
        self.as_mut().cambio_local();
        self.avisar(if gusta { "Agregada a Tus me gusta" } else { "Quitada de Tus me gusta" });
    }

    fn le_gusta(&self, ruta: &QString, _version: i32) -> bool {
        self.es().gustos.contains(&ruta.to_string())
    }

    fn tocar_despues(self: Pin<&mut Self>, c: &QVariant) {
        let c = de_qml(c);
        con_rep(|r| r.tocar_despues(&c));
        self.avisar("Suena después de esta");
    }

    fn agregar_a_cola(self: Pin<&mut Self>, c: &QVariant) {
        let c = de_qml(c);
        con_rep(|r| r.agregar_a_cola(&c));
        self.avisar("Agregada a la cola");
    }

    // --- lo que avisa el reproductor -------------------------------------------------------------------------

    pub fn al_reproductor(mut self: Pin<&mut Self>, avisos: Vec<Aviso>) {
        for a in avisos {
            match a {
                Aviso::QuiereSonar => {
                    // Si le das play a tu música mientras suena un avance, el avance se calla
                    if !self.es().avance.id_actual().is_empty() {
                        self.as_mut().em().seguir_tras_avance = false;
                        self.es().avance.parar();
                    }
                }
                Aviso::Cancion => self.as_mut().cambio_cancion(),
                Aviso::Estado => {
                    self.as_mut().guardar_sesion();
                    if leer(|r| r.pausado()) {
                        self.as_mut().esperar("sonando", 2500, |o| o.subir_sonando(0));
                    }
                }
                Aviso::SeAcaba => self.as_mut().pedir_radio(),
                Aviso::Escuchada(c, s) => self.as_mut().anotar_escucha(&c, s),
                _ => {}
            }
        }
    }

    pub fn anotar_escucha(mut self: Pin<&mut Self>, c: &Value, segundos: f64) {
        let _ = self.bib().anotar_escucha_json(c, segundos, None);
        self.as_mut().em().resumen = json!({});
        self.poner_resumen();
    }

    // --- arranque y salida -----------------------------------------------------------------------------------

    fn cerrar(mut self: Pin<&mut Self>) {
        if !self.es().vivo.swap(false, Ordering::SeqCst) {
            return;
        }
        self.es().avance.parar();
        self.as_mut().guardar_sesion(); // el segundo exacto donde te quedaste
        self.as_mut().al_salir(); // y en la nube, para seguir en el otro aparato
        for a in motor::cerrar() {
            if let Aviso::Escuchada(c, s) = a {
                let _ = self.bib().anotar_escucha_json(&c, s, None);
            }
        }
    }
}

/// SHA-1 en hexadecimal (el nombre de la portada en la caché, igual que `hashlib.sha1` en la de Python).
pub fn sha1_hex(datos: &[u8]) -> String {
    let mut h: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let mut m = datos.to_vec();
    let bits = (datos.len() as u64).wrapping_mul(8);
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&bits.to_be_bytes());
    for bloque in m.chunks(64) {
        let mut w = [0u32; 80];
        for (i, p) in bloque.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([p[0], p[1], p[2], p[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod pruebas {
    #[test]
    fn sha1_como_python() {
        assert_eq!(super::sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(super::sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    }

    #[test]
    fn rutas_de_qml() {
        assert_eq!(super::ruta_de("file:///m%C3%BAsica/a%20b.flac"), "/música/a b.flac");
        assert_eq!(super::ruta_de("/ya/es/ruta"), "/ya/es/ruta");
    }
}
