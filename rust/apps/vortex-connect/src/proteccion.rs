//! La protección: habla con el cliente oficial de Cloudflare WARP (`warp-cli`). Sin Qt.
//!
//! `--accept-tos` le dice a `warp-cli` que ya se aceptaron las condiciones de Cloudflare, así que solo se usa
//! si la persona las aceptó en la ventana (`ajustes.condiciones`); sin eso, ningún comando llega a `warp-cli`
//! (eso lo cuida el motor: aquí nunca se llama sin que él lo permita).
//!
//! En Linux el servicio de WARP no corre en reposo: se prende cuando hace falta y se apaga al quitar la
//! protección.

use crate::ES_WINDOWS;
use crate::http;
use crate::sistema::Sistema;
use regex::Regex;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

const WARP_WINDOWS: &str = r"C:\Program Files\Cloudflare\Cloudflare WARP\warp-cli.exe";
const TRAZA_REAL: &str = "https://www.cloudflare.com/cdn-cgi/trace";
const ESPERA_CIFRADO: Duration = Duration::from_secs(40);

static MODO_DOH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bMode:\s*DnsOverHttps\b").unwrap());

/// Algo que se le puede decir a la persona tal cual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorProteccion(pub String);

impl std::fmt::Display for ErrorProteccion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ErrorProteccion {}

/// Lo que está prendido ahora.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Modo {
    #[default]
    Ninguno,
    Cifrado,
    Dns,
}

/// Lo que el motor necesita de la protección (WARP real, o uno falso en las pruebas).
pub trait Proteccion: Send + Sync {
    fn instalado(&self) -> bool;
    fn conectado(&self) -> bool;
    fn modo(&self) -> Modo;
    fn preparar(&self) -> Result<(), ErrorProteccion>;
    fn activar_cifrado(&self) -> bool;
    fn activar_solo_dns(&self) -> bool;
    fn blindar_dns(&self) -> bool;
    fn apagar(&self);
}

impl<T: Proteccion + ?Sized> Proteccion for std::sync::Arc<T> {
    fn instalado(&self) -> bool {
        (**self).instalado()
    }
    fn conectado(&self) -> bool {
        (**self).conectado()
    }
    fn modo(&self) -> Modo {
        (**self).modo()
    }
    fn preparar(&self) -> Result<(), ErrorProteccion> {
        (**self).preparar()
    }
    fn activar_cifrado(&self) -> bool {
        (**self).activar_cifrado()
    }
    fn activar_solo_dns(&self) -> bool {
        (**self).activar_solo_dns()
    }
    fn blindar_dns(&self) -> bool {
        (**self).blindar_dns()
    }
    fn apagar(&self) {
        (**self).apagar()
    }
}

/// Con el túnel prendido, el firewall de WARP en Linux solo deja salir lo suyo y las redes privadas: lo que
/// Tailscale manda hacia afuera no pasa. En Windows no se ha comprobado, así que ahí no se avisa.
pub fn hay_tailscale() -> bool {
    !ES_WINDOWS && std::path::Path::new("/sys/class/net/tailscale0").exists()
}

/// Lo que ve Cloudflare es la prueba de verdad: «warp=on» o no va cifrado.
pub fn traza_dice_warp(sistema: &Sistema, url: &str, limite: Duration) -> bool {
    http::obtener(sistema, url, limite, 4096).is_some_and(|r| {
        r.cuerpo
            .lines()
            .any(|l| matches!(l.trim(), "warp=on" | "warp=plus"))
    })
}

pub type Traza = Box<dyn Fn() -> bool + Send + Sync>;
pub type Dormir = Box<dyn Fn(Duration) + Send + Sync>;

pub struct Warp {
    sistema: Sistema,
    traza: Traza,
    dormir: Dormir,
}

