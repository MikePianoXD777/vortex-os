//! El reproductor (como `reproductor.py`): la cola, el aleatorio y el repetir los lleva Vortex Player; mpv solo
//! toca la canción de turno. Va en dos partes:
//!
//! - [`Reproductor`]: el estado y las reglas, sin procesos ni hilos (se prueba solo, con eventos de mpv de
//!   mentira). Lo que hay que mandarle a mpv lo deja en [`Reproductor::salida`]; los crossfade que hay que
//!   lanzar, en [`Reproductor::fundidos`]; y lo que cambió (las señales de la de Python), en [`Reproductor::avisos`].
//! - [`MpvProceso`]: el mpv sin ventana, su socket JSON (IPC) y un hilo que lee sus eventos.
//!
//! Lo de siempre de la de Python: crossfade (la que se va suena en otro mpv apagándose), temporizador para
//! dormir, la cola «A continuación», practicar (velocidad y tramo A-B), la radio sin fin (`SeAcaba`) y cuánto
//! escuchaste de cada canción (`Escuchada`, para «Tu resumen»).

use rand::seq::SliceRandom;
use serde_json::{Map, Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::mpv::{CLIENTE_YOUTUBE, es_stream, validar_ruta_carga};
use crate::rutas;

pub const OBSERVAR: [(i64, &str); 4] = [(1, "time-pos"), (2, "duration"), (3, "pause"), (4, "idle-active")];
/// Segundos entre avisos de la posición: mpv la manda mucho más seguido.
pub const AVISO_POSICION: f64 = 0.2;

/// Lo que cambió (las señales de la de Python).
#[derive(Debug, Clone, PartialEq)]
pub enum Aviso {
    Cancion,
    Estado,
    Posicion,
    Cola,
    /// Le diste play (o tocar, siguiente…): el avance de una canción se calla.
    QuiereSonar,
    Dormir,
    Practica,
    Error(String),
    /// Suena la última de la cola (o ya terminó): pide radio.
    SeAcaba,
    /// (canción, segundos que de verdad sonó).
    Escuchada(Value, f64),
}

/// Segundos desde que arrancó el programa (el `time.monotonic()` de Python).
pub fn monotonico() -> f64 {
    static INICIO: OnceLock<Instant> = OnceLock::new();
    INICIO.get_or_init(Instant::now).elapsed().as_secs_f64()
}

fn ruta_de(c: &Value) -> &str {
    c.get("ruta").and_then(Value::as_str).unwrap_or("")
}

fn num(c: &Value, k: &str) -> f64 {
    c.get(k).and_then(Value::as_f64).unwrap_or(0.0)
}

fn redondear(x: f64, decimales: i32) -> f64 {
    let p = 10f64.powi(decimales);
    (x * p).round() / p
}

pub struct Reproductor {
    cola: Vec<Value>,
    /// El orden en que se tocan (cambia con aleatorio).
    orden: Vec<usize>,
    /// Posición en `orden` (-1 = nada).
    i: isize,
    posicion: f64,
    posicion_avisada: f64,
    duracion: f64,
    pausado: bool,
    volumen: i64,
    aleatorio: bool,
    /// «no», «todo» o «una».
    repetir: &'static str,
    /// De dónde suena («Tu biblioteca» o el nombre de la playlist).
    origen: String,
    /// Cuántas «tocar después» van detrás de la actual.
    despues: usize,
    /// Segundos de crossfade (0 = sin).
    fundido: f64,
    /// El ecualizador dentro de mpv (Windows), como `af`.
    eq: String,
    /// Para practicar: más lenta sin cambiar el tono.
    velocidad: f64,
    /// El tramo que se repite (-1 = sin marcar).
    ab: [f64; 2],
    /// La cola se acabó y quedó parada en la última.
    final_: bool,
    /// La canción a la que se le cuentan los segundos.
    sonando: Value,
    escuchado: f64,
    /// La de YouTube ya falló con el cliente visionos.
    sin_cliente: bool,
    /// Cuándo se cargó la canción (`reloj`).
    cargada: f64,
    /// Temporizador (`reloj`); 0 = apagado.
    dormir_hasta: f64,
    dormir_al_final: bool,
    apagando: bool,
    /// Cuándo hay que dar por terminada la canción que no se pudo tocar (0 = no).
    terminar_en: f64,
    ultimo_dormir: f64,
    /// `tocar_desde`: el segundo desde el que empieza la próxima `tocar`.
    desde: f64,
    pub reloj: fn() -> f64,
    /// Lo que hay que mandarle a mpv, en orden.
    pub salida: Vec<Value>,
    /// Los crossfade que hay que lanzar (argumentos de mpv, sin el programa).
    pub fundidos: Vec<Vec<String>>,
    /// Hay que matar los crossfade que estén sonando.
    pub parar_fundidos: bool,
    pub avisos: Vec<Aviso>,
}

impl Default for Reproductor {
    fn default() -> Self {
        Self::new()
    }
}

impl Reproductor {
    pub fn new() -> Self {
        Self {
            cola: Vec::new(),
            orden: Vec::new(),
            i: -1,
            posicion: 0.0,
            posicion_avisada: 0.0,
            duracion: 0.0,
            pausado: true,
            volumen: 80,
            aleatorio: false,
            repetir: "no",
            origen: String::new(),
            despues: 0,
            fundido: 0.0,
            eq: String::new(),
            velocidad: 1.0,
            ab: [-1.0, -1.0],
            final_: false,
            sonando: Value::Null,
            escuchado: 0.0,
            sin_cliente: false,
            cargada: 0.0,
            dormir_hasta: 0.0,
            dormir_al_final: false,
            apagando: false,
            terminar_en: 0.0,
            ultimo_dormir: 0.0,
            desde: 0.0,
            reloj: monotonico,
            salida: Vec::new(),
            fundidos: Vec::new(),
            parar_fundidos: false,
            avisos: Vec::new(),
        }
    }

    fn enviar(&mut self, v: Value) {
        self.salida.push(v);
    }

    fn avisar(&mut self, a: Aviso) {
        self.avisos.push(a);
    }

    fn n(&self) -> isize {
        self.orden.len() as isize
    }

    // --- mpv --------------------------------------------------------------------------------------------

    /// mpv se acaba de conectar (o se volvió a levantar): lo que necesita además de observar.
    pub fn al_conectar(&mut self) {
        let f = self.filtros();
        if !f.is_empty() {
            self.enviar(json!({"command": ["set_property", "af", f]}));
        }
    }

    /// mpv se cerró solo (se cayó): queda en pausa; el dueño lo vuelve a levantar sin perder la cola.
    pub fn mpv_murio(&mut self) {
        self.pausado = true;
        self.avisar(Aviso::Estado);
    }

    /// Un evento del socket de mpv.
    pub fn al_evento(&mut self, ev: &Value) {
        match ev.get("event").and_then(Value::as_str) {
            Some("property-change") => {
                let nombre = ev.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                self.propiedad(&nombre, ev.get("data").unwrap_or(&Value::Null));
            }
            Some("end-file") => match ev.get("reason").and_then(Value::as_str) {
                Some("eof") => self.termino_cancion(),
                Some("error") => {
                    let c = self.cancion();
                    if es_stream(ruta_de(&c)) && !self.sin_cliente {
                        // como el avance: otra vez sin cliente
                        self.sin_cliente = true;
                        self.cargar(false, 0.0, 0.0);
                        return;
                    }
                    self.fallo_de(&c);
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn fallo_de(&mut self, c: &Value) {
        let nombre = c
            .get("titulo")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                Path::new(ruta_de(c))
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
        self.avisar(Aviso::Error(format!("No se pudo tocar: {nombre}")));
        self.terminar_en = (self.reloj)() + 0.3;
    }

    fn propiedad(&mut self, nombre: &str, valor: &Value) {
        match nombre {
            "time-pos" => {
                let nueva = valor.as_f64().unwrap_or(0.0);
                let paso = nueva - self.posicion;
                if !self.pausado && 0.0 < paso && paso < 3.0 {
                    self.escuchado += paso; // lo que sí sonó (no los saltos)
                }
                self.posicion = nueva;
                if (nueva - self.posicion_avisada).abs() >= AVISO_POSICION {
                    self.posicion_avisada = nueva;
                    self.avisar(Aviso::Posicion);
                }
                self.revisar_fundido();
            }
            "duration" => {
                self.duracion = valor.as_f64().unwrap_or(0.0);
                self.avisar(Aviso::Posicion);
            }
            "pause" => {
                self.pausado = valor.as_bool().unwrap_or(false);
                if self.pausado {
                    self.parar_fundidos = true;
                }
                self.avisar(Aviso::Estado);
            }
            _ => {}
        }
    }

    /// Llamar seguido (cada ~¼ s): el temporizador para dormir y lo que espera un momento.
    pub fn tic(&mut self) {
        let ahora = (self.reloj)();
        if self.terminar_en > 0.0 && ahora >= self.terminar_en {
            self.terminar_en = 0.0;
            self.termino_cancion();
        }
        if (self.dormir_hasta > 0.0 || self.dormir_al_final) && ahora - self.ultimo_dormir >= 0.5 {
            self.ultimo_dormir = ahora;
            self.revisar_dormir();
        }
    }

    fn avanzar(&mut self) {
        self.i += 1;
        self.despues = self.despues.saturating_sub(1);
    }

    fn termino_cancion(&mut self) {
        if self.dormir_al_final {
            // «al terminar la canción»: queda la siguiente, en pausa
            self.dormir_al_final = false;
            self.avisar(Aviso::Dormir);
            if self.i + 1 < self.n() {
                self.avanzar();
            }
            self.cargar(true, 0.0, 0.0);
            return;
        }
        if self.repetir == "una" {
            self.cargar(false, 0.0, 0.0);
        } else if self.i + 1 < self.n() {
            self.avanzar();
            self.cargar(false, 0.0, 0.0);
        } else if self.repetir == "todo" && !self.orden.is_empty() {
            if self.aleatorio {
                self.orden.shuffle(&mut rand::rng());
            }
            self.i = 0;
            self.cargar(false, 0.0, 0.0);
        } else {
            self.pausado = true;
            self.posicion = 0.0;
            self.final_ = true;
            self.cerrar_escucha();
            self.avisar(Aviso::Estado);
            self.avisar(Aviso::SeAcaba);
        }
    }

    /// Cuenta como escuchada si sonó 30 s (o la mitad, si es más corta).
    pub fn cerrar_escucha(&mut self) {
        let c = std::mem::replace(&mut self.sonando, Value::Null);
        let s = std::mem::replace(&mut self.escuchado, 0.0);
        let d = num(&c, "duracion");
        let dur = if d != 0.0 { d } else { self.duracion };
        let minimo = if dur > 0.0 { (dur * 0.5).min(30.0) } else { 30.0 };
        if c.is_object() && c.as_object().is_some_and(|m| !m.is_empty()) && s >= minimo {
            self.avisar(Aviso::Escuchada(c, redondear(s, 1)));
        }
    }

    /// `entra`: segundos en que sube desde el silencio (la que entra con crossfade).
    fn cargar(&mut self, pausado: bool, desde: f64, entra: f64) {
        let c = self.cancion();
        if !c.as_object().is_some_and(|m| !m.is_empty()) {
            return;
        }
        let ruta = ruta_de(&c).to_string();
        if ruta != ruta_de(&self.sonando) {
            self.sin_cliente = false;
        }
        self.cerrar_escucha();
        if self.ab.iter().any(|x| *x >= 0.0) {
            self.quitar_ab(); // el tramo de A a B es de la canción que se practicaba
        }
        self.sonando = c.clone();
        self.final_ = false;
        self.posicion = desde;
        self.duracion = num(&c, "duracion");
        self.cargada = (self.reloj)();
        if validar_ruta_carga(&ruta).is_err() {
            self.fallo_de(&c);
            return;
        }
        // mpv 0.38+: loadfile <url> <flags> <índice> <opciones> (solo para este archivo)
        let mut opciones = Map::new();
        if desde > 0.0 {
            opciones.insert("start".into(), json!(format!("{desde:.2}")));
        }
        if entra > 0.0 {
            let f = [self.filtros(), format!("lavfi=[afade=t=in:d={entra:.1}]")]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(",");
            opciones.insert("af".into(), json!(f));
        }
        if es_stream(&ruta) && !self.sin_cliente {
            opciones.insert("ytdl-raw-options".into(), json!(CLIENTE_YOUTUBE));
        }
        if opciones.is_empty() {
            self.enviar(json!({"command": ["loadfile", ruta, "replace"]}));
        } else {
            self.enviar(json!({"command": ["loadfile", ruta, "replace", -1, opciones]}));
        }
        self.enviar(json!({"command": ["set_property", "pause", pausado]}));
        self.pausado = pausado;
        if !pausado {
            self.avisar(Aviso::QuiereSonar);
        }
        self.avisar(Aviso::Cancion);
        self.avisar(Aviso::Cola); // la cola «A continuación» empieza en otro lado
        self.avisar(Aviso::Estado);
        self.avisar(Aviso::Posicion);
        if self.repetir == "no" && self.i + 1 >= self.n() {
            self.avisar(Aviso::SeAcaba);
        }
    }

    // --- para la interfaz y MPRIS ---------------------------------------------------------------------------

    /// Como `tocar`, pero desde ese segundo (para seguir lo que sonaba en el otro aparato).
    pub fn tocar_desde(&mut self, canciones: &[Value], inicio: i64, segundo: f64) {
        self.desde = segundo.max(0.0);
        self.tocar(canciones, inicio);
    }

    /// Toca la lista desde la canción `inicio` (con aleatorio, esa primero y luego las demás revueltas).
    pub fn tocar(&mut self, canciones: &[Value], inicio: i64) {
        self.cola = canciones
            .iter()
            .filter(|c| c.is_object() && !ruta_de(c).is_empty())
            .cloned()
            .collect();
        if self.cola.is_empty() {
            return;
        }
        let inicio = inicio.clamp(0, self.cola.len() as i64 - 1) as usize;
        self.despues = 0;
        self.orden = (0..self.cola.len()).collect();
        if self.aleatorio {
            let mut resto: Vec<usize> = self.orden.iter().copied().filter(|i| *i != inicio).collect();
            resto.shuffle(&mut rand::rng());
            self.orden = std::iter::once(inicio).chain(resto).collect();
            self.i = 0;
        } else {
            self.i = inicio as isize;
        }
        self.avisar(Aviso::Cola);
        let desde = std::mem::replace(&mut self.desde, 0.0);
        self.cargar(false, desde, 0.0);
    }

    // --- seguir donde te quedaste -------------------------------------------------------------------------

    /// Lo que hace falta para volver a abrir la app y seguir aquí.
    pub fn sesion(&self) -> Value {
        json!({
            "rutas": self.cola.iter().map(ruta_de).collect::<Vec<_>>(), "orden": self.orden, "i": self.i,
            "posicion": redondear(self.posicion, 2), "aleatorio": self.aleatorio, "repetir": self.repetir,
            "origen": self.origen,
        })
    }

    /// Pone la cola guardada en pausa, en la canción y el segundo donde ibas. `cola`: las canciones de
    /// `s["rutas"]` que siguen en tu biblioteca (en el mismo orden; las que ya no están se brincan).
    pub fn restaurar(&mut self, cola: &[Value], s: &Value) -> bool {
        if cola.is_empty() {
            return false;
        }
        let rutas_: Vec<&str> = cola.iter().map(ruta_de).collect();
        let guardadas: Vec<String> = s
            .get("rutas")
            .and_then(Value::as_array)
            .map(|v| v.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
            .unwrap_or_default();
        let orden_viejo: Vec<usize> = s
            .get("orden")
            .and_then(Value::as_array)
            .filter(|v| !v.is_empty())
            .map(|v| v.iter().filter_map(|x| x.as_u64().map(|x| x as usize)).collect())
            .unwrap_or_else(|| (0..guardadas.len()).collect());
        let i = s.get("i").and_then(Value::as_i64).unwrap_or(0);
        let actual = (i >= 0)
            .then(|| orden_viejo.get(i as usize))
            .flatten()
            .and_then(|k| guardadas.get(*k))
            .cloned();
        // el orden guardado (con aleatorio) traducido a la cola nueva
        let mut orden: Vec<usize> = orden_viejo
            .iter()
            .filter_map(|k| guardadas.get(*k))
            .filter_map(|r| rutas_.iter().position(|x| x == r))
            .collect();
        let mut ordenado = orden.clone();
        ordenado.sort_unstable();
        if ordenado != (0..cola.len()).collect::<Vec<_>>() {
            orden = (0..cola.len()).collect();
        }
        self.cola = cola.to_vec();
        self.i = orden
            .iter()
            .position(|j| Some(ruta_de(&cola[*j])) == actual.as_deref())
            .unwrap_or(0) as isize;
        self.orden = orden;
        self.aleatorio = s.get("aleatorio").and_then(Value::as_bool).unwrap_or(false);
        self.repetir = match s.get("repetir").and_then(Value::as_str) {
            Some("todo") => "todo",
            Some("una") => "una",
            _ => "no",
        };
        self.origen = s.get("origen").and_then(Value::as_str).unwrap_or("").to_string();
        self.avisar(Aviso::Cola);
        let desde = if Some(ruta_de(&self.cancion())) == actual.as_deref() {
            s.get("posicion").and_then(Value::as_f64).unwrap_or(0.0)
        } else {
            0.0
        };
        self.cargar(true, desde, 0.0);
        true
    }

    pub fn poner_origen(&mut self, origen: &str) {
        self.origen = origen.to_string();
        self.avisar(Aviso::Cola);
    }

    fn hay_cancion(&self) -> bool {
        self.i >= 0 && (self.i as usize) < self.orden.len()
    }

    pub fn alternar(&mut self) {
        if !self.hay_cancion() {
            return;
        }
        if self.pausado {
            self.avisar(Aviso::QuiereSonar);
        }
        self.enviar(json!({"command": ["cycle", "pause"]}));
    }

    pub fn pausar(&mut self) {
        self.enviar(json!({"command": ["set_property", "pause", true]}));
    }

    pub fn seguir(&mut self) {
        if self.hay_cancion() {
            self.avisar(Aviso::QuiereSonar);
            self.enviar(json!({"command": ["set_property", "pause", false]}));
        }
    }

    pub fn siguiente(&mut self) {
        if self.i + 1 < self.n() {
            self.avanzar();
            self.cargar(false, 0.0, 0.0);
        } else if self.repetir != "no" && !self.orden.is_empty() {
            self.i = 0;
            self.cargar(false, 0.0, 0.0);
        }
    }

    pub fn anterior(&mut self) {
        // Como en todos lados: pasados 3 s vuelve al inicio de la misma
        if self.posicion > 3.0 || self.i <= 0 {
            self.buscar(0.0);
        } else {
            self.i -= 1;
            self.despues = 0;
            self.cargar(false, 0.0, 0.0);
        }
    }

    pub fn buscar(&mut self, segundos: f64) {
        let s = segundos.max(0.0);
        self.enviar(json!({"command": ["seek", s, "absolute"]}));
        self.posicion = s;
        self.avisar(Aviso::Posicion);
    }

    pub fn poner_volumen(&mut self, v: i64) {
        self.volumen = v.clamp(0, 100);
        self.enviar(json!({"command": ["set_property", "volume", self.volumen]}));
        self.avisar(Aviso::Estado);
    }

    pub fn alternar_aleatorio(&mut self) {
        self.aleatorio = !self.aleatorio;
        if self.hay_cancion() {
            let actual = self.orden[self.i as usize];
            if self.aleatorio {
                let mut resto: Vec<usize> = (0..self.cola.len()).filter(|i| *i != actual).collect();
                resto.shuffle(&mut rand::rng());
                self.orden = std::iter::once(actual).chain(resto).collect();
                self.i = 0;
            } else {
                self.orden = (0..self.cola.len()).collect();
                self.i = actual as isize;
            }
        }
        self.avisar(Aviso::Estado);
    }

    pub fn alternar_repetir(&mut self) {
        self.repetir = match self.repetir {
            "no" => "todo",
            "todo" => "una",
            _ => "no",
        };
        self.avisar(Aviso::Estado);
    }

    /// Si se borra una canción que está en la cola (no la que suena).
    pub fn quitar_de_cola(&mut self, ruta: &str) {
        let actual = ruta_de(&self.cancion()).to_string();
        if ruta == actual {
            return;
        }
        self.cola.retain(|c| ruta_de(c) != ruta);
        self.orden = (0..self.cola.len()).collect();
        self.i = self
            .cola
            .iter()
            .position(|c| ruta_de(c) == actual)
            .map_or(-1, |i| i as isize);
        self.avisar(Aviso::Cola);
    }

    // --- la cola «A continuación» -------------------------------------------------------------------------

    /// Suena justo después de la actual (y de las que ya pusiste con «tocar después»).
    pub fn tocar_despues(&mut self, c: &Value) {
        if ruta_de(c).is_empty() {
            return;
        }
        if self.orden.is_empty() {
            self.tocar(std::slice::from_ref(c), 0);
            return;
        }
        self.cola.push(c.clone());
        let donde = ((self.i + 1) as usize + self.despues).min(self.orden.len());
        self.orden.insert(donde, self.cola.len() - 1);
        self.despues += 1;
        self.avisar(Aviso::Cola);
    }

    /// Al final de la cola.
    pub fn agregar_a_cola(&mut self, c: &Value) {
        if ruta_de(c).is_empty() {
            return;
        }
        if self.orden.is_empty() {
            self.tocar(std::slice::from_ref(c), 0);
            return;
        }
        self.cola.push(c.clone());
        self.orden.push(self.cola.len() - 1);
        self.avisar(Aviso::Cola);
    }

    /// La radio sin fin: al final de la cola; si ya se había acabado, sigue sonando con la primera.
    pub fn agregar_radio(&mut self, canciones: &[Value]) {
        let nuevas: Vec<Value> = canciones.iter().filter(|c| !ruta_de(c).is_empty()).cloned().collect();
        if nuevas.is_empty() || self.orden.is_empty() {
            return;
        }
        for c in nuevas {
            self.cola.push(c);
            self.orden.push(self.cola.len() - 1);
        }
        self.avisar(Aviso::Cola);
        if self.final_ {
            self.i += 1;
            self.cargar(false, 0.0, 0.0);
        }
    }

    /// Toca la canción k de la cola (k: su lugar en `cola`).
    pub fn saltar_a(&mut self, k: i64) {
        if 0 <= k && (k as usize) < self.orden.len() {
            self.i = k as isize;
            self.despues = 0;
            self.cargar(false, 0.0, 0.0);
        }
    }

    /// Quita de la cola la k (una que viene, no la que suena).
    pub fn quitar_de_cola_en(&mut self, k: i64) {
        if (self.i as i64) < k && (k as usize) < self.orden.len() {
            if k <= self.i as i64 + self.despues as i64 {
                self.despues = self.despues.saturating_sub(1);
            }
            self.orden.remove(k as usize);
            self.avisar(Aviso::Cola);
        }
    }

    // --- temporizador para dormir ------------------------------------------------------------------------

    /// Pausa en `minutos` (bajando el volumen los últimos 10 s); -1: al terminar la canción; 0: apagado.
    pub fn dormir(&mut self, minutos: i64) {
        if self.apagando {
            self.enviar(json!({"command": ["set_property", "volume", self.volumen]}));
        }
        self.apagando = false;
        self.dormir_al_final = minutos < 0;
        self.dormir_hasta = if minutos > 0 {
            (self.reloj)() + minutos as f64 * 60.0
        } else {
            0.0
        };
        self.avisar(Aviso::Dormir);
    }

    fn revisar_dormir(&mut self) {
        if self.dormir_hasta == 0.0 {
            return;
        }
        let falta = self.dormir_hasta - (self.reloj)();
        if falta <= 0.0 {
            self.dormir_hasta = 0.0;
            self.pausar();
            self.enviar(json!({"command": ["set_property", "volume", self.volumen]}));
            self.apagando = false;
        } else if falta <= 10.0 && !self.pausado {
            self.apagando = true;
            let v = (self.volumen as f64 * falta / 10.0).round() as i64;
            self.enviar(json!({"command": ["set_property", "volume", v]}));
        }
        self.avisar(Aviso::Dormir);
    }

    pub fn dormir_texto(&self) -> String {
        if self.dormir_al_final {
            return "Al terminar la canción".into();
        }
        if self.dormir_hasta > 0.0 {
            let falta = (self.dormir_hasta - (self.reloj)()).max(0.0);
            return format!("{}:{:02}", (falta / 60.0) as i64, (falta % 60.0) as i64);
        }
        String::new()
    }

    pub fn dormir_activo(&self) -> bool {
        self.dormir_hasta > 0.0 || self.dormir_al_final
    }

    // --- ecualizador en mpv, practicar y crossfade ------------------------------------------------------

    /// El ecualizador dentro de mpv (`af`; «» = sin filtros).
    pub fn poner_eq(&mut self, filtro: &str) {
        if filtro == self.eq {
            return;
        }
        self.eq = filtro.to_string();
        let f = self.filtros();
        self.enviar(json!({"command": ["set_property", "af", f]}));
    }

    /// El ecualizador y, si va más lenta o más rápida, scaletempo2 (el tono no cambia).
    pub fn filtros(&self) -> String {
        let tempo = if self.velocidad != 1.0 { "scaletempo2" } else { "" };
        [self.eq.as_str(), tempo]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn poner_velocidad(&mut self, v: f64) {
        let v = redondear(v.clamp(0.25, 2.0), 2);
        if v == self.velocidad {
            return;
        }
        self.velocidad = v;
        let f = self.filtros();
        self.enviar(json!({"command": ["set_property", "af", f]}));
        self.enviar(json!({"command": ["set_property", "speed", v]}));
        self.avisar(Aviso::Practica);
    }

    /// La 1.ª vez marca A, la 2.ª B (y se repite de A a B), la 3.ª lo quita.
    pub fn marcar_ab(&mut self) {
        let [a, b] = self.ab;
        if a < 0.0 {
            self.ab = [redondear(self.posicion, 2), -1.0];
            self.enviar(json!({"command": ["set_property", "ab-loop-a", self.ab[0]]}));
        } else if b < 0.0 && self.posicion > a + 0.5 {
            self.ab = [a, redondear(self.posicion, 2)];
            self.enviar(json!({"command": ["set_property", "ab-loop-b", self.ab[1]]}));
            self.buscar(a);
        } else {
            self.quitar_ab();
            return;
        }
        self.avisar(Aviso::Practica);
    }

    pub fn quitar_ab(&mut self) {
        self.ab = [-1.0, -1.0];
        self.enviar(json!({"command": ["set_property", "ab-loop-a", "no"]}));
        self.enviar(json!({"command": ["set_property", "ab-loop-b", "no"]}));
        self.avisar(Aviso::Practica);
    }

    pub fn poner_fundido(&mut self, segundos: f64) {
        self.fundido = segundos.clamp(0.0, 12.0);
        self.avisar(Aviso::Estado);
    }

    fn siguiente_indice(&self) -> isize {
        if self.i + 1 < self.n() {
            return self.i + 1;
        }
        if self.repetir == "todo" && self.orden.len() > 1 { 0 } else { -1 }
    }

    /// A `fundido` segundos del final: la que se va sigue en otro mpv apagándose y la siguiente entra.
    fn revisar_fundido(&mut self) {
        let n = self.fundido;
        if n <= 0.0
            || self.pausado
            || self.repetir == "una"
            || self.velocidad != 1.0
            || self.ab[1] >= 0.0
            || self.dormir_al_final
            || self.duracion < n * 3.0
            || (self.reloj)() - self.cargada < 2.0
            || self.duracion - self.posicion > n
        {
            return;
        }
        let k = self.siguiente_indice();
        if k < 0 || !self.hay_cancion() {
            return;
        }
        let c = self.cancion();
        let otra = self.cola[self.orden[k as usize]].clone();
        if es_stream(ruta_de(&c)) || es_stream(ruta_de(&otra)) {
            return; // la de YouTube tarda en abrir: se pasa sin crossfade
        }
        let desde = self.posicion;
        let resto = (self.duracion - self.posicion).max(0.5);
        let af = [self.eq.clone(), format!("lavfi=[afade=t=out:st={desde:.2}:d={resto:.2}]")]
            .into_iter()
            .filter(|x| !x.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        self.fundidos.push(vec![
            "--no-video".into(),
            "--no-terminal".into(),
            "--no-config".into(),
            "--audio-display=no".into(),
            format!("--ao={}", rutas::salida_audio()),
            format!("--volume={}", self.volumen),
            "--volume-max=100".into(),
            "--replaygain=no".into(),
            "--audio-client-name=Vortex Player".into(),
            format!("--start={desde:.2}"),
            format!("--af={af}"),
            "--".into(),
            ruta_de(&c).to_string(),
        ]);
        if k == 0 && self.aleatorio {
            self.orden.shuffle(&mut rand::rng());
        }
        self.i = k;
        self.despues = self.despues.saturating_sub(1);
        self.cargar(false, 0.0, n.min(resto));
    }

    // --- lo que lee la ventana -------------------------------------------------------------------------

    pub fn cancion(&self) -> Value {
        if self.hay_cancion() {
            self.cola[self.orden[self.i as usize]].clone()
        } else {
            json!({})
        }
    }
    pub fn posicion(&self) -> f64 {
        self.posicion
    }
    pub fn duracion(&self) -> f64 {
        self.duracion
    }
    pub fn pausado(&self) -> bool {
        self.pausado
    }
    pub fn volumen(&self) -> i64 {
        self.volumen
    }
    pub fn aleatorio(&self) -> bool {
        self.aleatorio
    }
    pub fn repetir(&self) -> &str {
        self.repetir
    }
    pub fn hay_anterior(&self) -> bool {
        !self.orden.is_empty()
    }
    pub fn hay_siguiente(&self) -> bool {
        self.i + 1 < self.n() || (self.repetir != "no" && !self.orden.is_empty())
    }
    pub fn cola(&self) -> Vec<Value> {
        self.orden.iter().map(|i| self.cola[*i].clone()).collect()
    }
    /// Las que vienen (hasta 300).
    pub fn cola_siguientes(&self) -> Vec<Value> {
        let desde = ((self.i + 1).max(0) as usize).min(self.orden.len());
        self.orden[desde..]
            .iter()
            .take(300)
            .map(|i| self.cola[*i].clone())
            .collect()
    }
    pub fn cola_restantes(&self) -> i64 {
        (self.n() - self.i - 1).max(0) as i64
    }
    pub fn fundido(&self) -> f64 {
        self.fundido
    }
    pub fn velocidad(&self) -> f64 {
        self.velocidad
    }
    pub fn ab_a(&self) -> f64 {
        self.ab[0]
    }
    pub fn ab_b(&self) -> f64 {
        self.ab[1]
    }
    pub fn indice(&self) -> i64 {
        self.i as i64
    }
    pub fn origen(&self) -> &str {
        &self.origen
    }
    pub fn final_de_cola(&self) -> bool {
        self.final_
    }
}

// --- el proceso de mpv ----------------------------------------------------------------------------------

/// Lo que llega del hilo lector.
#[derive(Debug, Clone, PartialEq)]
pub enum DeMpv {
    /// Ya se conectó al socket (se le mandó lo pendiente).
    Conectado,
    Evento(Value),
    /// mpv se cerró solo: hay que volver a levantarlo.
    Murio,
    /// No arrancó (¿no está instalado?).
    NoArranco,
}

struct Conexion {
    #[cfg(unix)]
    escritor: Option<std::os::unix::net::UnixStream>,
    pendientes: Vec<String>,
}

/// El mpv sin ventana manejado por su socket JSON. Los eventos llegan a `al_recibir` desde un hilo aparte.
pub struct MpvProceso {
    hijo: Arc<Mutex<Option<Child>>>,
    conexion: Arc<Mutex<Conexion>>,
    socket: String,
    carpeta: Option<std::path::PathBuf>,
    fundidos: Vec<Child>,
    cerrando: Arc<std::sync::atomic::AtomicBool>,
}

impl MpvProceso {
    /// Arranca mpv (`VORTEX_PLAYER_MPV` cambia cuál; `VORTEX_PLAYER_AO=null` no saca sonido) y un hilo que se
    /// conecta a su socket y lee sus eventos.
    pub fn arrancar(volumen: i64, al_recibir: impl Fn(DeMpv) + Send + Sync + 'static) -> Self {
        let (carpeta, privada) = crate::mpv::carpeta_socket_privada(std::process::id())
            .unwrap_or_else(|_| (std::env::temp_dir(), None));
        let (ipc, socket) = rutas::canal_mpv("mpv", &carpeta);
        let p = Self {
            hijo: Arc::new(Mutex::new(None)),
            conexion: Arc::new(Mutex::new(Conexion {
                #[cfg(unix)]
                escritor: None,
                pendientes: Vec::new(),
            })),
            socket,
            carpeta: privada,
            fundidos: Vec::new(),
            cerrando: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        };
        p.levantar(volumen, &ipc, Arc::new(al_recibir));
        p
    }

    fn programa() -> String {
        std::env::var("VORTEX_PLAYER_MPV")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "mpv".into())
    }

    fn levantar(&self, volumen: i64, ipc: &str, al_recibir: Arc<dyn Fn(DeMpv) + Send + Sync + 'static>) {
        if !cfg!(windows) {
            let _ = std::fs::remove_file(&self.socket);
        }
        let hijo = Command::new(Self::programa())
            .args([
                "--idle=yes",
                "--no-video",
                "--no-terminal",
                "--no-config",
                "--audio-display=no",
                &format!("--ao={}", rutas::salida_audio()),
                "--volume-max=100",
                &format!("--volume={}", volumen.clamp(0, 100)),
                "--replaygain=no",
                "--audio-pitch-correction=no",
                "--gapless-audio=weak",
                "--audio-client-name=Vortex Player",
                "--pause=yes",
                "--ytdl-format=bestaudio", // la radio suena de YouTube sin bajarla
                &format!("--input-ipc-server={ipc}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        let Ok(hijo) = hijo else {
            al_recibir(DeMpv::NoArranco);
            return;
        };
        *self.hijo.lock().unwrap_or_else(|e| e.into_inner()) = Some(hijo);
        let (conexion, socket, hijo, cerrando) = (
            self.conexion.clone(),
            self.socket.clone(),
            self.hijo.clone(),
            self.cerrando.clone(),
        );
        std::thread::Builder::new()
            .name("vortex-player-mpv".into())
            .spawn(move || {
                #[cfg(unix)]
                {
                    use std::os::unix::net::UnixStream;
                    // Hasta ~9 s para que aparezca el socket (60 intentos, como la de Python)
                    let mut lector = None;
                    for _ in 0..60 {
                        if let Ok(s) = UnixStream::connect(&socket) {
                            lector = Some(s);
                            break;
                        }
                        let vivo = hijo
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .as_mut()
                            .is_some_and(|h| matches!(h.try_wait(), Ok(None)));
                        if !vivo {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    let Some(s) = lector else {
                        al_recibir(DeMpv::NoArranco);
                        return;
                    };
                    {
                        let mut c = conexion.lock().unwrap_or_else(|e| e.into_inner());
                        let mut e = match s.try_clone() {
                            Ok(e) => e,
                            Err(_) => {
                                al_recibir(DeMpv::NoArranco);
                                return;
                            }
                        };
                        for (id, nombre) in OBSERVAR {
                            let _ = writeln!(e, "{}", json!({"command": ["observe_property", id, nombre]}));
                        }
                        for l in c.pendientes.drain(..) {
                            let _ = e.write_all(l.as_bytes());
                        }
                        c.escritor = Some(e);
                    }
                    al_recibir(DeMpv::Conectado);
                    for linea in BufReader::new(s).lines() {
                        let Ok(linea) = linea else { break };
                        if let Ok(v) = serde_json::from_str::<Value>(&linea)
                            && v.get("event").is_some()
                        {
                            al_recibir(DeMpv::Evento(v));
                        }
                    }
                    conexion.lock().unwrap_or_else(|e| e.into_inner()).escritor = None;
                    if !cerrando.load(std::sync::atomic::Ordering::SeqCst) {
                        if let Some(h) = hijo.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
                            let _ = h.kill();
                            let _ = h.wait();
                        }
                        al_recibir(DeMpv::Murio);
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = (conexion, socket, hijo, cerrando);
                    al_recibir(DeMpv::NoArranco);
                }
            })
            .ok();
    }

    /// Lo vuelve a levantar después de que se cayó (`DeMpv::Murio`).
    pub fn relevantar(&self, volumen: i64, al_recibir: impl Fn(DeMpv) + Send + Sync + 'static) {
        let carpeta = Path::new(&self.socket).parent().map(Path::to_path_buf).unwrap_or_default();
        let (ipc, _) = rutas::canal_mpv("mpv", &carpeta);
        self.levantar(volumen, &ipc, Arc::new(al_recibir));
    }

    pub fn enviar(&self, v: &Value) {
        let linea = format!("{v}\n");
        let mut c = self.conexion.lock().unwrap_or_else(|e| e.into_inner());
        #[cfg(unix)]
        if let Some(e) = c.escritor.as_mut() {
            if e.write_all(linea.as_bytes()).is_ok() {
                return;
            }
            c.escritor = None;
        }
        c.pendientes.push(linea);
    }

    /// Le pasa a mpv lo que dejó el reproductor y lanza o para los crossfade.
    pub fn despachar(&mut self, r: &mut Reproductor) {
        for v in r.salida.drain(..) {
            self.enviar(&v);
        }
        if std::mem::take(&mut r.parar_fundidos) {
            self.parar_fundidos();
        }
        self.fundidos.retain_mut(|h| matches!(h.try_wait(), Ok(None)));
        for args in r.fundidos.drain(..) {
            if let Ok(h) = Command::new(Self::programa())
                .args(&args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                self.fundidos.push(h);
            }
        }
    }

    pub fn parar_fundidos(&mut self) {
        for mut h in self.fundidos.drain(..) {
            let _ = h.kill();
            let _ = h.wait();
        }
    }

    /// Cierra mpv (y los crossfade) y borra el socket.
    pub fn cerrar(&mut self) {
        self.cerrando.store(true, std::sync::atomic::Ordering::SeqCst);
        self.parar_fundidos();
        self.enviar(&json!({"command": ["quit"]}));
        if let Some(mut h) = self.hijo.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let limite = Instant::now() + Duration::from_millis(1500);
            while matches!(h.try_wait(), Ok(None)) && Instant::now() < limite {
                std::thread::sleep(Duration::from_millis(30));
            }
            let _ = h.kill();
            let _ = h.wait();
        }
        if !cfg!(windows) {
            let _ = std::fs::remove_file(&self.socket);
        }
        if let Some(c) = self.carpeta.take() {
            let _ = std::fs::remove_dir(c);
        }
    }
}

impl Drop for MpvProceso {
    fn drop(&mut self) {
        if !self.cerrando.load(std::sync::atomic::Ordering::SeqCst) {
            self.cerrar();
        }
    }
}

/// Un canal sencillo para pasar `DeMpv` a otro hilo (las pruebas y quien no use Qt).
pub fn canal() -> (impl Fn(DeMpv) + Send + Sync + Clone + 'static, mpsc::Receiver<DeMpv>) {
    let (tx, rx) = mpsc::channel();
    let tx = Arc::new(Mutex::new(tx));
    (
        move |m| {
            let _ = tx.lock().unwrap_or_else(|e| e.into_inner()).send(m);
        },
        rx,
    )
}
