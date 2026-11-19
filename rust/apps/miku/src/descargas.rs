//! Los modelos de la voz y del oído de Miku: no vienen en la ISO, se bajan una sola vez al abrirla.
//!
//!   voz   Supertonic 3 (Supertone, ONNX) y la voz F1: ~380 MB  → ~/.local/share/vortex/miku/voz/
//!   oido  Whisper small cuantizado (ggml, para whisper.cpp): ~181 MB → ~/.local/share/vortex/miku/oido/
//!
//! Cada archivo tiene URL fija (a un commit, no a «main») y su sha256: lo que llega distinto de lo esperado
//! no se instala nunca. Se baja a un «.parte» y, si se corta la red, se reanuda donde se quedó. Un archivo
//! presente en su sitio ya fue comprobado al bajarlo, así que mirar si falta algo es instantáneo.
//!
//! La lista (`manifiesto.json`) es la misma de `descargas.py`. Para pruebas, `VORTEX_MIKU_MANIFIESTO=ruta.json`
//! la cambia (mismo formato); cada archivo sigue comprobándose con su sha256, así que no se abre ningún hueco.
//! La red va por `curl` (reanuda con `-C -`; no hay crate TLS en el proyecto).

use crate::rutas::Rutas;
use crate::sha256;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Lo que se le dice a la persona; son 563 MiB (590 MB) en total.
pub const TAMANO_TOTAL_TEXTO: &str = "~560 MB";
const MANIFIESTO: &str = include_str!("../manifiesto.json");

#[derive(Debug, Clone, PartialEq)]
pub struct ErrorDescarga(pub String);

impl std::fmt::Display for ErrorDescarga {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Archivo {
    pub ruta: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone)]
pub struct Paquete {
    pub id: String,
    pub nombre: String,
    pub archivos: Vec<Archivo>,
}

fn leer_paquetes(v: &Value) -> Vec<Paquete> {
    let raiz = v.get("paquetes").unwrap_or(v);
    let Some(m) = raiz.as_object() else { return vec![] };
    m.iter()
        .filter_map(|(id, p)| {
            let archivos = p["archivos"]
                .as_array()?
                .iter()
                .filter_map(|a| {
                    Some(Archivo {
                        ruta: a["ruta"].as_str()?.into(),
                        url: a["url"].as_str()?.into(),
                        bytes: a["bytes"].as_u64()?,
                        sha256: a["sha256"].as_str()?.into(),
                    })
                })
                .collect();
            Some(Paquete {
                id: id.clone(),
                nombre: p["nombre"].as_str().unwrap_or(id).into(),
                archivos,
            })
        })
        .collect()
}

/// La lista de paquetes: la de fábrica o la de `VORTEX_MIKU_MANIFIESTO`. El orden es el de bajada: primero
/// el oído (más chico) y luego la voz.
pub fn paquetes() -> Vec<Paquete> {
    if let Some(ruta) = std::env::var_os("VORTEX_MIKU_MANIFIESTO").filter(|v| !v.is_empty())
        && let Some(v) = std::fs::read_to_string(ruta)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
    {
        let p = leer_paquetes(&v);
        if !p.is_empty() {
            return p;
        }
    }
    let mut p = leer_paquetes(&serde_json::from_str::<Value>(MANIFIESTO).unwrap_or_default());
    p.sort_by_key(|x| usize::from(x.id != "oido"));
    p
}

pub fn carpeta(rutas: &Rutas, paquete: &str) -> PathBuf {
    rutas.datos.join(paquete)
}

/// Los archivos del paquete que aún no están en su sitio (o están a medias).
pub fn archivos_faltantes(rutas: &Rutas, paquete: &Paquete) -> Vec<Archivo> {
    let base = carpeta(rutas, &paquete.id);
    paquete
        .archivos
        .iter()
        .filter(|a| {
            std::fs::metadata(base.join(&a.ruta))
                .map(|m| !m.is_file() || m.len() != a.bytes)
                .unwrap_or(true)
        })
        .cloned()
        .collect()
}

pub fn paquete_listo(rutas: &Rutas, id: &str) -> bool {
    paquetes()
        .iter()
        .find(|p| p.id == id)
        .map(|p| archivos_faltantes(rutas, p).is_empty())
        .unwrap_or(true)
}

/// Los paquetes (en orden) a los que les falta algo.
pub fn pendientes(rutas: &Rutas) -> Vec<String> {
    paquetes()
        .into_iter()
        .filter(|p| !archivos_faltantes(rutas, p).is_empty())
        .map(|p| p.id)
        .collect()
}

