//! El avance (como `avance.py`): 30 s de una canción de YouTube Music antes de bajarla, desde un poco antes de
//! la mitad. Es un mpv aparte: no toca la cola ni la canción de turno. Mientras suena, Vortex Player pausa tu
//! música y al terminar la sigue (eso lo hace el puente con `Empezo` y `Termino`).
//!
//! yt-dlp saca el audio con el cliente «visionos» de YouTube: con el de siempre YouTube pide esperar 5 s (un
//! anuncio). Si ese cliente deja de servir, se intenta una vez con el de siempre.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::mpv::CLIENTE_YOUTUBE;
use crate::rutas;

/// Segundos.
pub const DURA: i64 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum DeAvance {
    Cambio,
    /// Se va a abrir mpv (el puente pausa tu música).
    Empezo,
    /// Se acabó o lo paraste (el puente la sigue).
    Termino,
    Error(String),
}

type Aviso = Arc<dyn Fn(DeAvance) + Send + Sync>;

#[derive(Default)]
struct Estado {
    id: String,
    sonando: bool,
    args: Vec<String>,
    con_cliente: bool,
    hijo: Option<Child>,
    /// Cada mpv lleva su número: lo que llega de uno viejo se ignora.
    generacion: u64,
}

#[derive(Clone)]
pub struct Avance {
    e: Arc<Mutex<Estado>>,
    al: Aviso,
    /// El mpv (`VORTEX_PLAYER_MPV` o `mpv`).
    pub programa: Option<String>,
}

impl Avance {
    pub fn new(al: impl Fn(DeAvance) + Send + Sync + 'static) -> Self {
        Self {
            e: Arc::default(),
            al: Arc::new(al),
            programa: None,
        }
    }

    fn estado(&self) -> std::sync::MutexGuard<'_, Estado> {
        self.e.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn id_actual(&self) -> String {
        self.estado().id.clone()
    }

    pub fn sonando(&self) -> bool {
        self.estado().sonando
    }

    /// Toca el avance de `vid` (si ya sonaba otro, lo cambia sin que tu música se cuele en medio).
    pub fn tocar(&self, vid: &str, url: &str, duracion: f64, volumen: i64) {
        let habia = {
            let mut e = self.estado();
            let habia = !e.id.is_empty();
            if habia {
                e.generacion += 1; // el viejo ya no avisa
                if let Some(mut h) = e.hijo.take() {
                    let _ = h.kill();
                    let _ = h.wait();
                }
            }
            habia
        };
        if !habia {
            (self.al)(DeAvance::Empezo);
        }
        let inicio = if duracion >= 90.0 {
            (duracion * 0.35) as i64
        } else {
            0
        };
        {
            let mut e = self.estado();
            e.args = vec![
                "--no-video".into(),
                "--no-config".into(),
                "--msg-level=all=error,cplayer=info".into(),
                "--term-status-msg=".into(),
                format!("--ao={}", rutas::salida_audio()),
                "--volume-max=100".into(),
                format!("--volume={}", volumen.clamp(0, 100)),
                "--ytdl-format=bestaudio".into(),
                format!("--start={inicio}"),
                format!("--end={}", inicio + DURA),
                "--audio-client-name=Vortex Player".into(),
                url.into(),
            ];
            e.id = vid.into();
            e.sonando = false;
            e.con_cliente = true;
        }
        self.arrancar();
        (self.al)(DeAvance::Cambio);
    }

    fn arrancar(&self) {
        let programa = self
            .programa
            .clone()
            .or_else(|| std::env::var("VORTEX_PLAYER_MPV").ok().filter(|v| !v.is_empty()))
            .unwrap_or_else(|| "mpv".into());
        let mut e = self.estado();
        e.generacion += 1;
        let g = e.generacion;
        let mut args: Vec<String> = Vec::new();
        if e.con_cliente {
            args.push(format!("--ytdl-raw-options={CLIENTE_YOUTUBE}"));
        }
        args.extend(e.args.iter().cloned());
        let hijo = std::io::pipe().ok().and_then(|(lector, escritor)| {
            let e2 = escritor.try_clone().ok()?;
            let h = Command::new(&programa)
                .args(&args)
                .stdin(Stdio::null())
                .stdout(escritor)
                .stderr(e2)
                .spawn()
                .ok()?;
            Some((h, lector))
        });
        let Some((h, lector)) = hijo else {
            drop(e);
            self.fin(g, None);
            return;
        };
        e.hijo = Some(h);
        drop(e);
        // mpv dice «AO: [pipewire] 48000Hz…» cuando abre la salida de audio: ya suena
        let yo = self.clone();
        let _ = std::thread::Builder::new()
            .name("vortex-player-avance".into())
            .spawn(move || {
                for l in BufReader::new(lector).lines() {
                    let Ok(l) = l else { break };
                    if l.contains("AO:") {
                        let mut e = yo.estado();
                        if e.generacion == g && !e.sonando {
                            e.sonando = true;
                            drop(e);
                            (yo.al)(DeAvance::Cambio);
                        }
                    }
                }
            });
        // y otro que espera a que termine (el tubo puede seguir abierto por el yt-dlp que lanzó)
        let yo = self.clone();
        let _ = std::thread::Builder::new()
            .name("vortex-player-avance-fin".into())
            .spawn(move || {
                loop {
                    let mut e = yo.estado();
                    if e.generacion != g {
                        return;
                    }
                    let st = match e.hijo.as_mut().map(Child::try_wait) {
                        Some(Ok(Some(st))) => Some(st),
                        Some(Ok(None)) => None,
                        _ => {
                            drop(e);
                            yo.fin(g, None);
                            return;
                        }
                    };
                    if let Some(st) = st {
                        e.hijo = None;
                        drop(e);
                        yo.fin(g, Some(st));
                        return;
                    }
                    drop(e);
                    std::thread::sleep(Duration::from_millis(100));
                }
            });
    }

    fn fin(&self, g: u64, st: Option<ExitStatus>) {
        let mut e = self.estado();
        if e.generacion != g {
            return;
        }
        // salió solo con error antes de sonar (si lo matamos no cuenta: eso es parar)
        let fallo = !e.sonando && st.is_none_or(|s| s.code().is_some_and(|c| c != 0));
        if fallo && e.con_cliente {
            e.con_cliente = false;
            drop(e);
            self.arrancar();
            return;
        }
        e.id.clear();
        e.sonando = false;
        e.hijo = None;
        drop(e);
        if fallo {
            (self.al)(DeAvance::Error("No pude escuchar el avance".into()));
        }
        (self.al)(DeAvance::Cambio);
        (self.al)(DeAvance::Termino);
    }

    /// Lo para (llega `Termino`).
    pub fn parar(&self) {
        if let Some(h) = self.estado().hijo.as_mut() {
            let _ = h.kill();
        }
    }
}
