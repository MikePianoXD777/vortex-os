//! Lo que comparten los cuatro objetos de QML (`ia`, `modelos`, `chat`, `boveda`): el `Puente` de
//! `puente.py`. Un solo Ollama, la configuración, y un directorio de «hilos de Qt» para que cualquiera (también
//! un hilo de trabajo) pueda pedirle algo a otro objeto: la petición se encola y corre en el hilo de la
//! interfaz, en orden.

use crate::boveda::qobject::Boveda;
use crate::chat::qobject::Chat;
use crate::general::qobject::General;
use crate::modelos::qobject::Modelos;
use core::pin::Pin;
use cxx_qt_lib::QString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use vortex_local_ai::config::Config;
use vortex_local_ai::memoria::Memoria;
use vortex_local_ai::ollama::{ErrorOllama, Ollama};
use vortex_local_ai::rutas::Rutas;
use vortex_local_ai::vault::{ErrorVault, Vault};

/// Lo que un objeto necesita saber de otro sin esperarlo en el hilo de Qt.
#[derive(Default)]
pub struct Compartido {
    /// Los modelos de chat instalados (`Modelos.nombres_chat`).
    pub nombres_chat: Vec<String>,
    /// El id del modelo recomendado (`Modelos._recomendado`).
    pub recomendado: String,
    /// La carpeta del vault de ahora (`Boveda.raiz`).
    pub raiz: PathBuf,
}

pub struct Comun {
    pub ollama: Arc<Ollama>,
    pub rutas: Rutas,
    pub config: Mutex<Config>,
    pub compartido: Mutex<Compartido>,
}

pub fn comun() -> &'static Comun {
    static COMUN: OnceLock<Comun> = OnceLock::new();
    COMUN.get_or_init(|| {
        let rutas = Rutas::actual();
        Comun {
            ollama: Arc::new(Ollama::new(None, None)),
            config: Mutex::new(Config::new(rutas.config())),
            compartido: Mutex::new(Compartido {
                raiz: rutas.vault_por_defecto.clone(),
                ..Compartido::default()
            }),
            rutas,
        }
    })
}

/// Una `Memoria` nueva con su propio `Vault` (cada hilo la suya: SQLite y la caché del vault no se
/// comparten entre hilos).
pub fn nueva_memoria() -> Result<Memoria, ErrorVault> {
    let c = comun();
    let raiz = c.compartido.lock().unwrap().raiz.clone();
    let modelo = c.config.lock().unwrap().texto("modelo_embeddings");
    Ok(Memoria::new(
        Vault::new(&raiz)?,
        c.ollama.clone(),
        (!modelo.is_empty()).then_some(modelo),
        &c.rutas.cache,
    ))
}

// --- mensajes de error (`_mensaje_error` de puente.py) ---

pub fn msg_ollama(e: &ErrorOllama) -> String {
    e.mensaje.clone()
}

pub fn msg_vault(e: &ErrorVault) -> String {
    match e {
        ErrorVault::Existe(_) => "Ya hay una nota con ese nombre.".into(),
        ErrorVault::NoExiste(_) => {
            "Esa nota ya no existe (¿la borraste o la moviste fuera de la app?).".into()
        }
        ErrorVault::Permiso(_) => "No tengo permiso para escribir ahí.".into(),
        ErrorVault::Ruta(s) if !s.is_empty() => s.clone(),
        ErrorVault::Ruta(s) | ErrorVault::Otro(s) => format!("Algo salió mal: {s}"),
    }
}

pub fn msg_io(e: &std::io::Error) -> String {
    msg_vault(&ErrorVault::from(std::io::Error::new(e.kind(), e.to_string())))
}

// --- el directorio de hilos ---

struct Hilos {
    general: Option<cxx_qt::CxxQtThread<General>>,
    modelos: Option<cxx_qt::CxxQtThread<Modelos>>,
    chat: Option<cxx_qt::CxxQtThread<Chat>>,
    boveda: Option<cxx_qt::CxxQtThread<Boveda>>,
}

static HILOS: Mutex<Hilos> = Mutex::new(Hilos {
    general: None,
    modelos: None,
    chat: None,
    boveda: None,
});

pub fn registrar_general(h: cxx_qt::CxxQtThread<General>) {
    HILOS.lock().unwrap().general = Some(h);
}
pub fn registrar_modelos(h: cxx_qt::CxxQtThread<Modelos>) {
    HILOS.lock().unwrap().modelos = Some(h);
}
pub fn registrar_chat(h: cxx_qt::CxxQtThread<Chat>) {
    HILOS.lock().unwrap().chat = Some(h);
}
pub fn registrar_boveda(h: cxx_qt::CxxQtThread<Boveda>) {
    HILOS.lock().unwrap().boveda = Some(h);
}

/// Corre `f` en el hilo de la interfaz, sobre `ia`.
pub fn en_general(f: impl FnOnce(Pin<&mut General>) + Send + 'static) {
    let h = HILOS.lock().unwrap().general.clone();
    if let Some(h) = h {
        let _ = h.queue(f);
    }
}
pub fn en_modelos(f: impl FnOnce(Pin<&mut Modelos>) + Send + 'static) {
    let h = HILOS.lock().unwrap().modelos.clone();
    if let Some(h) = h {
        let _ = h.queue(f);
    }
}
pub fn en_chat(f: impl FnOnce(Pin<&mut Chat>) + Send + 'static) {
    let h = HILOS.lock().unwrap().chat.clone();
    if let Some(h) = h {
        let _ = h.queue(f);
    }
}
pub fn en_boveda(f: impl FnOnce(Pin<&mut Boveda>) + Send + 'static) {
    let h = HILOS.lock().unwrap().boveda.clone();
    if let Some(h) = h {
        let _ = h.queue(f);
    }
}

/// El aviso de la ventana (`ia.aviso`): texto y tipo (`info`, `error` o `exito`).
pub fn avisar(texto: impl Into<String>, tipo: &'static str) {
    let texto = texto.into();
    en_general(move |g| g.aviso(&QString::from(texto.as_str()), &QString::from(tipo)));
}