pub fn bytes_pendientes(rutas: &Rutas, nombres: &[String]) -> u64 {
    paquetes()
        .iter()
        .filter(|p| nombres.contains(&p.id))
        .flat_map(|p| archivos_faltantes(rutas, p))
        .map(|a| a.bytes)
        .sum()
}

/// Tamaño para mostrar (en MiB, que es como lo cuenta el disco: la descarga entera son ~563).
pub fn megas(n: u64) -> String {
    format!("{:.0} MB", n as f64 / f64::from(1 << 20))
}

/// Solo se baja de https (o de esta misma máquina, para las pruebas).
pub fn url_permitida(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    ["http://127.0.0.1", "http://localhost", "http://[::1]"]
        .iter()
        .any(|p| {
            url.strip_prefix(p)
                .is_some_and(|r| r.is_empty() || r.starts_with([':', '/']))
        })
}

/// Por qué falló curl, en pocas palabras y en español (nunca el texto crudo del sistema).
fn resumen_curl(codigo: i32, http: &str) -> String {
    match codigo {
        6 => "no encuentro el servidor".into(),
        28 => "el servidor tardó demasiado".into(),
        7 => "el servidor rechazó la conexión".into(),
        22 => format!("el servidor respondió {http}"),
        35 | 52 | 55 | 56 | 60 => "se cortó la conexión".into(),
        _ => "sin conexión".into(),
    }
}

pub type Progreso = Arc<dyn Fn(&str, f64) + Send + Sync>;

/// Baja los paquetes que falten. `progreso(texto, fracción)` se llama unas 4 veces por segundo y
/// `al_paquete(id)` cada vez que uno termina. `cancelar` corta la descarga en cuanto puede; lo ya bajado
/// queda en el «.parte» para reanudar la próxima vez.
pub struct Descarga {
    pub rutas: Rutas,
    pub nombres: Vec<String>,
    pub progreso: Progreso,
    pub al_paquete: Arc<dyn Fn(&str) + Send + Sync>,
    pub cancelar: Arc<AtomicBool>,
    pub total: u64,
    pub hecho: u64,
    /// Pruebas: la lista de paquetes en lugar de la de fábrica (o la de `VORTEX_MIKU_MANIFIESTO`).
    pub lista: Option<Vec<Paquete>>,
    /// Cuánto se espera entre un intento y otro (las pruebas lo bajan).
    pub pausa: Duration,
    ultimo: Instant,
}

impl Descarga {
    pub fn nueva(
        rutas: &Rutas,
        nombres: Option<Vec<String>>,
        progreso: Progreso,
        cancelar: Arc<AtomicBool>,
        al_paquete: Arc<dyn Fn(&str) + Send + Sync>,
    ) -> Self {
        let nombres = nombres.unwrap_or_else(|| pendientes(rutas));
        let total = bytes_pendientes(rutas, &nombres);
        Self {
            rutas: rutas.clone(),
            nombres,
            progreso,
            al_paquete,
            cancelar,
            total,
            hecho: 0,
            lista: None,
            pausa: Duration::from_secs(2),
            ultimo: Instant::now() - Duration::from_secs(1),
        }
    }

    /// Baja todo lo que falte o devuelve el error con un mensaje para mostrar.
    pub fn correr(&mut self) -> Result<(), ErrorDescarga> {
        self.hay_espacio()?;
        let todos = self.lista.clone().unwrap_or_else(paquetes);
        for id in self.nombres.clone() {
            let Some(p) = todos.iter().find(|p| p.id == id) else {
                continue;
            };
            let base = carpeta(&self.rutas, &p.id);
            for a in archivos_faltantes(&self.rutas, p) {
                self.bajar(&a, &base.join(&a.ruta), &p.nombre)?;
            }
            (self.al_paquete)(&p.id);
        }
        self.avisar(true, "");
        Ok(())
    }

    fn hay_espacio(&self) -> Result<(), ErrorDescarga> {
        let _ = std::fs::create_dir_all(&self.rutas.datos);
        let libre = espacio_libre(&self.rutas.datos);
        let hace_falta = self.total + (200 << 20);
        if let Some(libre) = libre
            && libre < hace_falta
        {
            return Err(ErrorDescarga(format!(
                "No hay espacio para bajar la voz de Miku: hacen falta {} libres y hay {}.",
                megas(hace_falta),
                megas(libre)
            )));
        }
        Ok(())
    }

