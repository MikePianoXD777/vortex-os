//! Lo que hace Connect, sin Qt: entrar a una red, pasar el portal y proteger. La ventana y el modo viaje
//! usan esto.
//!
//! El orden importa: una red con portal no da internet hasta aceptar, y WARP no puede salir antes. Por eso
//! primero se entra, luego se pregunta si hay internet o portal, y solo al final se cifra.

use crate::carpeta_config;
use crate::http;
use crate::proteccion::{Modo, Proteccion, Warp};
use crate::redes::{self, Actual, ErrorRed, Redes, Tipo};
use crate::sistema::Sistema;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Da 204 si hay internet. Va sin https a propósito: un portal solo puede interceptar lo que no va cifrado.
pub const PRUEBA: &str = "http://cp.cloudflare.com/generate_204";
pub const ESPERA_PORTAL: Duration = Duration::from_secs(300);
pub const ENFRIAR_FALLIDA: Duration = Duration::from_secs(600);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conexion {
    Internet,
    Portal,
    Nada,
}

/// La dirección de la prueba de internet: `VORTEX_CONNECT_PRUEBA` (pruebas) o la de Cloudflare.
pub fn url_de_prueba() -> String {
    std::env::var("VORTEX_CONNECT_PRUEBA").unwrap_or_else(|_| PRUEBA.into())
}

/// (internet, ""), (portal, dirección del portal) o (nada, ""). No sigue redirecciones ni usa proxy.
pub fn probar_internet(sistema: &Sistema, url: &str, limite: Duration) -> (Conexion, String) {
    let Some(r) = http::obtener(sistema, url, limite, 0) else {
        return (Conexion::Nada, String::new());
    };
    match r.estado {
        204 => (Conexion::Internet, String::new()),
        200..=299 => (Conexion::Portal, url.to_string()),
        _ => {
            let destino = r.ubicacion.filter(|d| !d.is_empty());
            let destino = match destino {
                Some(d) if d.starts_with("http://") || d.starts_with("https://") => d,
                _ => url.to_string(),
            };
            (Conexion::Portal, destino)
        }
    }
}

// --- Ajustes ---

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ajustes {
    /// la persona aceptó las condiciones de Cloudflare en la ventana
    pub condiciones: bool,
    pub modo_viaje: bool,
    /// si la prendió Connect, Connect la apaga al salir de la red abierta
    pub protegio_connect: bool,
}

impl Ajustes {
    pub fn ruta() -> PathBuf {
        carpeta_config().join("connect.json")
    }

    pub fn leer() -> Self {
        Self::leer_de(&Self::ruta())
    }

    /// Lo que no sea exactamente `true` cuenta como «no»: un archivo roto no acepta nada.
    pub fn leer_de(ruta: &Path) -> Self {
        let Ok(texto) = std::fs::read_to_string(ruta) else {
            return Self::default();
        };
        let Ok(Value::Object(d)) = serde_json::from_str::<Value>(&texto) else {
            return Self::default();
        };
        let si = |k: &str| d.get(k).and_then(Value::as_bool) == Some(true);
        Ajustes {
            condiciones: si("condiciones"),
            modo_viaje: si("modo_viaje"),
            protegio_connect: si("protegio_connect"),
        }
    }

    pub fn guardar(&self) {
        self.guardar_en(&Self::ruta());
    }

    pub fn guardar_en(&self, ruta: &Path) {
        let texto = json!({
            "condiciones": self.condiciones,
            "modo_viaje": self.modo_viaje,
            "protegio_connect": self.protegio_connect,
        })
        .to_string();
        let _ = vortex_core::config::escribir_atomico(ruta, &texto);
    }
}

// --- Un solo trabajo a la vez, entre la ventana y el modo viaje ---

pub struct Candado {
    ruta: PathBuf,
    archivo: Option<File>,
}

impl Candado {
    pub fn new() -> Self {
        Self::en(carpeta_config().join("connect.candado"))
    }

