//! MPRIS (como `mpris.py`): Vortex Player se anuncia en D-Bus como reproductor
//! (`org.mpris.MediaPlayer2.vortex_player`). Así lo ven el panel de música del escritorio (con «Vía Vortex
//! Player», la portada, la barra de tiempo y los botones), las teclas multimedia y cualquier otro control.
//!
//! Va en su hilo con zbus (sin tokio). La ventana le pasa lo que suena con [`Mpris::poner`] y recibe lo que
//! pidan los controles como [`OrdenMpris`] por un canal. Sin bus de sesión, la app funciona igual sin MPRIS.

use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::{ObjectPath, OwnedVariant as OwnedValue, Variant as Zv};

use crate::{cripto, red};

pub const RAIZ: &str = "org.mpris.MediaPlayer2";
pub const NOMBRE: &str = "org.mpris.MediaPlayer2.vortex_player";
pub const RUTA: &str = "/org/mpris/MediaPlayer2";

/// Lo que piden los controles del escritorio.
#[derive(Debug, Clone, PartialEq)]
pub enum OrdenMpris {
    Siguiente,
    Anterior,
    Pausar,
    Alternar,
    Seguir,
    /// A ese segundo.
    Buscar(f64),
    Volumen(i64),
    Aleatorio(bool),
    /// «no», «todo» o «una».
    Repetir(String),
    Levantar,
    Salir,
}

/// Lo que suena (lo pone la ventana).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EstadoMpris {
    pub cancion: Value,
    /// URL de la portada («» si no hay).
    pub portada: String,
    pub pausado: bool,
    pub repetir: String,
    pub aleatorio: bool,
    pub volumen: i64,
    pub posicion: f64,
    /// La duración que dio mpv (para el panel cuando la canción no la trae).
    pub duracion: f64,
    pub hay_siguiente: bool,
    pub hay_anterior: bool,
}

impl EstadoMpris {
    fn hay(&self) -> bool {
        self.cancion.as_object().is_some_and(|m| !m.is_empty())
    }

    pub fn estado_texto(&self) -> &'static str {
        if !self.hay() {
            "Stopped"
        } else if self.pausado {
            "Paused"
        } else {
            "Playing"
        }
    }

    pub fn repetir_texto(&self) -> &'static str {
        match self.repetir.as_str() {
            "todo" => "Playlist",
            "una" => "Track",
            _ => "None",
        }
    }
}

/// Un metadato, antes de pasarlo a D-Bus.
#[derive(Debug, Clone, PartialEq)]
pub enum Meta {
    Ruta(String),
    Entero(i64),
    Texto(String),
    Lista(Vec<String>),
}

fn uri(ruta: &str) -> String {
    if ruta.is_empty() { String::new() } else { format!("file://{}", red::escapar_ruta(ruta)) }
}

