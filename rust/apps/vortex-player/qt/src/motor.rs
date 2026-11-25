//! El reproductor de verdad (el de `vortex_player::reproductor`, sin Qt) y su mpv, en el hilo de la
//! interfaz. `rep` (la cara para QML) y `vp` (el puente) lo usan con `con_rep`: corre lo pedido, le pasa a
//! mpv lo que dejó, y encola a `rep` la foto de lo que cambió y a `vp` los avisos (las señales de la de
//! Python). Nunca se llama a Qt con el reproductor prestado: todo va por `queue`.

use crate::puente::qobject::Puente;
use crate::rep::{Foto, qobject::Rep};
use cxx_qt::CxxQtThread;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use vortex_player::biblioteca::Biblioteca;
use vortex_player::discord::{Opciones, Presencia, Sonando};
use vortex_player::mpris::{EstadoMpris, Mpris, OrdenMpris};
use vortex_player::reproductor::{Aviso, DeMpv, MpvProceso, Reproductor};

/// Cada cuánto corre `Reproductor::tic` (el temporizador para dormir y el crossfade), solo mientras hace falta.
const TIC: Duration = Duration::from_millis(250);

struct Motor {
    r: Reproductor,
    mpv: Option<MpvProceso>,
    rep: Option<CxxQtThread<Rep>>,
    vp: Option<CxxQtThread<Puente>>,
    mpris: Option<Mpris>,
    discord: Option<Presencia>,
    discord_activo: bool,
    tic: Arc<AtomicBool>,
}

thread_local! {
    static MOTOR: RefCell<Motor> = RefCell::new(Motor {
        r: Reproductor::new(),
        mpv: None,
        rep: None,
        vp: None,
        mpris: None,
        discord: None,
        discord_activo: false,
        tic: Arc::new(AtomicBool::new(false)),
    });
}

/// Para leer el reproductor (cola, canción…) sin cambiarlo.
pub fn leer<T>(f: impl FnOnce(&Reproductor) -> T) -> T {
    MOTOR.with_borrow(|m| f(&m.r))
}

/// Corre `f` sobre el reproductor y reparte lo que cambió.
pub fn con_rep<T>(f: impl FnOnce(&mut Reproductor) -> T) -> T {
    let (t, avisos, foto, rep, vp) = MOTOR.with_borrow_mut(|m| {
        let t = f(&mut m.r);
        if let Some(p) = m.mpv.as_mut() {
            p.despachar(&mut m.r);
        }
        m.revisar_tic();
        let avisos = std::mem::take(&mut m.r.avisos);
        if avisos.is_empty() {
            return (t, avisos, None, None, None);
        }
        m.publicar(&avisos);
        let foto = Foto::de(&m.r, &avisos);
        (t, avisos, Some(foto), m.rep.clone(), m.vp.clone())
    });
    if let (Some(foto), Some(rep)) = (foto, rep) {
        let _ = rep.queue(move |o| o.aplicar(foto));
    }
    if let Some(vp) = vp {
        let _ = vp.queue(move |o| o.al_reproductor(avisos));
    }
    t
}

impl Motor {
    /// MPRIS (el panel de música y las teclas) y Discord, con lo que suena.
    fn publicar(&self, avisos: &[Aviso]) {
        let c = self.r.cancion();
        if let Some(m) = &self.mpris {
            let portada = c.get("portada").and_then(|p| p.as_str()).unwrap_or("");
            m.poner(EstadoMpris {
                cancion: c.clone(),
                portada: if portada.starts_with("file:") || portada.starts_with("http") {
                    portada.to_string()
                } else {
                    String::new()
                },
                pausado: self.r.pausado(),
                repetir: self.r.repetir().to_string(),
                aleatorio: self.r.aleatorio(),
                volumen: self.r.volumen(),
                posicion: self.r.posicion(),
                duracion: self.r.duracion(),
                hay_siguiente: self.r.hay_siguiente(),
                hay_anterior: self.r.hay_anterior(),
            });
        }
        if let Some(d) = &self.discord {
            d.poner(self.sonando(c));
            if avisos
                .iter()
                .any(|a| matches!(a, Aviso::Cancion | Aviso::Estado | Aviso::Practica))
            {
                d.actualizar();
            }
        }
    }

    fn sonando(&self, cancion: serde_json::Value) -> Sonando {
        Sonando {
            cancion,
            posicion: self.r.posicion(),
            duracion: self.r.duracion(),
            pausado: self.r.pausado(),
            velocidad: self.r.velocidad(),
            activa: self.discord_activo,
        }
    }

    /// Arranca (o deja que se apague) el hilo del tic.
    fn revisar_tic(&self) {
        if !self.r.necesita_tic() {
            self.tic.store(false, Ordering::Relaxed);
            return;
        }
        let Some(rep) = self.rep.clone() else { return };
        if self.tic.swap(true, Ordering::Relaxed) {
            return; // ya corre
        }
        let sigue = self.tic.clone();
        std::thread::spawn(move || {
            while sigue.load(Ordering::Relaxed) {
                std::thread::sleep(TIC);
                if rep.queue(|_| con_rep(|r| r.tic())).is_err() {
                    break;
                }
            }
        });
    }
}

