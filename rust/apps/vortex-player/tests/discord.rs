//! `test_discord.py`: la actividad y la conexión contra un Discord de mentira (un socket en una carpeta
//! temporal). No toca el Discord de quien las corre.

use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_player::discord::{self as pd, Opciones, Presencia, Sonando};

const YT: &str = "/música/Bohemia [dQw4w9WgXcQ].opus";

fn esperar(mut cond: impl FnMut() -> bool, ms: u64) -> bool {
    let fin = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < fin {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    cond()
}

#[test]
fn ida_y_vuelta_partido_y_bytes() {
    let mut datos = pd::marco(1, &json!({"a": "ñandú"}));
    datos.extend(pd::marco(3, &json!({"b": 2})));
    let (m, r) = pd::leer_marcos(&datos[..datos.len() - 2]);
    assert_eq!(m, [(1, json!({"a": "ñandú"}))]);
    let mut r = r;
    r.extend_from_slice(&datos[datos.len() - 2..]);
    assert_eq!(pd::leer_marcos(&r).0, [(3, json!({"b": 2}))]);
    assert_eq!(pd::leer_marcos(&pd::marco(1, &json!({"t": "日本語"}))).0[0].1["t"], "日本語");
}

#[test]
fn actividad_de_youtube_y_propia() {
    let c = json!({"titulo": "Bohemia", "artista": "Queen", "album": "Opera", "ruta": YT});
    let a = pd::actividad(&c, 10.0, 100.0, 1.0, 1000.0);
    assert_eq!(a["type"], 2);
    assert_eq!((a["details"].as_str(), a["state"].as_str()), (Some("Bohemia"), Some("Queen")));
    assert_eq!(
        a["assets"],
        json!({"large_image": "https://i.ytimg.com/vi/dQw4w9WgXcQ/mqdefault.jpg", "large_text": "Opera"})
    );
    assert_eq!(a["timestamps"], json!({"start": 990000, "end": 1090000}));
    assert_eq!(a["buttons"][0]["url"], "https://music.youtube.com/watch?v=dQw4w9WgXcQ");
    assert!(a["buttons"][0]["label"].as_str().unwrap().chars().count() <= 32);

    let a = pd::actividad(&json!({"ruta": "/música/mi canción.flac"}), 0.0, 0.0, 1.0, 0.0);
    assert_eq!(a["details"], "mi canción");
    assert_eq!(a["assets"]["large_image"], pd::LOGO);
    for falta in ["state", "buttons", "timestamps"] {
        assert!(a.get(falta).is_none(), "{falta}");
    }
    let a = pd::actividad(&json!({"titulo": "x".repeat(300), "artista": "U", "ruta": "/a.flac"}), 0.0, 0.0, 1.0, 0.0);
    assert_eq!(a["details"].as_str().unwrap().len(), 128);
    assert_eq!(a["state"].as_str().unwrap().chars().count(), 2);
    // practicando no lleva tiempo
    assert!(pd::actividad(&json!({"titulo": "aa", "ruta": YT}), 0.0, 100.0, 0.75, 0.0).get("timestamps").is_none());
}

#[test]
fn misma() {
    let c = json!({"titulo": "aa", "ruta": YT});
    let a = pd::actividad(&c, 0.0, 100.0, 1.0, 1000.0);
    assert!(pd::misma(&pd::actividad(&c, 1.0, 100.0, 1.0, 1001.0), Some(&a))); // el tiempo solo avanzó
    assert!(!pd::misma(&pd::actividad(&c, 50.0, 100.0, 1.0, 1001.0), Some(&a))); // un salto
    assert!(!pd::misma(&pd::actividad(&json!({"titulo": "bb", "ruta": YT}), 0.0, 100.0, 1.0, 1000.0), Some(&a)));
    assert!(!pd::misma(&pd::actividad(&c, 0.0, 100.0, 0.5, 1000.0), Some(&a)));
    assert!(!pd::misma(&a, None));
}

// --- la conexión ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Visto {
    marcos: Vec<(u32, Value)>,
    conexiones: Vec<UnixStream>,
}

/// Contesta el saludo con READY y guarda lo que le llega.
struct DiscordFalso {
    visto: Arc<Mutex<Visto>>,
    saludar: Arc<Mutex<bool>>,
    ruta: PathBuf,
}