/// Los xesam/mpris de la canción.
pub fn metadatos(c: &Value, portada: &str) -> Vec<(&'static str, Meta)> {
    if !c.as_object().is_some_and(|m| !m.is_empty()) {
        return Vec::new();
    }
    let campo = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or("");
    let ruta = campo("ruta");
    let n = u64::from_str_radix(&cripto::hex(&cripto::sha1(ruta.as_bytes()))[..12], 16).unwrap_or(0) % 1_000_000_000_000;
    let titulo = Some(campo("titulo"))
        .filter(|t| !t.is_empty())
        .map(String::from)
        .unwrap_or_else(|| Path::new(ruta).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let mut artistas: Vec<String> =
        campo("artista").split(',').map(str::trim).filter(|a| !a.is_empty()).map(String::from).collect();
    if artistas.is_empty() {
        artistas.push(String::new());
    }
    let mut m = vec![
        ("mpris:trackid", Meta::Ruta(format!("/org/vortex/player/cancion/{n}"))),
        ("mpris:length", Meta::Entero((c.get("duracion").and_then(Value::as_f64).unwrap_or(0.0) * 1_000_000.0) as i64)),
        ("xesam:title", Meta::Texto(titulo)),
        ("xesam:artist", Meta::Lista(artistas)),
        ("xesam:album", Meta::Texto(campo("album").into())),
        ("xesam:url", Meta::Texto(uri(ruta))),
    ];
    if !portada.is_empty() {
        m.push(("mpris:artUrl", Meta::Texto(portada.into())));
    }
    m
}

fn a_dbus(m: Vec<(&'static str, Meta)>) -> HashMap<String, OwnedValue> {
    m.into_iter()
        .filter_map(|(k, v)| {
            let z = match v {
                Meta::Ruta(r) => Zv::ObjectPath(ObjectPath::try_from(r).ok()?),
                Meta::Entero(i) => Zv::from(i),
                Meta::Texto(t) => Zv::from(t),
                Meta::Lista(l) => Zv::from(l),
            };
            Some((k.to_string(), OwnedValue::try_from(z).ok()?))
        })
        .collect()
}

/// La canción con la duración de mpv si no traía una: sin duración, el panel no deja elegir el tiempo.
fn con_duracion(e: &EstadoMpris) -> Value {
    let mut c = e.cancion.clone();
    let trae = c.get("duracion").and_then(Value::as_f64).is_some_and(|d| d > 0.0);
    if !trae && e.duracion > 0.0 && let Some(m) = c.as_object_mut() {
        m.insert("duracion".into(), Value::from(e.duracion));
    }
    c
}

type Compartido = Arc<Mutex<EstadoMpris>>;

fn leer(e: &Compartido) -> EstadoMpris {
    e.lock().unwrap_or_else(|x| x.into_inner()).clone()
}

struct Raiz {
    orden: Sender<OrdenMpris>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl Raiz {
    fn raise(&self) {
        let _ = self.orden.send(OrdenMpris::Levantar);
    }
    fn quit(&self) {
        let _ = self.orden.send(OrdenMpris::Salir);
    }
    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn can_raise(&self) -> bool {
        true
    }
    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false
    }
    #[zbus(property)]
    fn identity(&self) -> &str {
        "Vortex Player"
    }
    #[zbus(property)]
    fn desktop_entry(&self) -> &str {
        "vortex-player"
    }
    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<String> {
        vec!["file".into()]
    }
    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<String> {
        ["audio/flac", "audio/x-wav", "audio/mpeg", "audio/ogg", "audio/opus", "audio/mp4", "audio/x-aiff"]
            .map(String::from)
            .to_vec()
    }
}

struct Jugador {
    estado: Compartido,
    orden: Sender<OrdenMpris>,
}

impl Jugador {
    fn mandar(&self, o: OrdenMpris) {
        let _ = self.orden.send(o);
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl Jugador {
    fn next(&self) {
        self.mandar(OrdenMpris::Siguiente);
    }
    fn previous(&self) {
        self.mandar(OrdenMpris::Anterior);
    }
    fn pause(&self) {
        self.mandar(OrdenMpris::Pausar);
    }
    fn play_pause(&self) {
        self.mandar(OrdenMpris::Alternar);
    }
    fn stop(&self) {
        self.mandar(OrdenMpris::Pausar);
        self.mandar(OrdenMpris::Buscar(0.0));
    }
    fn play(&self) {
        self.mandar(OrdenMpris::Seguir);
    }
    async fn seek(&self, offset: i64, #[zbus(signal_emitter)] em: SignalEmitter<'_>) {
        let nueva = {
            let mut e = self.estado.lock().unwrap_or_else(|x| x.into_inner());
            e.posicion = (e.posicion + offset as f64 / 1_000_000.0).max(0.0);
            e.posicion
        };
        self.mandar(OrdenMpris::Buscar(nueva));
        let _ = Self::seeked(&em, (nueva * 1_000_000.0) as i64).await;
    }
    async fn set_position(&self, _pista: ObjectPath<'_>, posicion: i64, #[zbus(signal_emitter)] em: SignalEmitter<'_>) {
        let s = (posicion as f64 / 1_000_000.0).max(0.0);
        self.estado.lock().unwrap_or_else(|x| x.into_inner()).posicion = s;
        self.mandar(OrdenMpris::Buscar(s));
        let _ = Self::seeked(&em, posicion).await;
    }
    fn open_uri(&self, _uri: &str) {}

    #[zbus(signal)]
    async fn seeked(em: &SignalEmitter<'_>, posicion: i64) -> zbus::Result<()>;

    #[zbus(property)]
    fn playback_status(&self) -> &'static str {
        leer(&self.estado).estado_texto()
    }
    #[zbus(property)]
    fn loop_status(&self) -> &'static str {
        leer(&self.estado).repetir_texto()
    }
    #[zbus(property)]
    fn set_loop_status(&mut self, v: &str) {
        let quiere = match v {
            "Playlist" => "todo",
            "Track" => "una",
            _ => "no",
        };
        self.mandar(OrdenMpris::Repetir(quiere.into()));
    }
    #[zbus(property)]
    fn shuffle(&self) -> bool {
        leer(&self.estado).aleatorio
    }
    #[zbus(property)]
    fn set_shuffle(&mut self, v: bool) {
        if v != leer(&self.estado).aleatorio {
            self.mandar(OrdenMpris::Aleatorio(v));
        }
    }
    #[zbus(property)]
    fn rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn minimum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn maximum_rate(&self) -> f64 {
        1.0
    }
    #[zbus(property)]
    fn metadata(&self) -> HashMap<String, OwnedValue> {
        let e = leer(&self.estado);
        a_dbus(metadatos(&con_duracion(&e), &e.portada))
    }
    #[zbus(property)]
    fn volume(&self) -> f64 {
        leer(&self.estado).volumen as f64 / 100.0
    }
    #[zbus(property)]
    fn set_volume(&mut self, v: f64) {
        self.mandar(OrdenMpris::Volumen((v * 100.0) as i64));
    }
    #[zbus(property(emits_changed_signal = "false"))]
    fn position(&self) -> i64 {
        (leer(&self.estado).posicion * 1_000_000.0) as i64
    }
    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        leer(&self.estado).hay_siguiente
    }
    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        leer(&self.estado).hay_anterior
    }
    #[zbus(property)]
    fn can_play(&self) -> bool {
        leer(&self.estado).hay()
    }
    #[zbus(property)]
    fn can_pause(&self) -> bool {
        leer(&self.estado).hay()
    }
    #[zbus(property)]
    fn can_seek(&self) -> bool {
        leer(&self.estado).hay()
    }
    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

/// Qué grupos de propiedades cambiaron.
#[derive(Debug, Clone, Copy)]
enum Cambio {
    Cancion,
    Estado,
}

/// MPRIS en su hilo. Se va del bus al soltarlo.
pub struct Mpris {
    estado: Compartido,
    aviso: futures_channel::mpsc::UnboundedSender<Option<Cambio>>,
}

impl Mpris {
    /// En el bus de sesión (o en `direccion`, para las pruebas). `None` si no hay bus.
    pub fn arrancar(direccion: Option<&str>, orden: Sender<OrdenMpris>) -> Option<Self> {
        let estado: Compartido = Arc::default();
        let (aviso, mut rx) = futures_channel::mpsc::unbounded::<Option<Cambio>>();
        let (listo_tx, listo_rx) = std::sync::mpsc::channel();
        let e2 = estado.clone();
        let direccion = direccion.map(String::from);
        std::thread::Builder::new()
            .name("vortex-player-mpris".into())
            .spawn(move || {
                async_io::block_on(async move {
                    let b = match &direccion {
                        Some(d) => zbus::connection::Builder::address(d.as_str()),
                        None => zbus::connection::Builder::session(),
                    };
                    let c = b
                        .name(NOMBRE)
                        .serve_at(RUTA, Raiz { orden: orden.clone() })
                        .serve_at(RUTA, Jugador { estado: e2, orden })
                        .build()
                        .await;
                    let c = match c {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Vortex Player: sin MPRIS ({e})");
                            let _ = listo_tx.send(false);
                            return;
                        }
                    };
                    let _ = listo_tx.send(true);
                    let Ok(j) = c.object_server().interface::<_, Jugador>(RUTA).await else { return };
                    while let Some(Some(cambio)) = rx.next().await {
                        let em = j.signal_emitter();
                        let i = j.get().await;
                        let _ = match cambio {
                            Cambio::Cancion => {
                                let _ = i.metadata_changed(em).await;
                                // Sin esto, si Vortex Player se abre sin canción el panel se queda con
                                // CanSeek = false y su barra no deja elegir el tiempo.
                                let _ = i.can_seek_changed(em).await;
                                let _ = i.can_go_next_changed(em).await;
                                i.can_go_previous_changed(em).await
                            }
                            Cambio::Estado => {
                                let _ = i.playback_status_changed(em).await;
                                let _ = i.loop_status_changed(em).await;
                                let _ = i.shuffle_changed(em).await;
                                let _ = i.volume_changed(em).await;
                                let _ = i.can_play_changed(em).await;
                                let _ = i.can_seek_changed(em).await;
                                i.can_pause_changed(em).await
                            }
                        };
                    }
                });
            })
            .ok()?;
        listo_rx.recv().ok().filter(|ok| *ok)?;
        Some(Self { estado, aviso })
    }

    /// Lo que suena. Avisa al escritorio solo de lo que cambió (la posición no se avisa: se pide).
    pub fn poner(&self, nuevo: EstadoMpris) {
        let viejo = std::mem::replace(&mut *self.estado.lock().unwrap_or_else(|x| x.into_inner()), nuevo.clone());
        if viejo.cancion != nuevo.cancion
            || con_duracion(&viejo) != con_duracion(&nuevo)
            || viejo.portada != nuevo.portada
            || viejo.hay_siguiente != nuevo.hay_siguiente
            || viejo.hay_anterior != nuevo.hay_anterior
        {
            let _ = self.aviso.unbounded_send(Some(Cambio::Cancion));
        }
        let clave = |e: &EstadoMpris| (e.estado_texto(), e.repetir.clone(), e.aleatorio, e.volumen);
        if clave(&viejo) != clave(&nuevo) {
            let _ = self.aviso.unbounded_send(Some(Cambio::Estado));
        }
    }
}

impl Drop for Mpris {
    fn drop(&mut self) {
        let _ = self.aviso.unbounded_send(None);
    }
}