fn al_mpv(hilo: CxxQtThread<Rep>) -> impl Fn(DeMpv) + Send + Sync + 'static {
    move |d| {
        let _ = hilo.queue(move |_| de_mpv(d));
    }
}

fn de_mpv(d: DeMpv) {
    match d {
        DeMpv::Conectado => con_rep(|r| r.al_conectar()),
        DeMpv::Evento(v) => con_rep(|r| r.al_evento(&v)),
        DeMpv::NoArranco => con_rep(|r| {
            r.avisos
                .push(Aviso::Error("No pude arrancar mpv (¿está instalado?)".into()))
        }),
        DeMpv::Murio => {
            con_rep(|r| r.mpv_murio());
            let Some(rep) = MOTOR.with_borrow(|m| m.rep.clone()) else {
                return;
            };
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(500));
                let h = rep.clone();
                let _ = rep.queue(move |_| {
                    MOTOR.with_borrow(|m| {
                        if let Some(p) = &m.mpv {
                            p.relevantar(m.r.volumen(), al_mpv(h));
                        }
                    });
                });
            });
        }
    }
}

/// `rep` ya existe: arranca mpv (con tu volumen) y le manda a `rep` cómo está todo.
pub fn registrar_rep(hilo: CxxQtThread<Rep>) {
    let volumen = Biblioteca::ajustes().volumen();
    let foto = MOTOR.with_borrow_mut(|m| {
        m.rep = Some(hilo.clone());
        m.r.poner_volumen(volumen);
        m.r.salida.clear(); // aún no hay mpv: arranca ya con ese volumen
        m.r.avisos.clear();
        m.mpv = Some(MpvProceso::arrancar(volumen, al_mpv(hilo.clone())));
        Foto::completa(&m.r)
    });
    let _ = hilo.queue(move |o| o.aplicar(foto));
}

pub fn registrar_vp(hilo: CxxQtThread<Puente>) {
    MOTOR.with_borrow_mut(|m| m.vp = Some(hilo));
}

/// MPRIS y la presencia en Discord (no en las pruebas: no tocan el escritorio de nadie).
pub fn arrancar_extras(discord: bool) {
    let (tx, rx) = std::sync::mpsc::channel();
    let mpris = Mpris::arrancar(None, tx);
    let rep = MOTOR.with_borrow(|m| m.rep.clone());
    if mpris.is_some()
        && let Some(rep) = rep
    {
        std::thread::spawn(move || {
            for orden in rx {
                if rep.queue(move |_| ordenar(orden)).is_err() {
                    break;
                }
            }
        });
    } else if mpris.is_none() {
        eprintln!("Vortex Player: sin MPRIS");
    }
    MOTOR.with_borrow_mut(|m| {
        m.mpris = mpris;
        m.discord_activo = discord;
        m.discord = Some(Presencia::arrancar(Opciones::default()));
        m.publicar(&[Aviso::Cancion]);
    });
}

fn ordenar(orden: OrdenMpris) {
    match orden {
        OrdenMpris::Siguiente => con_rep(|r| r.siguiente()),
        OrdenMpris::Anterior => con_rep(|r| r.anterior()),
        OrdenMpris::Pausar => con_rep(|r| r.pausar()),
        OrdenMpris::Alternar => con_rep(|r| r.alternar()),
        OrdenMpris::Seguir => con_rep(|r| r.seguir()),
        OrdenMpris::Buscar(s) => con_rep(|r| r.buscar(s)),
        OrdenMpris::Volumen(v) => con_rep(|r| r.poner_volumen(v)),
        OrdenMpris::Aleatorio(a) => con_rep(|r| {
            if r.aleatorio() != a {
                r.alternar_aleatorio();
            }
        }),
        OrdenMpris::Repetir(modo) => con_rep(|r| {
            for _ in 0..3 {
                if r.repetir() == modo {
                    break;
                }
                r.alternar_repetir();
            }
        }),
        OrdenMpris::Levantar => {
            if let Some(vp) = MOTOR.with_borrow(|m| m.vp.clone()) {
                let _ = vp.queue(|o| o.mostrar_ventana());
            }
        }
        OrdenMpris::Salir => crate::arranque::ffi::salir_de_la_app(),
    }
}

/// El interruptor de Discord en Ajustes.
pub fn poner_discord(activo: bool) {
    MOTOR.with_borrow_mut(|m| {
        m.discord_activo = activo;
        if let Some(d) = &m.discord {
            d.poner(m.sonando(m.r.cancion()));
            d.actualizar();
        }
    });
}

/// Al salir: cierra mpv y lo demás. Devuelve la última escucha (cuenta para «Tu resumen»): ya no hay
/// vuelta del ciclo de eventos para encolarla.
pub fn cerrar() -> Vec<Aviso> {
    MOTOR.with_borrow_mut(|m| {
        m.r.cerrar_escucha();
        m.tic.store(false, Ordering::Relaxed);
        m.mpris = None;
        m.discord = None;
        if let Some(mut p) = m.mpv.take() {
            p.cerrar();
        }
        std::mem::take(&mut m.r.avisos)
    })
}