impl DiscordFalso {
    fn nuevo(ruta: &Path) -> Self {
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        let _ = std::fs::remove_file(ruta);
        let srv = UnixListener::bind(ruta).unwrap();
        let visto: Arc<Mutex<Visto>> = Arc::default();
        let saludar = Arc::new(Mutex::new(true));
        let (v, sal) = (visto.clone(), saludar.clone());
        std::thread::spawn(move || {
            for c in srv.incoming() {
                let Ok(mut c) = c else { break };
                v.lock().unwrap().conexiones.push(c.try_clone().unwrap());
                let (v, sal) = (v.clone(), sal.clone());
                std::thread::spawn(move || {
                    let (mut resto, mut b) = (Vec::new(), [0u8; 4096]);
                    while let Ok(n) = c.read(&mut b) {
                        if n == 0 {
                            break;
                        }
                        resto.extend_from_slice(&b[..n]);
                        let (m, sobra) = pd::leer_marcos(&resto);
                        resto = sobra;
                        for (op, d) in m {
                            v.lock().unwrap().marcos.push((op, d));
                            if op == pd::SALUDO && *sal.lock().unwrap() {
                                let _ = c.write_all(&pd::marco(
                                    pd::MARCO,
                                    &json!({"cmd": "DISPATCH", "evt": "READY", "data": {"v": 1}}),
                                ));
                            }
                        }
                    }
                });
            }
        });
        Self { visto, saludar, ruta: ruta.to_path_buf() }
    }

    fn mandar(&self, op: u32, d: &Value) {
        let mut v = self.visto.lock().unwrap();
        let c = v.conexiones.last_mut().unwrap();
        c.write_all(&pd::marco(op, d)).unwrap();
    }

    fn marcos(&self) -> Vec<(u32, Value)> {
        self.visto.lock().unwrap().marcos.clone()
    }

    fn conexiones(&self) -> usize {
        self.visto.lock().unwrap().conexiones.len()
    }

    fn titulos(&self) -> Vec<Option<String>> {
        self.marcos()
            .into_iter()
            .filter(|(op, d)| *op == pd::MARCO && d["cmd"] == "SET_ACTIVITY")
            .map(|(_, d)| d["args"].get("activity").map(|a| a["details"].as_str().unwrap().to_string()))
            .collect()
    }

    /// Discord se cierra.
    fn cerrar(&self) {
        let _ = std::fs::remove_file(&self.ruta);
        for c in self.visto.lock().unwrap().conexiones.drain(..) {
            let _ = c.shutdown(std::net::Shutdown::Both);
        }
    }
}

struct Prueba {
    t: tempfile::TempDir,
    p: Presencia,
    s: Sonando,
}

fn t(v: &[Option<&str>]) -> Vec<Option<String>> {
    v.iter().map(|x| x.map(String::from)).collect()
}

impl Prueba {
    fn nueva(cliente: &str, entre_envios: f64) -> Self {
        let t = tempfile::Builder::new().prefix("vp-dc-").tempdir_in("/tmp").unwrap();
        let raiz = t.path().to_path_buf();
        let rutas = move || {
            ["", "app/com.discordapp.Discord"]
                .iter()
                .flat_map(|c| (0..10).map(move |i| (c, i)))
                .map(|(c, i)| raiz.join(c).join(format!("discord-ipc-{i}")))
                .filter(|r| r.exists())
                .map(|r| r.to_string_lossy().into_owned())
                .collect()
        };
        let p = Presencia::arrancar(Opciones {
            cliente: cliente.into(),
            entre_envios: Duration::from_secs_f64(entre_envios),
            espera: Duration::from_millis(1),
            pulso: Duration::from_millis(30),
            rutas: Arc::new(rutas),
        });
        Self { t, p, s: Sonando { duracion: 200.0, velocidad: 1.0, activa: true, ..Sonando::default() } }
    }

    fn discord(&self, sub: &str) -> DiscordFalso {
        DiscordFalso::nuevo(&self.t.path().join(sub).join("discord-ipc-0"))
    }

    fn tocar(&mut self, titulo: &str) {
        self.s.cancion = json!({"titulo": titulo, "artista": "Artista", "album": "Disco", "ruta": YT});
        self.s.posicion = 0.0;
        self.p.poner(self.s.clone());
        self.p.actualizar();
    }

    fn pausar(&mut self, si: bool) {
        self.s.pausado = si;
        self.p.poner(self.s.clone());
        self.p.actualizar();
    }
}

#[test]
fn saluda_manda_y_no_repite() {
    let mut p = Prueba::nueva("123", 0.0);
    let f = p.discord("");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000), "{:?}", f.marcos());
    let m = f.marcos();
    assert_eq!(m[0], (pd::SALUDO, json!({"v": 1, "client_id": "123"})));
    assert_eq!(m[1].1["args"]["pid"], std::process::id());
    esperar(|| false, 200); // varios pulsos
    assert_eq!(f.titulos(), t(&[Some("Uno")]));
}

