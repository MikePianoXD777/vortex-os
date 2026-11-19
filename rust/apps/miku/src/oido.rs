//! El oído de Miku: graba del micrófono, corta solo cuando dejas de hablar y transcribe en la propia máquina.
//!
//! - El micrófono se abre SOLO mientras `escuchar()` corre (la ventana está abierta y el indicador visible).
//!   No hay palabra de activación ni escucha de fondo.
//! - El audio vive solo en memoria: va de `pw-record` a este proceso y de aquí a `whisper-cli` por su entrada
//!   estándar. No se escribe ningún archivo.
//! - Transcribe con whisper.cpp (`whisper-cli`) y el modelo `ggml-small-q5_1` que se baja la primera vez a
//!   `~/.local/share/vortex/miku/oido/`, en español y sin internet.
//!
//! El detector de voz sigue al ruido de fondo (con preénfasis, para que el zumbido grave de un ventilador no
//! cuente como voz) y corta tras ~1,5 s de silencio. Es el mismo de `oido.py`, número por número.

use crate::{rutas::Rutas, sistema};
use regex::Regex;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{OnceLock, mpsc};
use std::time::{Duration, Instant};

pub const MODELO: &str = "ggml-small-q5_1.bin";
pub const RATE: usize = 16000;
pub const TRAMO_MS: usize = 20;
pub const TRAMO: usize = RATE * TRAMO_MS / 1000;
pub const BYTES_TRAMO: usize = TRAMO * 2;

/// Algo falló y se puede decir en español (mensaje listo para mostrar).
#[derive(Debug, Clone, PartialEq)]
pub struct ErrorOido(pub String);

impl std::fmt::Display for ErrorOido {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn marcas() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| Regex::new(r"\[[^\]]*\]|\([^)]*\)|[♪♫]+").unwrap())
}

// Whisper, cuando le llega ruido o silencio, inventa frases de sus datos de entrenamiento y marcas como
// «[Música]» o «(aplausos)». Se quitan las marcas, y si lo que queda es solo una de esas frases, no dijiste nada.
fn frases_fantasma() -> &'static Regex {
    static C: OnceLock<Regex> = OnceLock::new();
    C.get_or_init(|| {
        Regex::new(
            r"(?i)^(gracias por (ver|escuchar)( este| el)?( v[ií]deo)?|suscr[ií]bete( al canal)?|subt[ií]tulos? (realizados? )?por.*|.*amara\.org.*|nos vemos en el pr[oó]ximo( v[ií]deo)?)$",
        )
        .unwrap()
    })
}

/// Nivel en dB de un tramo de 16 bits. Preénfasis y[n] = x[n] - 0,95·x[n-1]: el ruido de fondo es sobre todo
/// grave y su nivel oscila ±8 dB; así queda estable y la voz sigue ~24 dB por encima.
pub fn nivel_db(tramo: &[u8]) -> f64 {
    let a: Vec<f64> = tramo
        .chunks_exact(2)
        .map(|b| f64::from(i16::from_le_bytes([b[0], b[1]])))
        .collect();
    if a.len() < 2 {
        return -120.0;
    }
    let e: f64 = (1..a.len())
        .map(|i| (a[i] - 0.95 * a[i - 1]).powi(2))
        .sum::<f64>()
        / (a.len() - 1) as f64;
    10.0 * (e / (32768.0_f64 * 32768.0) + 1e-12).log10()
}

/// (whisper-cli, modelo) o el error en español.
pub fn encontrar_whisper(rutas: &Rutas) -> Result<(PathBuf, PathBuf), ErrorOido> {
    let cli = std::env::var_os("VORTEX_MIKU_WHISPER")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| sistema::cual("whisper-cli"));
    let modelo = rutas.oido.join(MODELO);
    let Some(cli) = cli.filter(|c| sistema::es_ejecutable(c)) else {
        return Err(ErrorOido("No encuentro el oído (whisper-cli). Reinstala el paquete vortex-miku. Mientras tanto puedes escribirme.".into()));
    };
    if !modelo.is_file() {
        return Err(ErrorOido("Todavía no tengo el oído: se baja la primera vez que me abres, con internet. Mientras tanto puedes escribirme.".into()));
    }
    Ok((cli, modelo))
}

