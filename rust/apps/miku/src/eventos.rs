//! Los eventos con los que el cerebro (Claude o cualquier IA) le cuenta al asistente qué pasa, iguales en
//! los dos para que el asistente no distinga entre uno y otro.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum Evento {
    /// Claude arrancó.
    Inicio,
    /// Un trozo de la respuesta.
    Texto(String),
    /// Va a mirar o a hacer algo (nombre completo `mcp__…`).
    Herramienta(String),
    /// Terminó un bloque de texto (hay que vaciar la última frase).
    BloqueFin,
    /// La respuesta completa.
    Listo(String),
    /// Algo falló: el mensaje ya está en español. Las claves «activar» y «sesion» piden un botón.
    Error { clave: String, mensaje: String },
}

/// Quién escucha: `(etiqueta, evento)`. La etiqueta vuelve en cada evento para descartar los viejos.
pub type AlEvento = Arc<dyn Fn(u64, Evento) + Send + Sync>;

/// Lo que comparten los dos cerebros con el asistente.
pub trait Cerebro: Send + Sync {
    /// «ok» si se puede hablar; si no, (clave, mensaje en español con qué hacer).
    fn comprobar(&self) -> (String, String);
    /// Hace una pregunta y avisa del resultado (bloquea: se corre en un hilo).
    fn preguntar(&self, texto: &str, etiqueta: u64);
    fn cancelar(&self);
    /// Termina la conversación y borra su rastro.
    fn olvidar(&self);
    /// Cómo se siente ahora (`corazon.contexto`), va con su personalidad.
    fn poner_sentimientos(&self, texto: &str);
    /// El socket del permiso de las acciones.
    fn poner_autorizar(&self, ruta: &str);
    fn poner_internet(&self, si: bool);
    /// Una acción espera tu clic o está corriendo: no cortar a Claude.
    fn ocupado(&self) -> Arc<AtomicBool>;
    fn agregar_persona(&self, texto: &str);
}

/// Un cerebro de mentira que solo recoge los eventos (las pruebas).
#[derive(Default, Clone)]
pub struct Recolector(pub Arc<Mutex<Vec<(u64, Evento)>>>);

impl Recolector {
    pub fn al_evento(&self) -> AlEvento {
        let v = self.0.clone();
        Arc::new(move |e, ev| v.lock().unwrap().push((e, ev)))
    }
    pub fn eventos(&self) -> Vec<(u64, Evento)> {
        self.0.lock().unwrap().clone()
    }
    pub fn texto(&self) -> String {
        self.eventos()
            .into_iter()
            .filter_map(|(_, e)| if let Evento::Texto(t) = e { Some(t) } else { None })
            .collect()
    }
}
