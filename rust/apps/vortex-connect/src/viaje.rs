//! El modo viaje: corre aparte de la ventana, sin Qt. Se despierta cuando cambia la red y hace una vuelta
//! de `Connect::revisar`. Solo corre si la persona lo prendió.
//!
//! En Linux lo despierta «nmcli monitor», así que no gasta nada mientras no cambie la red; Windows no tiene
//! eso y revisa cada 20 s.

use crate::ES_WINDOWS;
use crate::motor::{Cancelar, Candado, Connect, Estado, Situacion};
use crate::sistema::Sistema;
use crate::{APP_ID, NOMBRE};
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const SERVICIO: &str = "vortex-connect-viaje.service";
/// una conexión dispara varios cambios seguidos
pub const CALMA: Duration = Duration::from_secs(3);
pub const REPASO: Duration = Duration::from_secs(60);
pub const ENFRIAR_INTENTO: Duration = Duration::from_secs(300);

/// Los avisos del escritorio (`notify-send`). El del portal se cierra solo al pasar el portal: si no,
/// seguiría pidiendo algo que ya está hecho.
#[derive(Clone)]
pub struct Avisos {
    sistema: Sistema,
    id_portal: Arc<Mutex<String>>,
}

impl Avisos {
    pub fn new(sistema: Sistema) -> Self {
        Avisos {
            sistema,
            id_portal: Arc::default(),
        }
    }

    fn cerrar_portal(&self) {
        let previo = std::mem::take(&mut *self.id_portal.lock().unwrap());
        if previo.is_empty() || self.sistema.buscar("gdbus").is_none() {
            return;
        }
        // Cerrarlo también termina al notify-send que espera el botón.
        self.sistema.correr(
            &[
                "gdbus",
                "call",
                "--session",
                "--dest",
                "org.freedesktop.Notifications",
                "--object-path",
                "/org/freedesktop/Notifications",
                "--method",
                "org.freedesktop.Notifications.CloseNotification",
                &previo,
            ],
            Duration::from_secs(5),
        );
    }

    pub fn notificar(&self, texto: &str, urgente: bool) {
        self.cerrar_portal();
        if ES_WINDOWS || self.sistema.buscar("notify-send").is_none() {
            return;
        }
        self.sistema.correr(
            &[
                "notify-send",
                "-a",
                NOMBRE,
                "-i",
                APP_ID,
                "-u",
                if urgente { "critical" } else { "normal" },
                NOMBRE,
                texto,
            ],
            Duration::from_secs(10),
        );
    }

    /// Nunca abre el navegador por su cuenta en Linux: quizá no estás viendo, y el botón está en el aviso.
    pub fn ofrecer_portal(&self, url: &str) {
        let Some(notify) = self.sistema.buscar("notify-send") else {
            self.sistema.abrir_url(url);
            return;
        };
        if ES_WINDOWS {
            self.sistema.abrir_url(url);
            return;
        }
        self.cerrar_portal();
        let este = self.clone();
        let url = url.to_string();
        std::thread::spawn(move || {
            let hijo = Command::new(notify)
                .args([
                    "-p",
                    "-a",
                    NOMBRE,
                    "-i",
                    APP_ID,
                    "-u",
                    "critical",
                    "-A",
                    "default=Ver",
                    "-A",
                    "abrir=Abrir la página",
                    "Esta red pide aceptar algo",
                    "Ábrela para tener internet. Lo que escribas ahí no va cifrado.",
                ])
                .env("PATH", este.sistema.path.clone().unwrap_or_default())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut hijo) = hijo else { return };
            let mut lineas = BufReader::new(hijo.stdout.take().unwrap()).lines();
            // con -p, lo primero es el número del aviso
            *este.id_portal.lock().unwrap() = lineas.next().and_then(Result::ok).unwrap_or_default();
            let accion = lineas.next().and_then(Result::ok).unwrap_or_default();
            let _ = hijo.wait();
            if accion.trim() == "abrir" || accion.trim() == "default" {
                este.sistema.abrir_url(&url);
            }
        });
    }
}

/// Lo que pasó en una vuelta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Vuelta {
    /// el modo viaje se apagó: el servicio termina
    Terminar,
    /// no hizo falta nada (o no se podía)
    Nada,
    Hecha(Estado),
}

pub struct Vigia {
    pub connect: Connect,
    avisos: Avisos,
    candado: Candado,
    ultimo_intento: Option<Instant>,
    ultimo_final: Option<Situacion>,
}

impl Vigia {
    pub fn new(connect: Connect, avisos: Avisos, candado: Candado) -> Self {
        Vigia {
            connect,
            avisos,
            candado,
            ultimo_intento: None,
            ultimo_final: None,
        }
    }