/// Un WAV mono de 16 bits.
pub fn envolver_wav(pcm: &[u8], tasa: u32) -> Vec<u8> {
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend(b"RIFF");
    w.extend((36 + pcm.len() as u32).to_le_bytes());
    w.extend(b"WAVEfmt ");
    w.extend(16u32.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(1u16.to_le_bytes());
    w.extend(tasa.to_le_bytes());
    w.extend((tasa * 2).to_le_bytes());
    w.extend(2u16.to_le_bytes());
    w.extend(16u16.to_le_bytes());
    w.extend(b"data");
    w.extend((pcm.len() as u32).to_le_bytes());
    w.extend(pcm);
    w
}

/// Lo que whisper devolvió, sin marcas ni frases inventadas; vacío si no dijiste nada.
pub fn limpiar(salida: &str) -> String {
    let texto = salida.split_whitespace().collect::<Vec<_>>().join(" ");
    let texto = marcas()
        .replace_all(&texto, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let limpio = texto.trim_matches(|c: char| " .,;:-¡!¿?…".contains(c));
    if limpio.chars().count() < 2 || frases_fantasma().is_match(limpio) {
        return String::new();
    }
    texto
}

/// Corre un programa con entrada binaria y tiempo límite: (código, salida estándar).
fn correr_con_entrada(
    cli: &Path,
    args: &[String],
    entrada: Vec<u8>,
    timeout: Duration,
    cwd: &Path,
) -> std::io::Result<(i32, Vec<u8>)> {
    let mut h = Command::new(cli)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut e = h.stdin.take().unwrap();
    std::thread::spawn(move || {
        let _ = e.write_all(&entrada);
    });
    let mut o = h.stdout.take().unwrap();
    let lector = std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = o.read_to_end(&mut v);
        v
    });
    let limite = Instant::now() + timeout;
    loop {
        match h.try_wait()? {
            Some(s) => return Ok((s.code().unwrap_or(-1), lector.join().unwrap_or_default())),
            None if Instant::now() > limite => {
                let _ = h.kill();
                let _ = h.wait();
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "tiempo"));
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Pasa audio crudo (16 kHz, 16 bits, mono) a texto con el programa y el modelo dados.
pub fn transcribir_con(
    cli: &Path,
    modelo: &Path,
    pcm: &[u8],
    hilos: usize,
    cwd: &Path,
    timeout: Duration,
) -> Result<String, ErrorOido> {
    // -otxt hace que imprima el texto por stdout cuando la entrada es stdin (-f -). El cwd es la carpeta en
    // memoria del usuario: si whisper llegara a escribir algo, no sería en su carpeta de trabajo.
    let args: Vec<String> = [
        "-m",
        &modelo.to_string_lossy(),
        "-l",
        "es",
        "-t",
        &hilos.to_string(),
        "-bs",
        "1",
        "-bo",
        "1",
        "-np",
        "-nt",
        "-otxt",
        "-f",
        "-",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    match correr_con_entrada(cli, &args, envolver_wav(pcm, RATE as u32), timeout, cwd) {
        Err(e) => Err(ErrorOido(format!(
            "No pude transcribir lo que dijiste ({:?}).",
            e.kind()
        ))),
        Ok((c, _)) if c != 0 => Err(ErrorOido(
            "No pude transcribir lo que dijiste (whisper falló).".into(),
        )),
        Ok((_, s)) => Ok(limpiar(&String::from_utf8_lossy(&s))),
    }
}

pub fn transcribir(rutas: &Rutas, pcm: &[u8]) -> Result<String, ErrorOido> {
    let (cli, modelo) = encontrar_whisper(rutas)?;
    transcribir_con(
        &cli,
        &modelo,
        pcm,
        crate::rutas::hilos(),
        &rutas.runtime,
        Duration::from_secs(90),
    )
}

/// Una escucha a la vez. `escuchar()` bloquea hasta tener tu frase (córrelo en un hilo).
pub struct Oido {
    pub silencio: f64,
    pub espera: f64,
    pub maximo: f64,
    pub margen_db: f64,
    /// Para las pruebas: el programa que da el PCM en lugar de `pw-record` (`VORTEX_MIKU_ENTRADA` da un WAV).
    pub fuente: Option<Vec<String>>,
    cortar: AtomicBool,
    cancelar: AtomicBool,
    pid: AtomicU32,
}

impl Oido {
    pub fn nuevo(silencio: f64, espera: f64) -> Self {
        Self {
            silencio,
            espera,
            maximo: 30.0,
            margen_db: 10.0,
            fuente: None,
            cortar: AtomicBool::new(false),
            cancelar: AtomicBool::new(false),
            pid: AtomicU32::new(0),
        }
    }

    /// «Ya acabé» (clic en el micrófono).
    pub fn cortar_ahora(&self) {
        self.cortar.store(true, Ordering::SeqCst);
    }

    /// Descarta lo grabado.
    pub fn cancelar(&self) {
        self.cancelar.store(true, Ordering::SeqCst);
        self.cerrar_grabador();
    }

    fn cerrar_grabador(&self) {
        let pid = self.pid.swap(0, Ordering::SeqCst);
        if pid != 0 {
            sistema::matar(pid, 15);
        }
    }

    fn abrir_fuente(&self) -> Result<std::process::Child, ErrorOido> {
        let cmd: Vec<String> = if let Some(f) = &self.fuente {
            f.clone()
        } else if let Some(p) = std::env::var("VORTEX_MIKU_ENTRADA")
            .ok()
            .filter(|p| !p.is_empty())
        {
            [
                "ffmpeg", "-v", "quiet", "-re", "-i", &p, "-f", "s16le", "-ar", "16000", "-ac", "1", "-",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        } else if sistema::cual("pw-record").is_some() {
            let n = ((self.maximo + self.espera + 2.0) * RATE as f64) as i64;
            [
                "pw-record",
                "--raw",
                "--rate",
                "16000",
                "--channels",
                "1",
                "--format",
                "s16",
                "--latency",
                "20ms",
                "-n",
                &n.to_string(),
                "-",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        } else {
            return Err(ErrorOido(
                "No encuentro pw-record (PipeWire) para usar el micrófono. Puedes escribirme.".into(),
            ));
        };
        let h = Command::new(&cmd[0])
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        let h = h.map_err(|_| ErrorOido("No pude abrir el micrófono. Puedes escribirme.".into()))?;
        self.pid.store(h.id(), Ordering::SeqCst);
        Ok(h)
    }

    /// Graba hasta que dejes de hablar. Devuelve el PCM de tu frase, o `None` si nadie habló o se canceló.
    /// `al_nivel(0..1)` se llama ~25 veces por segundo con el volumen del micrófono (para el indicador);
    /// `al_hablar()` una vez, cuando empieza a oír voz.
    pub fn escuchar(
        &self,
        al_nivel: &dyn Fn(f64),
        al_hablar: Option<&dyn Fn()>,
    ) -> Result<Option<Vec<u8>>, ErrorOido> {
        self.escuchar_con(self.espera, al_nivel, al_hablar)
    }

    /// Lo mismo, con otra espera (cuánto aguarda a que empieces a hablar): la primera vez y el seguimiento
    /// no esperan igual.
    pub fn escuchar_con(
        &self,
        espera: f64,
        al_nivel: &dyn Fn(f64),
        al_hablar: Option<&dyn Fn()>,
    ) -> Result<Option<Vec<u8>>, ErrorOido> {
        self.cortar.store(false, Ordering::SeqCst);
        self.cancelar.store(false, Ordering::SeqCst);
        let mut pw = self.abrir_fuente()?;
        let mut salida = pw.stdout.take().unwrap();
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        std::thread::spawn(move || {
            let mut b = [0u8; 4096];
            while let Ok(n) = salida.read(&mut b) {
                if n == 0 || tx.send(b[..n].to_vec()).is_err() {
                    break;
                }
            }
        });
        let mut buf: Vec<u8> = Vec::new();
        let mut t: usize = 0; // tramos leídos: el reloj es el propio audio
        let mut previo: VecDeque<Vec<u8>> = VecDeque::with_capacity(20); // 0,4 s anteriores al inicio de la voz
        let mut captura: Vec<Vec<u8>> = Vec::new();
        let (mut hablando, mut nadie) = (false, false);
        let (mut seguidos_voz, mut seguidos_silencio, mut sobre) = (0usize, 0usize, 0usize);
        let mut suelo: Option<f64> = None;
        let mut calib: Vec<f64> = Vec::new();
        let mut inicio_t = 0usize;
        let mut t_pared = Instant::now();
        let ms = TRAMO_MS as f64 / 1000.0;
        'fuera: while !self.cancelar.load(Ordering::SeqCst) {
            if self.cortar.load(Ordering::SeqCst) {
                break;
            }
            let d = match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(d) => d,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if t_pared.elapsed().as_secs_f64() > espera + self.maximo + 5.0 {
                        break; // el flujo se paró
                    }
                    continue;
                }
                Err(_) => break, // fin del flujo (la fuente de pruebas o un micrófono que se fue)
            };
            t_pared = Instant::now();
            buf.extend(d);
            while buf.len() >= BYTES_TRAMO {
                let tramo: Vec<u8> = buf.drain(..BYTES_TRAMO).collect();
                t += 1;
                let n = nivel_db(&tramo);
                if t % 2 == 0 {
                    al_nivel(((n + 62.0) / 40.0).clamp(0.0, 1.0));
                }
                let Some(mut s) = suelo else {
                    // Ruido de fondo = mediana de los primeros 0,3 s (acotada).
                    calib.push(n);
                    previo.push_back(tramo);
                    if previo.len() > 20 {
                        previo.pop_front();
                    }
                    if calib.len() >= 15 {
                        let mut o = calib.clone();
                        o.sort_by(|a, b| a.partial_cmp(b).unwrap());
                        suelo = Some(o[o.len() / 2].clamp(-70.0, -46.0));
                    }
                    continue;
                };
                let voz = n > s + self.margen_db;
                if !hablando {
                    if !voz && -90.0 < n && n < s + 6.0 {
                        s = 0.98 * s + 0.02 * n; // sigue el ruido lento (un cero digital no cuenta)
                    }
                    s = s.clamp(-70.0, -46.0);
                    suelo = Some(s);
                    previo.push_back(tramo);
                    if previo.len() > 20 {
                        previo.pop_front();
                    }
                    seguidos_voz = if voz { seguidos_voz + 1 } else { 0 };
                    if seguidos_voz >= 5 {
                        // 100 ms seguidos de voz
                        hablando = true;
                        inicio_t = t;
                        captura = previo.iter().cloned().collect();
                        seguidos_silencio = 0;
                        if let Some(f) = al_hablar {
                            f();
                        }
                    } else if t as f64 * ms > espera {
                        nadie = true;
                        break 'fuera;
                    }
                } else {
                    captura.push(tramo);
                    // Un tramo suelto por encima del umbral (clic de teclado, golpe) no cuenta como voz: para
                    // reiniciar la cuenta del silencio hacen falta 3 seguidos (60 ms).
                    if n > s + self.margen_db - 4.0 {
                        sobre += 1;
                        seguidos_silencio = if sobre >= 3 { 0 } else { seguidos_silencio + 1 };
                    } else {
                        sobre = 0;
                        seguidos_silencio += 1;
                    }
                    if seguidos_silencio as f64 * ms >= self.silencio
                        || (t - inicio_t) as f64 * ms >= self.maximo
                    {
                        break 'fuera;
                    }
                }
            }
        }
        self.cerrar_grabador();
        let _ = pw.kill(); // no se vuelve hasta que el grabador haya muerto de verdad
        let _ = pw.wait();
        if nadie || self.cancelar.load(Ordering::SeqCst) || !hablando {
            return Ok(None);
        }
        if seguidos_silencio > 0 {
            // Quita casi todo el silencio final (deja 0,3 s).
            let quitar = seguidos_silencio.saturating_sub(15);
            captura.truncate(captura.len().saturating_sub(quitar));
        }
        Ok(Some(captura.concat()))
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Audio de mentira: `seg` segundos de algo a esa amplitud (ruido o un tono con armónicos).
    fn tramo_audio(seg: f64, amplitud: f64, tono: bool, semilla: &mut u64) -> Vec<u8> {
        let n = (seg * RATE as f64) as usize;
        let mut v = Vec::with_capacity(n * 2);
        for i in 0..n {
            *semilla = semilla
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let ruido = ((*semilla >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0;
            let x = if tono {
                let f = i as f64 / RATE as f64;
                amplitud
                    * ((2.0 * std::f64::consts::PI * 700.0 * f).sin()
                        + 0.5 * (2.0 * std::f64::consts::PI * 1800.0 * f).sin())
                    / 1.5
            } else {
                amplitud * ruido
            };
            v.extend((x as i16).to_le_bytes());
        }
        v
    }

    fn con_fuente(dir: &Path, nombre: &str, pcm: &[u8]) -> Oido {
        let f = dir.join(nombre);
        std::fs::write(&f, pcm).unwrap();
        let mut o = Oido::nuevo(1.0, 2.0);
        o.fuente = Some(vec!["cat".into(), f.to_string_lossy().into_owned()]);
        o
    }

    #[test]
    fn los_niveles() {
        let mut s = 1;
        let silencio = tramo_audio(0.02, 0.0, false, &mut s);
        assert!(nivel_db(&silencio) < -90.0);
        let ruido = tramo_audio(0.02, 30.0, false, &mut s);
        let voz = tramo_audio(0.02, 8000.0, true, &mut s);
        assert!(nivel_db(&voz) > nivel_db(&ruido) + 30.0);
        // Un zumbido grave de la misma fuerza cuenta mucho menos que la voz (el preénfasis).
        let zumbido: Vec<u8> = (0..TRAMO)
            .flat_map(|i| {
                (((i as f64 / 16000.0 * 2.0 * std::f64::consts::PI * 60.0).sin() * 8000.0) as i16)
                    .to_le_bytes()
            })
            .collect();
        assert!(nivel_db(&zumbido) < nivel_db(&voz) - 8.0);
    }

    #[test]
    fn corta_cuando_dejas_de_hablar() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = 7;
        let mut pcm = tramo_audio(0.6, 30.0, false, &mut s); // el cuarto en silencio
        pcm.extend(tramo_audio(1.0, 8000.0, true, &mut s)); // hablas un segundo
        pcm.extend(tramo_audio(3.0, 30.0, false, &mut s)); // y callas
        let o = con_fuente(dir.path(), "habla.raw", &pcm);
        let empezo = std::sync::atomic::AtomicBool::new(false);
        let hablo = || empezo.store(true, Ordering::SeqCst);
        let r = o
            .escuchar(&|_| {}, Some(&hablo))
            .unwrap()
            .expect("tenía que oírte");
        assert!(empezo.load(Ordering::SeqCst));
        let seg = r.len() as f64 / 2.0 / RATE as f64;
        // 0,4 s antes de la voz + 1 s de voz + ~0,3 s de cola (se corta tras 1 s de silencio y deja 0,3 s).
        assert!((1.2..2.2).contains(&seg), "duró {seg} s");
    }

    #[test]
    fn nadie_habla_y_se_rinde() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = 3;
        let pcm = tramo_audio(4.0, 30.0, false, &mut s);
        let o = con_fuente(dir.path(), "nada.raw", &pcm);
        assert_eq!(o.escuchar(&|_| {}, None).unwrap(), None);
    }

    #[test]
    fn un_golpe_suelto_no_es_voz_ni_la_alarga() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = 11;
        let mut pcm = tramo_audio(0.6, 30.0, false, &mut s);
        pcm.extend(tramo_audio(0.02, 8000.0, true, &mut s)); // un clic de teclado
        pcm.extend(tramo_audio(3.0, 30.0, false, &mut s));
        let o = con_fuente(dir.path(), "clic.raw", &pcm);
        assert_eq!(
            o.escuchar(&|_| {}, None).unwrap(),
            None,
            "100 ms seguidos hacen falta"
        );
    }

    #[test]
    fn sin_micro_dice_que_pasa() {
        let mut o = Oido::nuevo(1.0, 1.0);
        o.fuente = Some(vec!["/no/existe/pw-record".into()]);
        assert_eq!(
            o.escuchar(&|_| {}, None).unwrap_err().0,
            "No pude abrir el micrófono. Puedes escribirme."
        );
    }

    #[test]
    fn limpia_lo_que_whisper_inventa() {
        assert_eq!(limpiar(" Hola   Miku \n"), "Hola Miku");
        assert_eq!(limpiar("[Música]"), "");
        assert_eq!(limpiar("(aplausos) ¡Hola!"), "¡Hola!");
        assert_eq!(limpiar("Gracias por ver el video."), "");
        assert_eq!(limpiar("Suscríbete al canal"), "");
        assert_eq!(limpiar("."), "");
        assert_eq!(limpiar("Subtítulos realizados por la comunidad de Amara.org"), "");
    }

    #[test]
    fn transcribe_con_un_whisper_falso() {
        let dir = tempfile::tempdir().unwrap();
        let cli = dir.path().join("whisper-cli");
        // Comprueba que llega un WAV por la entrada y que se pide español; contesta con texto.
        std::fs::write(&cli, "#!/bin/sh\nhead -c 4 > \"$(dirname \"$0\")/cabecera\"\ncase \"$*\" in *'-l es'*) echo ' Abre Firefox, por favor.';; *) echo mal;; esac\n").unwrap();
        crate::sistema::permisos(&cli, 0o755).unwrap();
        let modelo = dir.path().join("modelo.bin");
        std::fs::write(&modelo, b"x").unwrap();
        let t = transcribir_con(&cli, &modelo, &[0u8; 640], 2, dir.path(), Duration::from_secs(5)).unwrap();
        assert_eq!(t, "Abre Firefox, por favor.");
        assert_eq!(std::fs::read(dir.path().join("cabecera")).unwrap(), b"RIFF");
        let malo = dir.path().join("malo");
        std::fs::write(&malo, "#!/bin/sh\nexit 3\n").unwrap();
        crate::sistema::permisos(&malo, 0o755).unwrap();
        assert!(
            transcribir_con(&malo, &modelo, &[0u8; 64], 1, dir.path(), Duration::from_secs(5))
                .unwrap_err()
                .0
                .contains("whisper falló")
        );
    }

    #[test]
    fn el_wav_tiene_su_cabecera() {
        let w = envolver_wav(&[1, 2, 3, 4], 16000);
        assert_eq!(w.len(), 48);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(w[24..28].try_into().unwrap()), 16000);
        assert_eq!(u32::from_le_bytes(w[40..44].try_into().unwrap()), 4);
    }
}
