//! La descarga de los modelos contra un servidor local FALSO: no se baja nada de internet. Con archivos
//! pequeños y su sha256 comprueba: la descarga completa con su progreso, reanudar tras cortarse (Range), que
//! un archivo distinto no se instala ni deja restos, el 404, sin red, una URL no segura y cancelar.

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_miku::descargas::{Archivo, Descarga, Paquete};
use vortex_miku::rutas::Rutas;
use vortex_miku::sha256;

#[derive(Default)]
struct Estado {
    cortar_una_vez: HashSet<String>,
    corromper: HashSet<String>,
    rangos: Vec<String>,
    peticiones: Vec<String>,
}

struct Servidor {
    url: String,
    estado: Arc<Mutex<Estado>>,
}

fn servidor(archivos: Vec<(String, Vec<u8>)>) -> Servidor {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    let estado = Arc::new(Mutex::new(Estado::default()));
    let (e2, archivos) = (estado.clone(), Arc::new(archivos));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (e, a) = (e2.clone(), archivos.clone());
            std::thread::spawn(move || atender(s, &e, &a));
        }
    });
    Servidor { url, estado }
}

fn atender(mut s: TcpStream, estado: &Mutex<Estado>, archivos: &[(String, Vec<u8>)]) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut primera = String::new();
    if r.read_line(&mut primera).unwrap_or(0) == 0 {
        return;
    }
    let ruta = primera.split_whitespace().nth(1).unwrap_or("").trim_start_matches('/').to_string();
    let mut rango = None;
    loop {
        let mut l = String::new();
        if r.read_line(&mut l).unwrap_or(0) == 0 || l.trim().is_empty() {
            break;
        }
        if let Some(v) = l.to_lowercase().strip_prefix("range: bytes=") {
            rango = v.trim().trim_end_matches('-').parse::<usize>().ok();
        }
    }
    let mut e = estado.lock().unwrap();
    e.peticiones.push(ruta.clone());
    let Some((_, datos)) = archivos.iter().find(|(n, _)| *n == ruta) else {
        let _ = s.write_all(b"HTTP/1.1 404 No\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        return;
    };
    let mut cuerpo = datos.clone();
    if e.corromper.contains(&ruta) {
        cuerpo[0] ^= 0xFF;
    }
    let desde = rango.unwrap_or(0);
    if rango.is_some() {
        e.rangos.push(format!("{ruta}:{desde}"));
    }
    let cortar = e.cortar_una_vez.remove(&ruta);
    drop(e);
    let parte = &cuerpo[desde.min(cuerpo.len())..];
    let cab = if rango.is_some() {
        format!("HTTP/1.1 206 Partial\r\nContent-Length: {}\r\nContent-Range: bytes {desde}-{}/{}\r\nConnection: close\r\n\r\n", parte.len(), cuerpo.len() - 1, cuerpo.len())
    } else {
        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", parte.len())
    };
    let _ = s.write_all(cab.as_bytes());
    if cortar {
        // Manda la mitad y corta la conexión.
        let _ = s.write_all(&parte[..parte.len() / 2]);
        let _ = s.flush();
        return;
    }
    let _ = s.write_all(parte);
}

fn datos(n: usize, semilla: u8) -> Vec<u8> {
    (0..n).map(|i| (i as u8).wrapping_mul(31).wrapping_add(semilla)).collect()
}

struct Banco {
    _d: tempfile::TempDir,
    rutas: Rutas,
    srv: Servidor,
    archivo: Archivo,
    cancelar: Arc<AtomicBool>,
}

fn banco(base_url: Option<&str>) -> Banco {
    let d = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(d.path());
    let contenido = datos(300_000, 7);
    let srv = servidor(vec![("voz/modelo.onnx".into(), contenido.clone())]);
    let archivo = Archivo { ruta: "onnx/modelo.onnx".into(), url: format!("{}/voz/modelo.onnx", base_url.unwrap_or(&srv.url)), bytes: contenido.len() as u64, sha256: sha256::de_bytes(&contenido) };
    Banco { _d: d, rutas, srv, archivo, cancelar: Arc::new(AtomicBool::new(false)) }
}