#[test]
fn pausa_cambio_y_salto() {
    let mut p = Prueba::nueva("123", 0.0);
    let f = p.discord("");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000));
    p.pausar(true);
    assert!(esperar(|| f.titulos() == t(&[Some("Uno"), None]), 3000), "{:?}", f.titulos());
    p.pausar(false);
    assert!(esperar(|| f.titulos() == t(&[Some("Uno"), None, Some("Uno")]), 3000), "{:?}", f.titulos());
    p.tocar("Dos");
    assert!(esperar(|| f.titulos().last() == Some(&Some("Dos".into())), 3000));
    let n = f.titulos().len();
    p.s.posicion = 150.0; // un salto no avisa: lo nota el pulso
    p.p.poner(p.s.clone());
    assert!(esperar(|| f.titulos().len() == n + 1, 3000), "{:?}", f.titulos());
}

#[test]
fn el_interruptor_y_sin_id() {
    let mut p = Prueba::nueva("123", 0.0);
    let f = p.discord("");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000));
    p.s.activa = false;
    p.tocar("Dos");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno"), None]), 3000));
    esperar(|| false, 100);
    assert_eq!(f.titulos(), t(&[Some("Uno"), None]));
    p.s.activa = true;
    p.p.poner(p.s.clone());
    p.p.actualizar();
    assert!(esperar(|| f.titulos().last() == Some(&Some("Dos".into())), 3000));

    // apagado, o sin Application ID, ni se conecta
    for (cliente, activa) in [("123", false), ("", true)] {
        let mut q = Prueba::nueva(cliente, 0.0);
        let g = q.discord("");
        q.s.activa = activa;
        q.tocar("Uno");
        esperar(|| false, 100);
        assert_eq!(g.conexiones(), 0);
    }
}

#[test]
fn discord_abre_despues_se_cierra_y_vuelve() {
    let mut p = Prueba::nueva("123", 0.0);
    p.tocar("Uno");
    esperar(|| false, 80);
    let f = p.discord("");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000), "{:?}", f.marcos());
    f.cerrar();
    esperar(|| false, 80);
    let g = p.discord("");
    assert!(esperar(|| g.titulos() == t(&[Some("Uno")]), 3000), "{:?}", g.marcos());
}

#[test]
fn socket_muerto_y_flatpak() {
    let mut p = Prueba::nueva("123", 0.0);
    std::fs::write(p.t.path().join("discord-ipc-0"), "").unwrap(); // quedó de un Discord que tronó
    let f = p.discord("app/com.discordapp.Discord");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000), "{:?}", f.marcos());
}

#[test]
fn ping_error_y_cierre() {
    let mut p = Prueba::nueva("123", 0.0);
    let f = p.discord("");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000));
    f.mandar(pd::PING, &json!({"n": 7}));
    assert!(esperar(|| f.marcos().contains(&(pd::PONG, json!({"n": 7}))), 3000));
    f.mandar(pd::MARCO, &json!({"evt": "ERROR", "data": {"code": 4000, "message": "de prueba"}}));
    esperar(|| false, 60);
    assert_eq!(f.conexiones(), 1); // un error no corta

    // un Application ID malo: Discord no saluda y cierra; se vuelve a intentar con el pulso
    let mut q = Prueba::nueva("malo", 0.0);
    let g = q.discord("");
    *g.saludar.lock().unwrap() = false;
    q.tocar("Uno");
    assert!(esperar(|| !g.marcos().is_empty(), 3000));
    g.mandar(pd::CIERRE, &json!({"code": 4000, "message": "Invalid Client ID"}));
    assert!(esperar(|| g.conexiones() > 1, 3000));
    assert!(g.titulos().is_empty());
}

#[test]
fn respeta_el_limite_de_discord() {
    let mut p = Prueba::nueva("123", 0.3);
    let f = p.discord("");
    p.tocar("Uno");
    assert!(esperar(|| f.titulos() == t(&[Some("Uno")]), 3000));
    let inicio = Instant::now();
    p.tocar("Dos");
    esperar(|| false, 30);
    p.tocar("Tres");
    assert!(esperar(|| f.titulos().last() == Some(&Some("Tres".into())), 3000), "{:?}", f.titulos());
    assert!(inicio.elapsed() >= Duration::from_millis(200));
    assert!(!f.titulos().contains(&Some("Dos".into()))); // brincada antes de mandarla
}