impl Warp {
    /// El WARP real; la dirección de la prueba sale de `VORTEX_CONNECT_TRAZA` (pruebas) o es la de Cloudflare.
    pub fn new(sistema: Sistema) -> Self {
        let url = std::env::var("VORTEX_CONNECT_TRAZA").unwrap_or_else(|_| TRAZA_REAL.into());
        let s = sistema.clone();
        Self::con(
            sistema,
            Box::new(move || traza_dice_warp(&s, &url, Duration::from_secs(8))),
            Box::new(std::thread::sleep),
        )
    }

    pub fn con(sistema: Sistema, traza: Traza, dormir: Dormir) -> Self {
        Warp {
            sistema,
            traza,
            dormir,
        }
    }

    fn ruta_warp(&self) -> Option<PathBuf> {
        if ES_WINDOWS && std::path::Path::new(WARP_WINDOWS).exists() {
            return Some(PathBuf::from(WARP_WINDOWS));
        }
        self.sistema.buscar("warp-cli")
    }

    fn cli(&self, args: &[&str], limite: Duration) -> (i32, String) {
        let Some(ruta) = self.ruta_warp() else {
            return (127, String::new());
        };
        let ruta = ruta.to_string_lossy().into_owned();
        let mut orden = vec![ruta.as_str(), "--accept-tos"];
        orden.extend_from_slice(args);
        let s = self.sistema.correr(&orden, limite);
        (s.codigo, s.texto())
    }

    fn servicio(&self, verbo: &str) -> bool {
        if ES_WINDOWS {
            return true; // en Windows es un servicio de Cloudflare que ya corre
        }
        self.sistema.codigo(
            &["systemctl", "--no-ask-password", verbo, "warp-svc.service"],
            Duration::from_secs(30),
        ) == 0
    }

    fn servicio_activo(&self) -> bool {
        ES_WINDOWS
            || self.sistema.codigo(
                &["systemctl", "is-active", "--quiet", "warp-svc.service"],
                Duration::from_secs(10),
            ) == 0
    }

    fn esperar_servicio(&self) -> Result<(), ErrorProteccion> {
        if !self.servicio_activo() && !self.servicio("start") {
            return Err(ErrorProteccion("No se pudo prender el servicio de WARP.".into()));
        }
        for _ in 0..20 {
            // recién prendido, tarda en contestar
            if self.cli(&["status"], Duration::from_secs(5)).0 == 0 {
                return Ok(());
            }
            (self.dormir)(Duration::from_millis(500));
        }
        Err(ErrorProteccion("El servicio de WARP no contesta.".into()))
    }

    fn esperar_estado(&self, limite: Duration) -> bool {
        let fin = Instant::now() + limite;
        while Instant::now() < fin {
            if self
                .cli(&["status"], Duration::from_secs(5))
                .1
                .contains("Connected")
            {
                return true;
            }
            (self.dormir)(Duration::from_secs(1));
        }
        false
    }

    /// Recién conectado, «Connected» puede llegar antes de que el túnel deje pasar tráfico: se le da tiempo.
    fn traza_confirma(&self, intentos: u32) -> bool {
        for i in 0..intentos {
            if i > 0 {
                (self.dormir)(Duration::from_secs(2));
            }
            if (self.traza)() {
                return true;
            }
        }
        false
    }

    fn enlaces_de_salida(&self) -> Vec<String> {
        static DEV: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bdev (\S+)").unwrap());
        let mut enlaces: Vec<String> = Vec::new();
        for familia in ["-4", "-6"] {
            let s = self.sistema.correr(
                &["ip", familia, "-o", "route", "show", "default"],
                Duration::from_secs(5),
            );
            for c in DEV.captures_iter(&s.salida) {
                if !enlaces.iter().any(|e| *e == c[1]) {
                    enlaces.push(c[1].to_string());
                }
            }
        }
        enlaces
    }