    /// Una vuelta: relee los ajustes y, si toca, revisa la red (con el candado, para no pisar a la ventana).
    pub fn una_vuelta(&mut self) -> Vuelta {
        self.connect.releer_ajustes();
        if !self.connect.ajustes().modo_viaje {
            return Vuelta::Terminar;
        }
        let protegido = matches!(self.ultimo_final, Some(Situacion::Protegido | Situacion::SoloDns));
        if self.ultimo_intento.is_some_and(|t| t.elapsed() < ENFRIAR_INTENTO) && !protegido {
            return Vuelta::Nada; // el intento anterior no pudo: no insistir con cada cambio de la red
        }
        if !self.candado.tomar() {
            return Vuelta::Nada; // la ventana está trabajando
        }
        let resultado = self.connect.revisar(&Cancelar::new());
        self.candado.soltar();
        let Some(r) = resultado else {
            return Vuelta::Nada;
        };
        self.ultimo_intento = Some(Instant::now());
        self.ultimo_final = Some(r.situacion);
        let nombre = if r.red.ssid.is_empty() {
            "esta red"
        } else {
            &r.red.ssid
        };
        match r.situacion {
            Situacion::Protegido => self.avisos.notificar(
                &format!("Entraste a «{nombre}» y lo que mandas va cifrado."),
                false,
            ),
            Situacion::SoloDns => self.avisos.notificar(
                &format!("En «{nombre}» solo va cifrado el DNS: esa red bloquea lo demás."),
                true,
            ),
            _ => {}
        }
        Vuelta::Hecha(r)
    }
}

fn despertar_con_nmcli(sistema: &Sistema, tx: Sender<()>) -> Option<Child> {
    let ruta = sistema.buscar("nmcli")?;
    let mut hijo = Command::new(ruta)
        .arg("monitor")
        .env("PATH", sistema.path.clone().unwrap_or_default())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let salida = hijo.stdout.take()?;
    std::thread::spawn(move || {
        for _ in BufReader::new(salida).lines() {
            if tx.send(()).is_err() {
                break;
            }
        }
    });
    Some(hijo)
}

fn vaciar(rx: &Receiver<()>) {
    while rx.try_recv().is_ok() {}
}

/// El servicio del modo viaje (`vortex-connect vigilar`). Devuelve el código de salida.
pub fn vigilar(sistema: Sistema) -> i32 {
    let connect = Connect::real(sistema.clone());
    let a = connect.ajustes();
    if !a.modo_viaje || !a.condiciones {
        return 0;
    }
    let dicho = Arc::new(Mutex::new(String::new()));
    let d = dicho.clone();
    connect.poner_avisar(Arc::new(move |texto| {
        let mut d = d.lock().unwrap();
        if *d != texto {
            *d = texto.to_string();
            println!("{texto}");
        }
    }));
    let avisos = Avisos::new(sistema.clone());
    let av = avisos.clone();
    connect.poner_abrir_portal(Arc::new(move |url| av.ofrecer_portal(url)));
    let (tx, rx) = channel();
    let _ = tx.send(()); // la primera vuelta, de una
    let mut monitor = if ES_WINDOWS {
        None
    } else {
        despertar_con_nmcli(&sistema, tx)
    };
    let mut vigia = Vigia::new(connect, avisos, Candado::new());
    let codigo = loop {
        let espera = if monitor.is_none() {
            Duration::from_secs(20)
        } else {
            REPASO
        };
        if let Err(RecvTimeoutError::Disconnected) = rx.recv_timeout(espera) {
            // «nmcli monitor» se cerró: se revisa de vez en cuando, sin dar vueltas sin parar
            std::thread::sleep(espera);
        }
        vaciar(&rx);
        std::thread::sleep(CALMA);
        vaciar(&rx);
        if vigia.una_vuelta() == Vuelta::Terminar {
            break 0;
        }
    };
    if let Some(m) = monitor.as_mut() {
        let _ = m.kill();
        let _ = m.wait();
    }
    codigo
}

/// Lo deja corriendo y que arranque solo al entrar a tu sesión, o lo apaga del todo.
pub fn prender(sistema: &Sistema, si: bool) -> bool {
    if ES_WINDOWS {
        return prender_windows(sistema, si);
    }
    sistema.codigo(
        &[
            "systemctl",
            "--user",
            if si { "enable" } else { "disable" },
            "--now",
            SERVICIO,
        ],
        Duration::from_secs(20),
    ) == 0
}

fn prender_windows(sistema: &Sistema, si: bool) -> bool {
    const CLAVE: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let exe = exe.to_string_lossy().into_owned();
    let t = Duration::from_secs(15);
    if si {
        let valor = format!("\"{exe}\" vigilar");
        let bien = sistema.codigo(
            &[
                "reg",
                "add",
                CLAVE,
                "/v",
                "VortexineConnect",
                "/t",
                "REG_SZ",
                "/d",
                &valor,
                "/f",
            ],
            t,
        ) == 0;
        if bien {
            let _ = Command::new(&exe).arg("vigilar").spawn();
        }
        bien
    } else {
        // al apagarlo, el que corre se entera solo: lee los ajustes en cada vuelta
        sistema.codigo(&["reg", "delete", CLAVE, "/v", "VortexineConnect", "/f"], t);
        true
    }
}