impl Banco {
    fn descarga(&self, avances: Arc<Mutex<Vec<(String, f64)>>>, paquetes: Arc<Mutex<Vec<String>>>) -> Descarga {
        let progreso: vortex_miku::descargas::Progreso = Arc::new(move |t, f| avances.lock().unwrap().push((t.into(), f)));
        let mut d = Descarga::nueva(&self.rutas, Some(vec!["voz".into()]), progreso, self.cancelar.clone(), Arc::new(move |p| paquetes.lock().unwrap().push(p.into())));
        d.lista = Some(vec![Paquete { id: "voz".into(), nombre: "la voz".into(), archivos: vec![self.archivo.clone()] }]);
        d.total = self.archivo.bytes;
        d.pausa = Duration::from_millis(20);
        d
    }

    fn destino(&self) -> std::path::PathBuf {
        self.rutas.datos.join("voz").join(&self.archivo.ruta)
    }
}

#[test]
fn baja_completo_con_su_progreso_y_su_sha256() {
    let b = banco(None);
    let (av, pa) = (Arc::new(Mutex::new(vec![])), Arc::new(Mutex::new(vec![])));
    let mut d = b.descarga(av.clone(), pa.clone());
    d.correr().unwrap();
    assert_eq!(std::fs::read(b.destino()).unwrap(), datos(300_000, 7));
    assert!(!b.destino().with_extension("onnx.parte").exists());
    assert_eq!(pa.lock().unwrap().as_slice(), ["voz"]);
    let av = av.lock().unwrap();
    assert!((av.last().unwrap().1 - 1.0).abs() < 1e-9, "termina en 100 %: {:?}", av.last());
    assert!(av.last().unwrap().0.contains("MB de"), "{:?}", av.last());
}

#[test]
fn reanuda_donde_se_quedo() {
    let b = banco(None);
    b.srv.estado.lock().unwrap().cortar_una_vez.insert("voz/modelo.onnx".into());
    let mut d = b.descarga(Default::default(), Default::default());
    d.correr().unwrap();
    assert_eq!(std::fs::read(b.destino()).unwrap(), datos(300_000, 7), "queda completo y bien");
    let e = b.srv.estado.lock().unwrap();
    assert_eq!(e.peticiones.len(), 2, "se reintentó una vez");
    assert!(e.rangos.len() == 1 && e.rangos[0].starts_with("voz/modelo.onnx:") && !e.rangos[0].ends_with(":0"), "pidió solo lo que faltaba: {:?}", e.rangos);
}

#[test]
fn lo_que_llega_distinto_no_se_instala() {
    let b = banco(None);
    b.srv.estado.lock().unwrap().corromper.insert("voz/modelo.onnx".into());
    let mut d = b.descarga(Default::default(), Default::default());
    let e = d.correr().unwrap_err();
    assert!(e.0.contains("llegó distinto de lo esperado (sha256); no se instala"), "{e}");
    assert!(!b.destino().exists(), "no se instaló");
    assert!(!std::path::Path::new(&format!("{}.parte", b.destino().display())).exists(), "ni dejó restos");
}

#[test]
fn un_404_no_se_reintenta() {
    let mut b = banco(None);
    b.archivo.url = format!("{}/no/existe", b.srv.url);
    let mut d = b.descarga(Default::default(), Default::default());
    let e = d.correr().unwrap_err();
    assert!(e.0.contains("No encuentro la voz de Miku en el servidor (error 404)"), "{e}");
    assert_eq!(b.srv.estado.lock().unwrap().peticiones.len(), 1);
}

#[test]
fn sin_red_lo_dice_en_español() {
    // Un puerto donde no escucha nadie.
    let puerto = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let b = banco(Some(&format!("http://127.0.0.1:{puerto}")));
    let mut d = b.descarga(Default::default(), Default::default());
    let e = d.correr().unwrap_err();
    assert!(e.0.contains("No pude bajar la voz de Miku (el servidor rechazó la conexión). ¿Hay internet?"), "{e}");
    assert!(!b.destino().exists());
}

#[test]
fn una_direccion_no_segura_no_se_baja() {
    let b = banco(Some("http://ejemplo.com"));
    let mut d = b.descarga(Default::default(), Default::default());
    assert_eq!(d.correr().unwrap_err().0, "La dirección de descarga no es segura; no se baja.");
}

#[test]
fn se_puede_cancelar() {
    let b = banco(None);
    b.cancelar.store(true, Ordering::SeqCst);
    let mut d = b.descarga(Default::default(), Default::default());
    assert_eq!(d.correr().unwrap_err().0, "Se canceló la descarga.");
    assert!(!b.destino().exists());
}
