//! Cliente IPC JSON de `mpv` y soporte para el segundo `mpv` de crossfade.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use thiserror::Error;

use crate::rutas;

/// Cliente `visionos` para evitar la espera de 5 s en YouTube al reproducir streams.
pub const CLIENTE_YOUTUBE: &str = "extractor-args=youtube:player_client=visionos";

#[derive(Debug, Error)]
pub enum ErrorMpv {
    #[error("No se pudo arrancar mpv: {0}")]
    Arranque(std::io::Error),
    #[error("No se pudo conectar al socket IPC de mpv en {0}")]
    ConexionTimeout(String),
    #[error("Error de comunicación IPC con mpv: {0}")]
    Ipc(#[from] std::io::Error),
    #[error("Error serializando comando JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Evento recibido por el socket JSON IPC de `mpv`.
#[derive(Debug, Clone, PartialEq)]
pub enum EventoMpv {
    CambioPropiedad { nombre: String, valor: Value },
    FinArchivo { razon: String },
    Otro(Value),
}

impl EventoMpv {
    /// Parsea una línea JSON recibida desde el socket de `mpv`.
    pub fn desde_json(valor: Value) -> Self {
        match valor.get("event").and_then(Value::as_str) {
            Some("property-change") => Self::CambioPropiedad {
                nombre: valor
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                valor: valor.get("data").cloned().unwrap_or(Value::Null),
            },
            Some("end-file") => Self::FinArchivo {
                razon: valor
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            },
            _ => Self::Otro(valor),
        }
    }
}

/// Construye el filtro `af` combinando ecualizador (`eq`) y `scaletempo2` si la velocidad != 1.
pub fn construir_filtros(eq: &str, velocidad: f64, entra_fade: Option<f64>) -> String {
    let mut partes = Vec::new();
    if !eq.is_empty() {
        partes.push(eq.to_string());
    }
    if (velocidad - 1.0).abs() > f64::EPSILON {
        partes.push("scaletempo2".to_string());
    }
    if let Some(d) = entra_fade
        && d > 0.0
    {
        partes.push(format!("lavfi=[afade=t=in:d={d:.1}]"));
    }
    partes.join(",")
}

/// Indica si una ruta es un stream remoto HTTP/HTTPS.
pub fn es_stream(ruta: &str) -> bool {
    ruta.starts_with("http://") || ruta.starts_with("https://")
}

/// Opciones para cargar una pista en `MpvIpc::cargar`.
#[derive(Debug, Clone, Copy)]
pub struct OpcionesCarga<'a> {
    pub pausado: bool,
    pub desde: f64,
    pub entra_fade: Option<f64>,
    pub eq: &'a str,
    pub velocidad: f64,
    pub usar_cliente_yt: bool,
}

impl Default for OpcionesCarga<'_> {
    fn default() -> Self {
        Self {
            pausado: false,
            desde: 0.0,
            entra_fade: None,
            eq: "",
            velocidad: 1.0,
            usar_cliente_yt: false,
        }
    }
}

/// Proceso `mpv` controlado por socket JSON IPC.
pub struct MpvIpc {
    proceso: Child,
    ipc_ruta: String,
    carpeta_privada: Option<PathBuf>,
    #[cfg(unix)]
    escritor: UnixStream,
    #[cfg(unix)]
    lector: BufReader<UnixStream>,
    cola_fundido: Vec<Child>,
}

fn carpeta_socket_privada(pid: u32) -> Result<(PathBuf, Option<PathBuf>), ErrorMpv> {
    if cfg!(windows) {
        return Ok((PathBuf::new(), None));
    }
    if let Some(rt) = vortex_core::rutas::runtime() {
        let sub = rt.join(format!("vortex-player-ipc-{pid}"));
        let probe = sub.join("mpv.sock");
        if probe.to_string_lossy().len() <= 100 {
            fs::create_dir_all(&sub)?;
            #[cfg(unix)]
            fs::set_permissions(&sub, fs::Permissions::from_mode(0o700))?;
            return Ok((sub.clone(), Some(sub)));
        }
    }
    let tmp_priv = env::temp_dir().join(format!("vp-ipc-{pid}"));
    fs::create_dir_all(&tmp_priv)?;
    #[cfg(unix)]
    fs::set_permissions(&tmp_priv, fs::Permissions::from_mode(0o700))?;
    Ok((tmp_priv.clone(), Some(tmp_priv)))
}