    pub fn en(ruta: PathBuf) -> Self {
        Candado { ruta, archivo: None }
    }

    pub fn tomar(&mut self) -> bool {
        if let Some(p) = self.ruta.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        let Ok(f) = OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .open(&self.ruta)
        else {
            return false;
        };
        if f.try_lock().is_err() {
            return false;
        }
        self.archivo = Some(f);
        true
    }

    pub fn soltar(&mut self) {
        self.archivo = None; // cerrar el archivo suelta el candado en los dos sistemas
    }
}

impl Default for Candado {
    fn default() -> Self {
        Self::new()
    }
}

/// Para parar lo que está esperando (el portal) desde otro hilo.
#[derive(Debug, Clone, Default)]
pub struct Cancelar(Arc<AtomicBool>);

impl Cancelar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancelar(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn cancelado(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

// --- El estado ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Situacion {
    #[default]
    SinRed,
    Portal,
    SinProteccion,
    SoloDns,
    Protegido,
    RedDeConfianza,
}

impl Situacion {
    /// El nombre que entiende la interfaz (`vc.estado`).
    pub fn como_texto(self) -> &'static str {
        match self {
            Situacion::SinRed => "sin_red",
            Situacion::Portal => "portal",
            Situacion::SinProteccion => "sin_proteccion",
            Situacion::SoloDns => "solo_dns",
            Situacion::Protegido => "protegido",
            Situacion::RedDeConfianza => "confianza",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Estado {
    pub situacion: Situacion,
    pub red: Actual,
    pub portal: String,
}

impl Estado {
    pub fn new(situacion: Situacion, red: Actual) -> Self {
        Estado {
            situacion,
            red,
            portal: String::new(),
        }
    }
}

pub type Aviso = Arc<dyn Fn(&str) + Send + Sync>;
pub type Probar = Box<dyn Fn() -> (Conexion, String) + Send + Sync>;
pub type Dormir = Box<dyn Fn(Duration) + Send + Sync>;

/// `avisar(texto)` cuenta lo que va pasando; `abrir_portal(url)` le enseña el portal a la persona.
pub struct Connect {
    pub redes: Box<dyn Redes>,
    pub warp: Box<dyn Proteccion>,
    probar: Probar,
    dormir: Dormir,
    ruta_ajustes: PathBuf,
    ajustes: Mutex<Ajustes>,
    avisar: Mutex<Aviso>,
    abrir_portal: Mutex<Aviso>,
    fallidas: Mutex<HashMap<String, Instant>>,
    espera_portal: Duration,
}

impl Connect {
    /// El de verdad: nmcli/netsh, warp-cli, la prueba de internet por `http://` y los ajustes de la persona.
    pub fn real(sistema: Sistema) -> Self {
        let url = url_de_prueba();
        let s = sistema.clone();
        let mut c = Self::nuevo(
            redes::crear(sistema.clone()),
            Box::new(Warp::new(sistema)),
            Box::new(move || probar_internet(&s, &url, Duration::from_secs(6))),
            Box::new(std::thread::sleep),
            Ajustes::leer(),
        );
        c.ruta_ajustes = Ajustes::ruta();
        c
    }

    /// Con las piezas que se le den (las pruebas ponen falsas). Los ajustes se guardan en la carpeta de
    /// configuración de Vortexine; `con_ruta_ajustes` cambia dónde.
    pub fn nuevo(
        redes: Box<dyn Redes>,
        warp: Box<dyn Proteccion>,
        probar: Probar,
        dormir: Dormir,
        ajustes: Ajustes,
    ) -> Self {
        let nada: Aviso = Arc::new(|_| {});
        Connect {
            redes,
            warp,
            probar,
            dormir,
            ruta_ajustes: Ajustes::ruta(),
            ajustes: Mutex::new(ajustes),
            avisar: Mutex::new(nada.clone()),
            abrir_portal: Mutex::new(nada),
            fallidas: Mutex::new(HashMap::new()),
            espera_portal: ESPERA_PORTAL,
        }
    }

    pub fn con_ruta_ajustes(mut self, ruta: PathBuf) -> Self {
        self.ruta_ajustes = ruta;
        self
    }

    pub fn con_espera_portal(mut self, espera: Duration) -> Self {
        self.espera_portal = espera;
        self
    }

    pub fn poner_avisar(&self, f: Aviso) {
        *self.avisar.lock().unwrap() = f;
    }

    pub fn poner_abrir_portal(&self, f: Aviso) {
        *self.abrir_portal.lock().unwrap() = f;
    }

    fn avisar(&self, texto: &str) {
        let f = self.avisar.lock().unwrap().clone();
        f(texto);
    }

    pub fn ajustes(&self) -> Ajustes {
        self.ajustes.lock().unwrap().clone()
    }

    /// Cambia los ajustes y los guarda.
    pub fn cambiar_ajustes(&self, f: impl FnOnce(&mut Ajustes)) {
        let mut a = self.ajustes.lock().unwrap();
        f(&mut a);
        a.guardar_en(&self.ruta_ajustes);
    }

    /// Vuelve a leer los ajustes del disco (el modo viaje lo hace en cada vuelta).
    pub fn releer_ajustes(&self) {
        *self.ajustes.lock().unwrap() = Ajustes::leer_de(&self.ruta_ajustes);
    }

    pub fn ruta_ajustes(&self) -> &Path {
        &self.ruta_ajustes
    }

    fn condiciones(&self) -> bool {
        self.ajustes.lock().unwrap().condiciones
    }

    pub fn estado(&self) -> Estado {
        let red = self.redes.actual();
        let modo = if self.condiciones() {
            self.warp.modo()
        } else {
            Modo::Ninguno
        };
        if modo == Modo::Cifrado {
            return Estado::new(Situacion::Protegido, red);
        }
        if red.tipo == Tipo::Ninguno {
            return Estado::new(Situacion::SinRed, red);
        }
        if modo == Modo::Dns {
            return Estado::new(Situacion::SoloDns, red);
        }
        let (conexion, portal) = (self.probar)();
        match conexion {
            Conexion::Portal => Estado {
                situacion: Situacion::Portal,
                red,
                portal,
            },
            Conexion::Nada => Estado::new(Situacion::SinRed, red),
            Conexion::Internet => {
                let s = if red.abierta {
                    Situacion::SinProteccion
                } else {
                    Situacion::RedDeConfianza
                };
                Estado::new(s, red)
            }
        }
    }

    /// Entra a esa red y la protege. Da `ErrorRed` si no se puede entrar.
    pub fn conectar(&self, ssid: &str, cancelar: &Cancelar) -> Result<Estado, ErrorRed> {
        if self.condiciones() && self.warp.conectado() {
            // un túnel prendido no deja ver el portal
            self.avisar("Pausando la protección para entrar…");
            self.warp.apagar();
        }
        self.avisar(&format!("Entrando a «{ssid}»…"));
        self.redes.conectar(ssid)?;
        Ok(self.asegurar(cancelar))
    }

    /// Ya dentro de una red: pasa el portal si lo hay y luego protege.
    pub fn asegurar(&self, cancelar: &Cancelar) -> Estado {
        if !self.pasar_portal(cancelar) {
            return self.estado();
        }
        self.proteger()
    }

    fn pasar_portal(&self, cancelar: &Cancelar) -> bool {
        let (mut conexion, mut portal) = (self.probar)();
        if conexion == Conexion::Internet {
            return true;
        }
        if conexion == Conexion::Nada {
            (self.dormir)(Duration::from_secs(3)); // el DHCP puede tardar tras entrar
            (conexion, portal) = (self.probar)();
            if conexion == Conexion::Internet {
                return true;
            }
            if conexion == Conexion::Nada {
                self.avisar("Esta red no da internet.");
                return false;
            }
        }
        self.avisar("Esta red pide aceptar algo. Lo que escribas en esa página no va cifrado.");
        let abrir = self.abrir_portal.lock().unwrap().clone();
        abrir(&portal);
        let fin = Instant::now() + self.espera_portal;
        while Instant::now() < fin && !cancelar.cancelado() {
            (self.dormir)(Duration::from_secs(3));
            if (self.probar)().0 == Conexion::Internet {
                return true;
            }
        }
        self.avisar("Sigue sin internet: la página de la red no se aceptó.");
        false
    }

    pub fn proteger(&self) -> Estado {
        if !self.condiciones() {
            self.avisar("Falta aceptar las condiciones de Cloudflare para cifrar.");
            return self.estado();
        }
        self.avisar("Cifrando tu conexión…");
        if let Err(e) = self.warp.preparar() {
            self.avisar(&e.to_string());
            return self.estado();
        }
        if self.warp.activar_cifrado() {
            self.marcar(true);
            self.avisar(if self.warp.blindar_dns() {
                "Protegido: lo que mandas va cifrado."
            } else {
                "Lo que mandas va cifrado, pero esta red aún puede ver los nombres de los sitios que abres."
            });
            return Estado::new(Situacion::Protegido, self.redes.actual());
        }
        self.avisar("Esta red bloquea el cifrado completo. Probando con el DNS cifrado…");
        if self.warp.activar_solo_dns() && self.warp.blindar_dns() {
            self.marcar(true);
            self.avisar("Solo el DNS va cifrado: esta red bloquea lo demás. Evita entrar a tu banco aquí.");
            return Estado::new(Situacion::SoloDns, self.redes.actual());
        }
        self.warp.apagar();
        self.marcar(false);
        self.avisar("No se pudo cifrar en esta red. Lo que mandas va sin protección.");
        self.estado()
    }

    pub fn quitar(&self) -> Estado {
        self.warp.apagar();
        self.marcar(false);
        self.avisar("Protección apagada.");
        self.estado()
    }

    fn marcar(&self, si: bool) {
        let mut a = self.ajustes.lock().unwrap();
        if a.protegio_connect != si {
            a.protegio_connect = si;
            a.guardar_en(&self.ruta_ajustes);
        }
    }

    /// Una vuelta del modo viaje. Solo actúa si hace falta; da `None` si no hizo nada.
    pub fn revisar(&self, cancelar: &Cancelar) -> Option<Estado> {
        let red = self.redes.actual();
        let a = self.ajustes();
        if red.tipo == Tipo::Cable || (red.tipo == Tipo::Wifi && !red.abierta) {
            // De vuelta en casa: la protección que prendió Connect ya no toca (y deja en paz a Tailscale).
            return a.protegio_connect.then(|| self.quitar());
        }
        if red.tipo == Tipo::Wifi {
            if a.condiciones && self.warp.modo() != Modo::Ninguno {
                // si el wifi se reconectó, NetworkManager le devolvió su DNS al enlace
                self.warp.blindar_dns();
                return None;
            }
            return Some(self.asegurar(cancelar));
        }
        if a.condiciones && self.warp.conectado() {
            self.warp.apagar(); // sin red, un túnel prendido solo estorba para la siguiente
        }
        let ahora = Instant::now();
        for candidata in self.redes.lista(true).unwrap_or_default() {
            let enfriando = self
                .fallidas
                .lock()
                .unwrap()
                .get(&candidata.ssid)
                .is_some_and(|t| ahora.duration_since(*t) < ENFRIAR_FALLIDA);
            if enfriando {
                continue;
            }
            match self.conectar(&candidata.ssid, cancelar) {
                Ok(e) => return Some(e),
                Err(_) => {
                    self.fallidas
                        .lock()
                        .unwrap()
                        .insert(candidata.ssid.clone(), ahora);
                }
            }
        }
        None
    }
}