    fn dns_del_enlace(&self, si: bool) -> bool {
        if ES_WINDOWS || self.sistema.buscar("resolvectl").is_none() {
            return true;
        }
        let mut bien = true;
        for enlace in self.enlaces_de_salida() {
            let ok = self.sistema.codigo(
                &[
                    "resolvectl",
                    "--no-ask-password",
                    "default-route",
                    &enlace,
                    if si { "true" } else { "false" },
                ],
                Duration::from_secs(10),
            ) == 0;
            bien = ok && bien;
        }
        bien
    }
}

impl Proteccion for Warp {
    fn instalado(&self) -> bool {
        self.ruta_warp().is_some()
    }

    /// Sin tocar nada: en reposo el servicio está apagado y preguntar no debe prenderlo.
    fn conectado(&self) -> bool {
        if !self.instalado() || !self.servicio_activo() {
            return false;
        }
        self.cli(&["status"], Duration::from_secs(8))
            .1
            .contains("Connected")
    }

    /// «cifrado», «dns» o nada. Mira lo que ya está prendido.
    fn modo(&self) -> Modo {
        if !self.conectado() {
            return Modo::Ninguno;
        }
        if self.traza_confirma(2) {
            // una respuesta perdida no debe hacer creer que dejó de cifrar
            return Modo::Cifrado;
        }
        let (codigo, salida) = self.cli(&["settings", "list"], Duration::from_secs(8));
        // El texto real es «(user set)  Mode: DnsOverHttps». Hay otra línea, «DoH MSS clamp…», que sale en
        // todos los modos.
        if codigo == 0 && MODO_DOH.is_match(&salida) {
            Modo::Dns
        } else {
            Modo::Ninguno
        }
    }

    /// Prende el servicio y registra este equipo la primera vez (gratis y sin cuenta).
    fn preparar(&self) -> Result<(), ErrorProteccion> {
        if !self.instalado() {
            return Err(ErrorProteccion("Falta el cliente de Cloudflare WARP.".into()));
        }
        self.esperar_servicio()?;
        let (codigo, salida) = self.cli(&["registration", "show"], Duration::from_secs(25));
        if codigo != 0 || salida.contains("Missing") {
            let (codigo, _) = self.cli(&["registration", "new"], Duration::from_secs(40));
            if codigo != 0 {
                return Err(ErrorProteccion(
                    "No se pudo registrar en WARP. ¿Ya hay internet?".into(),
                ));
            }
        }
        Ok(())
    }

    /// Todo cifrado hasta Cloudflare. El protocolo lo elige WARP: forzarlo probando MASQUE y luego WireGuard
    /// tarda en aplicarse y, con el cliente real, dejó el túnel sin poder conectar en los intentos siguientes.
    fn activar_cifrado(&self) -> bool {
        let t = Duration::from_secs(25);
        self.cli(&["disconnect"], t);
        self.cli(&["mode", "warp"], t);
        self.cli(&["connect"], t);
        self.esperar_estado(ESPERA_CIFRADO) && self.traza_confirma(4)
    }

    /// El respaldo: solo van cifradas las búsquedas de nombres (qué sitios abres), no lo demás.
    fn activar_solo_dns(&self) -> bool {
        let t = Duration::from_secs(25);
        self.cli(&["disconnect"], t);
        self.cli(&["mode", "doh"], t);
        self.cli(&["connect"], t);
        self.esperar_estado(Duration::from_secs(10))
    }

    /// WARP cambia resolv.conf, pero systemd-resolved le sigue preguntando también al DNS del wifi, y eso va
    /// en claro: quien controla la red vería el nombre de cada sitio. Se le quita al enlace mientras dure la
    /// protección.
    fn blindar_dns(&self) -> bool {
        self.dns_del_enlace(false)
    }

    fn apagar(&self) {
        if self.instalado() && self.servicio_activo() {
            self.cli(&["disconnect"], Duration::from_secs(25));
            self.servicio("stop");
        }
        self.dns_del_enlace(true);
    }
}