impl MpvIpc {
    /// Arranca `mpv --idle=yes` con socket IPC en carpeta privada `0700` y observa las propiedades esenciales.
    pub fn iniciar(volumen: i64) -> Result<Self, ErrorMpv> {
        Self::iniciar_con(volumen, None)
    }

    /// Arranca `mpv --idle=yes` permitiendo forzar la salida de audio (`ao_override`, útil en pruebas).
    pub fn iniciar_con(volumen: i64, ao_override: Option<&str>) -> Result<Self, ErrorMpv> {
        let mpv_bin = env::var("VORTEX_PLAYER_MPV")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "mpv".to_string());
        let ao = ao_override
            .map(str::to_string)
            .unwrap_or_else(rutas::salida_audio);

        let pid = std::process::id();
        let (carpeta_ipc, carpeta_privada) = carpeta_socket_privada(pid)?;
        let (ipc, socket_ruta) = rutas::canal_mpv("mpv", &carpeta_ipc);

        if !cfg!(windows) && Path::new(&socket_ruta).exists() {
            let _ = fs::remove_file(&socket_ruta);
        }

        let vol = volumen.clamp(0, 100);
        let mut proceso = Command::new(&mpv_bin)
            .args([
                "--idle=yes",
                "--no-video",
                "--no-terminal",
                "--no-config",
                "--audio-display=no",
                &format!("--ao={ao}"),
                "--volume-max=100",
                &format!("--volume={vol}"),
                "--replaygain=no",
                "--audio-pitch-correction=no",
                "--gapless-audio=weak",
                "--audio-client-name=Vortex Player",
                "--pause=yes",
                "--ytdl-format=bestaudio",
                &format!("--input-ipc-server={ipc}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ErrorMpv::Arranque)?;

        #[cfg(unix)]
        let (escritor, lector) = {
            let inicio = Instant::now();
            let limite = Duration::from_secs(9);
            let stream = loop {
                if Path::new(&socket_ruta).exists()
                    && let Ok(s) = UnixStream::connect(&socket_ruta)
                {
                    break s;
                }
                if inicio.elapsed() >= limite {
                    let _ = proceso.kill();
                    let _ = proceso.wait();
                    return Err(ErrorMpv::ConexionTimeout(socket_ruta));
                }
                thread::sleep(Duration::from_millis(50));
            };
            stream.set_read_timeout(Some(Duration::from_millis(100)))?;
            let clon = stream.try_clone()?;
            (stream, BufReader::new(clon))
        };

        let mut cliente = Self {
            proceso,
            ipc_ruta: socket_ruta,
            carpeta_privada,
            #[cfg(unix)]
            escritor,
            #[cfg(unix)]
            lector,
            cola_fundido: Vec::new(),
        };

        for (id, nombre) in [(1, "time-pos"), (2, "duration"), (3, "pause"), (4, "idle-active")] {
            cliente.enviar(&json!({ "command": ["observe_property", id, nombre] }))?;
        }

        Ok(cliente)
    }

    /// Ruta del socket IPC (en Unix) o nombre de tubería (en Windows).
    pub fn ipc_ruta(&self) -> &str {
        &self.ipc_ruta
    }

    /// Envía un mensaje JSON (una línea) al socket de `mpv`.
    pub fn enviar(&mut self, msg: &Value) -> Result<(), ErrorMpv> {
        let mut linea = serde_json::to_string(msg)?;
        linea.push('\n');
        #[cfg(unix)]
        {
            self.escritor.write_all(linea.as_bytes())?;
            self.escritor.flush()?;
        }
        Ok(())
    }

    /// Carga un archivo o stream en el `mpv` principal.
    pub fn cargar(&mut self, ruta: &str, opts: OpcionesCarga<'_>) -> Result<(), ErrorMpv> {
        let mut opciones: HashMap<String, String> = HashMap::new();
        if opts.desde > 0.0 {
            opciones.insert("start".into(), format!("{:.2}", opts.desde));
        }
        let af = construir_filtros(opts.eq, opts.velocidad, opts.entra_fade);
        if !af.is_empty() && opts.entra_fade.is_some() {
            opciones.insert("af".into(), af);
        }
        if es_stream(ruta) && opts.usar_cliente_yt {
            opciones.insert("ytdl-raw-options".into(), CLIENTE_YOUTUBE.into());
        }

        if opciones.is_empty() {
            self.enviar(&json!({ "command": ["loadfile", ruta, "replace"] }))?;
        } else {
            self.enviar(&json!({ "command": ["loadfile", ruta, "replace", -1, opciones] }))?;
        }
        self.enviar(&json!({ "command": ["set_property", "pause", opts.pausado] }))?;
        Ok(())
    }

    /// Lanza el segundo `mpv` que reproduce el tramo final apagándose (`afade=t=out`).
    pub fn lanzar_fundido_salida(
        &mut self,
        ruta: &str,
        desde: f64,
        resto: f64,
        volumen: i64,
        eq: &str,
    ) -> Result<(), ErrorMpv> {
        let mpv_bin = env::var("VORTEX_PLAYER_MPV")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "mpv".to_string());
        let ao = rutas::salida_audio();
        let vol = volumen.clamp(0, 100);
        let fade = format!("lavfi=[afade=t=out:st={desde:.2}:d={resto:.2}]");
        let af = if eq.is_empty() {
            fade
        } else {
            format!("{eq},{fade}")
        };

        let hijo = Command::new(mpv_bin)
            .args([
                "--no-video",
                "--no-terminal",
                "--no-config",
                "--audio-display=no",
                &format!("--ao={ao}"),
                &format!("--volume={vol}"),
                "--volume-max=100",
                "--replaygain=no",
                "--audio-client-name=Vortex Player",
                &format!("--start={desde:.2}"),
                &format!("--af={af}"),
                "--",
                ruta,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(ErrorMpv::Arranque)?;

        self.cola_fundido.push(hijo);
        Ok(())
    }

    /// Limpia procesos de crossfade ya terminados.
    pub fn recolectar_fundidos(&mut self) {
        self.cola_fundido.retain_mut(|h| matches!(h.try_wait(), Ok(None)));
    }

    /// Mata cualquier proceso de crossfade activo.
    pub fn parar_fundidos(&mut self) {
        for mut h in self.cola_fundido.drain(..) {
            let _ = h.kill();
            let _ = h.wait();
        }
    }

    /// Lee los eventos disponibles actualmente sin bloquear indefinidamente.
    pub fn leer_eventos(&mut self) -> Vec<EventoMpv> {
        let mut eventos = Vec::new();
        #[cfg(unix)]
        {
            loop {
                let mut linea = String::new();
                match self.lector.read_line(&mut linea) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(v) = serde_json::from_str::<Value>(&linea) {
                            eventos.push(EventoMpv::desde_json(v));
                        }
                    }
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                        ) =>
                    {
                        break;
                    }
                    Err(_) => break,
                }
            }
        }
        eventos
    }

    /// Cierra limpiamente el `mpv` principal, los secundarios y elimina el socket Unix.
    pub fn cerrar(&mut self) {
        self.parar_fundidos();
        let _ = self.enviar(&json!({ "command": ["quit"] }));
        let inicio = Instant::now();
        while inicio.elapsed() < Duration::from_millis(1500) {
            if matches!(self.proceso.try_wait(), Ok(Some(_))) {
                break;
            }
            thread::sleep(Duration::from_millis(25));
        }
        let _ = self.proceso.kill();
        let _ = self.proceso.wait();
        if !cfg!(windows) {
            let _ = fs::remove_file(&self.ipc_ruta);
            if let Some(ref dir) = self.carpeta_privada {
                let _ = fs::remove_dir(dir);
            }
        }
    }
}

impl Drop for MpvIpc {
    fn drop(&mut self) {
        self.cerrar();
    }
}