    fn avisar(&mut self, forzar: bool, que: &str) {
        if !forzar && self.ultimo.elapsed() < Duration::from_millis(250) {
            return;
        }
        self.ultimo = Instant::now();
        let fraccion = if self.total > 0 {
            self.hecho as f64 / self.total as f64
        } else {
            1.0
        };
        (self.progreso)(
            format!("{que} {} de {}", megas(self.hecho), megas(self.total)).trim(),
            fraccion.min(1.0),
        );
    }

    pub fn bajar(&mut self, a: &Archivo, destino: &Path, que: &str) -> Result<(), ErrorDescarga> {
        if !url_permitida(&a.url) {
            return Err(ErrorDescarga(
                "La dirección de descarga no es segura; no se baja.".into(),
            ));
        }
        if let Some(d) = destino.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let parte = PathBuf::from(format!("{}.parte", destino.display()));
        let mut intentos = 0;
        loop {
            match self.intento(a, destino, &parte, que) {
                Ok(()) => return Ok(()),
                Err(Fallo::Fatal(e)) => return Err(e),
                Err(Fallo::Red(porque)) => {
                    intentos += 1;
                    if self.cancelar.load(Ordering::SeqCst) {
                        return Err(ErrorDescarga("Se canceló la descarga.".into()));
                    }
                    if intentos >= 4 {
                        return Err(ErrorDescarga(format!(
                            "No pude bajar {que} de Miku ({porque}). ¿Hay internet? Mientras tanto te respondo solo por texto; vuelvo a intentarlo la próxima vez."
                        )));
                    }
                    std::thread::sleep(self.pausa.min(self.pausa * intentos));
                }
            }
        }
    }

    fn intento(&mut self, a: &Archivo, destino: &Path, parte: &Path, que: &str) -> Result<(), Fallo> {
        let tamano = std::fs::metadata(parte).map(|m| m.len()).unwrap_or(0);
        if tamano > a.bytes {
            let _ = std::fs::remove_file(parte);
        }
        let base = self.hecho;
        let tamano = std::fs::metadata(parte).map(|m| m.len()).unwrap_or(0);
        if tamano < a.bytes {
            let mut c = Command::new("curl");
            c.args([
                "-sS",
                "-L",
                "--fail",
                "--proto",
                "=https,http",
                "--proto-redir",
                "=https",
                "--connect-timeout",
                "25",
                "--speed-limit",
                "1",
                "--speed-time",
                "25",
                "-A",
                "vortex-miku (Vortexine's OS)",
                "-C",
                "-",
                "-o",
            ])
            .arg(parte)
            .args(["-w", "%{http_code}"])
            .arg(&a.url)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
            let mut h = c.spawn().map_err(|_| Fallo::Red("sin conexión".into()))?;
            let mut salida = h.stdout.take().unwrap();
            let lector = std::thread::spawn(move || {
                let mut s = String::new();
                let _ = std::io::Read::read_to_string(&mut salida, &mut s);
                s
            });
            let estado = loop {
                if self.cancelar.load(Ordering::SeqCst) {
                    let _ = h.kill();
                    let _ = h.wait();
                    return Err(Fallo::Fatal(ErrorDescarga("Se canceló la descarga.".into())));
                }
                match h.try_wait() {
                    Ok(Some(s)) => break s,
                    Ok(None) => {
                        self.hecho = base
                            + std::fs::metadata(parte)
                                .map(|m| m.len())
                                .unwrap_or(0)
                                .saturating_sub(tamano);
                        self.avisar(false, &format!("Bajando {que}"));
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => return Err(Fallo::Red("sin conexión".into())),
                }
            };
            let http = lector.join().unwrap_or_default();
            if !estado.success() {
                let codigo = estado.code().unwrap_or(-1);
                self.hecho = base;
                if codigo == 33 || http.trim() == "416" {
                    // El servidor no entiende Range, o el .parte no sirve: se empieza de nuevo.
                    let _ = std::fs::remove_file(parte);
                    return Err(Fallo::Red("sin conexión".into()));
                }
                if codigo == 22 {
                    let n: u32 = http.trim().parse().unwrap_or(0);
                    if (400..500).contains(&n) && ![408, 429].contains(&n) {
                        return Err(Fallo::Fatal(ErrorDescarga(format!(
                            "No encuentro {que} de Miku en el servidor (error {n}). Mientras tanto te respondo solo por texto."
                        ))));
                    }
                }
                return Err(Fallo::Red(resumen_curl(codigo, http.trim())));
            }
        }
        let tam = std::fs::metadata(parte).map(|m| m.len()).unwrap_or(0);
        if tam > a.bytes {
            let _ = std::fs::remove_file(parte);
            return Err(Fallo::Fatal(ErrorDescarga(format!(
                "«{}» llegó más grande de lo esperado; no se instala.",
                a.ruta
            ))));
        }
        if tam != a.bytes {
            self.hecho = base + tam;
            return Err(Fallo::Red(format!(
                "llegó incompleto ({tam} de {} bytes)",
                a.bytes
            )));
        }
        let suma = sha256::de_archivo(parte).map_err(|_| Fallo::Red("sin conexión".into()))?;
        if suma != a.sha256 {
            let _ = std::fs::remove_file(parte);
            self.hecho = base;
            return Err(Fallo::Fatal(ErrorDescarga(format!(
                "«{}» llegó distinto de lo esperado (sha256); no se instala. Vuelve a intentarlo más tarde.",
                a.ruta
            ))));
        }
        std::fs::rename(parte, destino).map_err(|_| Fallo::Red("sin conexión".into()))?;
        self.hecho = base + tam;
        Ok(())
    }
}

enum Fallo {
    /// Se reintenta (con el porqué en español).
    Red(String),
    /// No tiene caso reintentar.
    Fatal(ErrorDescarga),
}

/// Bytes libres en el disco de esa carpeta (`df`; sin él no se comprueba).
fn espacio_libre(carpeta: &Path) -> Option<u64> {
    let r = Command::new("df")
        .args(["-B1", "--output=avail"])
        .arg(carpeta)
        .output()
        .ok()?;
    String::from_utf8_lossy(&r.stdout)
        .lines()
        .nth(1)?
        .trim()
        .parse()
        .ok()
}

/// Un archivo suelto (p. ej. el estilo de una voz opcional) a `dest/ruta`, con su sha256.
pub fn bajar_archivo(
    rutas: &Rutas,
    dest: &Path,
    a: &Archivo,
    progreso: Option<Progreso>,
) -> Result<(), ErrorDescarga> {
    let destino = dest.join(&a.ruta);
    if std::fs::metadata(&destino)
        .map(|m| m.is_file() && m.len() == a.bytes)
        .unwrap_or(false)
    {
        return Ok(());
    }
    let mut d = Descarga::nueva(
        rutas,
        Some(vec![]),
        progreso.unwrap_or_else(|| Arc::new(|_, _| {})),
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_| {}),
    );
    d.total = a.bytes;
    d.bajar(a, &destino, "la voz")
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn el_manifiesto_de_fabrica() {
        let p = paquetes();
        assert_eq!(p[0].id, "oido", "primero el oído, que es más chico");
        assert_eq!(p[1].id, "voz");
        assert!(p[0].archivos[0].url.starts_with("https://") && p[0].archivos[0].sha256.len() == 64);
        assert!(p[1].archivos.len() > 3);
    }

    #[test]
    fn solo_https_o_esta_maquina() {
        assert!(url_permitida("https://huggingface.co/x"));
        assert!(url_permitida("http://127.0.0.1:8080/a"));
        assert!(url_permitida("http://localhost/a"));
        assert!(!url_permitida("http://huggingface.co/x"));
        assert!(!url_permitida("http://127.0.0.1.malo.com/a"));
        assert!(!url_permitida("ftp://127.0.0.1/a"));
    }

    #[test]
    fn lo_que_falta_y_lo_que_ya_esta() {
        let d = tempfile::tempdir().unwrap();
        let r = Rutas::en(d.path());
        let p = paquetes();
        assert_eq!(pendientes(&r), vec!["oido".to_string(), "voz".to_string()]);
        let a = &p[0].archivos[0];
        std::fs::create_dir_all(r.datos.join("oido")).unwrap();
        // Un archivo del tamaño justo cuenta como bajado (ya se comprobó al bajarlo).
        let f = std::fs::File::create(r.datos.join("oido").join(&a.ruta)).unwrap();
        f.set_len(a.bytes).unwrap();
        assert!(paquete_listo(&r, "oido"));
        assert_eq!(pendientes(&r), vec!["voz".to_string()]);
        assert_eq!(megas(1 << 20), "1 MB");
        assert_eq!(megas(190_085_487), "181 MB");
    }
}
