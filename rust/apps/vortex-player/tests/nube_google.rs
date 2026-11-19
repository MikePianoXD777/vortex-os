//! `test_nube.py`: la sincronía entre dos aparatos con un Drive en memoria, la biblioteca vieja que se pone al
//! día, e iniciar sesión en Google de punta a punta contra un Google de mentira (el «navegador» va al puerto
//! local como lo haría Google). No sale a internet ni toca tu sesión.

use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashSet;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use vortex_player::biblioteca::{Biblioteca, Orden, id_de};
use vortex_player::cuenta_google::{self as cg, Carpeta, Cuenta, ErrorGoogle, Http};
use vortex_player::sincronia;

/// La carpeta oculta de Drive, en memoria: lo que comparten la PC y el teléfono.
#[derive(Default)]
struct DriveFalso {
    archivo: RefCell<Option<String>>,
    escrituras: RefCell<u32>,
}

impl Carpeta for DriveFalso {
    fn buscar(&self) -> Result<Option<String>, ErrorGoogle> {
        Ok(self.archivo.borrow().as_ref().map(|_| "f1".into()))
    }
    fn leer(&self, _: &str) -> Result<Vec<u8>, ErrorGoogle> {
        Ok(self.archivo.borrow().clone().unwrap_or_default().into_bytes())
    }
    fn crear(&self, texto: &str) -> Result<String, ErrorGoogle> {
        *self.archivo.borrow_mut() = Some(texto.into());
        *self.escrituras.borrow_mut() += 1;
        Ok("f1".into())
    }
    fn escribir(&self, _: &str, texto: &str) -> Result<(), ErrorGoogle> {
        *self.archivo.borrow_mut() = Some(texto.into());
        *self.escrituras.borrow_mut() += 1;
        Ok(())
    }
}

fn bib(t: &Path, nombre: &str) -> Mutex<Biblioteca> {
    Mutex::new(Biblioteca::abrir(Some(&t.join(nombre).join("biblioteca.db")), "ffprobe").unwrap())
}

/// Como si Vortex Player la hubiera bajado (sin ffprobe ni archivo de verdad).
fn bajada(t: &Path, nombre: &str, vid: &str, titulo: &str) -> String {
    let ruta = format!("/musica/Vortex Player/Artista/{titulo} [{vid}].opus");
    let db = rusqlite::Connection::open(t.join(nombre).join("biblioteca.db")).unwrap();
    db.execute(
        "INSERT OR REPLACE INTO canciones (ruta, mtime, titulo, artista, album, duracion, codec, origen, agregada) \
         VALUES (?1, 0, ?2, 'Artista', '', 200.4, 'opus', 'youtube', ?3)",
        rusqlite::params![ruta, titulo, 1_790_000_000.0],
    )
    .unwrap();
    ruta
}

fn sinc(b: &Mutex<Biblioteca>, d: &DriveFalso, borradas: &mut Vec<String>, quitar: bool) -> Vec<String> {
    let (_, plan) = sincronia::sincronizar(b, d, &mut |bib, vid, ruta| {
        borradas.push(vid.into());
        if quitar {
            bib.quitar(ruta).unwrap();
        }
    })
    .unwrap();
    plan.bajar.into_iter().map(|(v, _)| v).collect()
}

#[test]
fn la_pc_y_el_telefono_quedan_iguales() {
    let t = tempfile::tempdir().unwrap();
    let drive = DriveFalso::default();
    let (pc, tel) = (bib(t.path(), "pc"), bib(t.path(), "tel"));
    let r1 = bajada(t.path(), "pc", "aaaaaaaaaaa", "Idol");
    bajada(t.path(), "pc", "bbbbbbbbbbb", "Yoru ni Kakeru");
    let pid = pc.lock().unwrap().crear_playlist("Anime", "").unwrap();
    pc.lock().unwrap().agregar_a_playlist(pid, &r1).unwrap();
    let local_tel = bajada(t.path(), "tel", "ccccccccccc", "Solo del teléfono");

    let mut borradas = Vec::new();
    assert!(sinc(&pc, &drive, &mut borradas, false).is_empty());
    let (_, plan) = sincronia::sincronizar(&tel, &drive, &mut |_, _, _| {}).unwrap();
    let faltan: HashSet<String> = plan.bajar.iter().map(|(v, _)| v.clone()).collect();
    assert_eq!(faltan, HashSet::from(["aaaaaaaaaaa".into(), "bbbbbbbbbbb".into()]));
    // La playlist ya existe en el teléfono, vacía hasta que se bajen sus canciones
    let nombres = |b: &Mutex<Biblioteca>| b.lock().unwrap().playlists().unwrap().into_iter().map(|p| p.nombre).collect::<Vec<_>>();
    assert_eq!(nombres(&tel), ["Anime"]);
    for (vid, datos) in &plan.bajar {
        bajada(t.path(), "tel", vid, &datos.titulo);
    }
    sinc(&tel, &drive, &mut borradas, false);
    let p = tel.lock().unwrap().playlists().unwrap().remove(0);
    let en_playlist: Vec<String> =
        tel.lock().unwrap().canciones_de_playlist(p.id).unwrap().into_iter().map(|c| c.titulo).collect();
    assert_eq!(en_playlist, ["Idol"]);

    // La del teléfono llega a la PC
    assert_eq!(sinc(&pc, &drive, &mut borradas, false), ["ccccccccccc"]);

    // Borrar en el teléfono → se borra en la PC; renombrar en la PC → se renombra en el teléfono
    {
        let b = tel.lock().unwrap();
        b.anotar_cancion_borrada("bbbbbbbbbbb").unwrap();
        b.quitar(&b.rutas_por_id().unwrap()["bbbbbbbbbbb"]).unwrap();
    }
    sinc(&tel, &drive, &mut Vec::new(), false);
    pc.lock().unwrap().renombrar_playlist(pid, "Anime 2026").unwrap();
    sinc(&pc, &drive, &mut borradas, true);
    assert_eq!(borradas, ["bbbbbbbbbbb"]);
    sinc(&tel, &drive, &mut Vec::new(), false);
    assert_eq!(nombres(&tel), ["Anime 2026"]);
    let rutas_tel: Vec<String> =
        tel.lock().unwrap().canciones("", Orden::Agregada).unwrap().into_iter().map(|c| c.ruta).collect();
    assert!(rutas_tel.contains(&local_tel));

    // Sin cambios no se vuelve a escribir en Drive
    let antes = *drive.escrituras.borrow();
    sinc(&pc, &drive, &mut Vec::new(), false);
    sinc(&tel, &drive, &mut Vec::new(), false);
    assert_eq!(antes, *drive.escrituras.borrow());
}

#[test]
fn la_biblioteca_vieja_se_pone_al_dia_y_lo_borrado_queda_anotado() {
    let t = tempfile::tempdir().unwrap();
    let archivo = t.path().join("vieja.db");
    {
        let db = rusqlite::Connection::open(&archivo).unwrap();
        db.execute_batch(
            "CREATE TABLE playlists (id INTEGER PRIMARY KEY AUTOINCREMENT, nombre TEXT NOT NULL, url TEXT DEFAULT '', creada REAL);\
             INSERT INTO playlists (nombre, url, creada) VALUES ('Chill piano', 'https://x', 100);",
        )
        .unwrap();
    }
    let b = Biblioteca::abrir(Some(&archivo), "ffprobe").unwrap();
    let local = b.estado_nube().unwrap();
    let (_, p) = local.playlists.iter().next().unwrap();
    assert_eq!((p.nombre.as_str(), p.modificada), ("Chill piano", 100000));
    assert_eq!(local.playlists, b.estado_nube().unwrap().playlists); // el uuid se queda

    let b = bib(t.path(), "pc");
    let b = b.lock().unwrap();
    let pid = b.crear_playlist("Anime", "").unwrap();
    let u = b.estado_nube().unwrap().playlists.keys().next().unwrap().clone();
    b.anotar_cancion_borrada("aaaaaaaaaaa").unwrap();
    b.borrar_playlist(pid, true).unwrap();
    let local = b.estado_nube().unwrap();
    assert!(local.quitadas.contains_key("aaaaaaaaaaa"));
    assert!(local.borradas.contains_key(&u));
    assert_eq!(id_de("/m/Idol [aaaaaaaaaaa].opus"), "aaaaaaaaaaa");
    assert_eq!(id_de("/m/Idol.flac"), "");
}

// --- Google de mentira -----------------------------------------------------------------------------------

fn form(cuerpo: Option<&[u8]>) -> Vec<(String, String)> {
    let Some(c) = cuerpo.map(|c| String::from_utf8_lossy(c).to_string()) else { return vec![] };
    if !c.contains('=') || c.contains('{') {
        return vec![];
    }
    c.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.replace('+', " ")))
        .collect()
}

type Pedidos = Arc<Mutex<Vec<(String, String, Vec<(String, String)>)>>>;

fn google_falso() -> (Cuenta, Pedidos, tempfile::TempDir) {
    let t = tempfile::tempdir().unwrap();
    let pedidos: Pedidos = Arc::default();
    let p2 = pedidos.clone();
    let http: Http = Arc::new(move |metodo, url, cuerpo, _cab| {
        let datos = form(cuerpo);
        let grant = datos.iter().find(|(k, _)| k == "grant_type").map(|(_, v)| v.clone()).unwrap_or_default();
        p2.lock().unwrap().push((metodo.into(), url.into(), datos));
        let r = |v: Value| Ok((200, v.to_string().into_bytes()));
        if url == cg::TOKEN && grant == "authorization_code" {
            return r(json!({"access_token": "acceso1", "refresh_token": "renovar1", "expires_in": 3600}));
        }
        if url == cg::TOKEN && grant == "refresh_token" {
            return r(json!({"access_token": "acceso2", "expires_in": 3600}));
        }
        if url.contains("/about") {
            return r(json!({"user": {"displayName": "Mike", "emailAddress": "mike@ejemplo.com"}}));
        }
        if url == cg::REVOCAR {
            return r(json!({}));
        }
        Ok((404, vec![]))
    });
    let mut c = Cuenta::en(t.path().join("config"), http);
    c.cliente_fijo = Some(("id-prueba".into(), "secreto-prueba".into()));
    (c, pedidos, t)
}

/// Lo que haría el navegador: ir al puerto local con `code` y `state` (o el `state` que le digas).
fn navegador(estado: Option<&'static str>) -> impl Fn(&str) {
    move |url: &str| {
        let q: Vec<(String, String)> = url
            .split_once('?')
            .unwrap()
            .1
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .map(|(k, v)| (k.to_string(), v.replace("%3A", ":").replace("%2F", "/")))
            .collect();
        let d = |k: &str| q.iter().find(|(a, _)| a == k).unwrap().1.clone();
        assert_eq!(d("scope"), "https://www.googleapis.com/auth/drive.appdata");
        assert_eq!(d("code_challenge_method"), "S256");
        let puerto = d("redirect_uri").rsplit(':').next().unwrap().to_string();
        let st = estado.map(String::from).unwrap_or_else(|| d("state"));
        std::thread::spawn(move || {
            let mut c = std::net::TcpStream::connect(format!("127.0.0.1:{puerto}")).unwrap();
            c.write_all(format!("GET /?code=codigo1&state={st} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes()).unwrap();
            let mut s = String::new();
            let _ = c.read_to_string(&mut s);
        });
    }
}

#[test]
fn iniciar_y_cerrar_sesion() {
    let (c, pedidos, t) = google_falso();
    let d = c.iniciar_sesion(&navegador(None), Duration::from_secs(10)).unwrap();
    assert_eq!(d, json!({"correo": "mike@ejemplo.com", "nombre": "Mike"}));
    let archivo = t.path().join("config/google.json");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(&archivo).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(c.sesion()["refresh_token"], "renovar1");
    let intercambio = pedidos
        .lock()
        .unwrap()
        .iter()
        .find(|(_, _, p)| p.iter().any(|(k, v)| k == "grant_type" && v == "authorization_code"))
        .unwrap()
        .2
        .clone();
    assert!(intercambio.iter().any(|(k, v)| k == "code_verifier" && v.len() >= 43));
    assert!(intercambio.iter().any(|(k, v)| k == "code" && v == "codigo1"));

    assert_eq!(c.token().unwrap(), "acceso1");
    c.olvidar_token();
    assert_eq!(c.token().unwrap(), "acceso2");

    c.cerrar_sesion();
    assert!(!archivo.exists());
    assert_eq!(c.sesion(), json!({}));
    c.olvidar_token();
    assert!(matches!(c.token(), Err(ErrorGoogle::SinPermiso(_))));
}

#[test]
fn un_estado_que_no_coincide_no_inicia_sesion() {
    let (c, _, _t) = google_falso();
    assert!(c.iniciar_sesion(&navegador(Some("otro")), Duration::from_secs(10)).is_err());
    assert_eq!(c.sesion(), json!({}));
}

#[test]
fn drive_sube_y_lee_el_archivo() {
    let guardado: Arc<Mutex<Option<String>>> = Arc::default();
    let g = guardado.clone();
    let http: Http = Arc::new(move |metodo, url, cuerpo, cab| {
        assert!(cab.contains(&("Authorization", "Bearer t")));
        let cuerpo = cuerpo.map(|c| String::from_utf8_lossy(c).to_string()).unwrap_or_default();
        let mut g = g.lock().unwrap();
        if url.contains("files?spaces=appDataFolder") {
            assert!(url.contains("q=name%20%3D%20%27vortex-player.json%27"));
            let f = if g.is_some() { json!([{"id": "f9"}]) } else { json!([]) };
            return Ok((200, json!({"files": f}).to_string().into_bytes()));
        }
        if metodo == "POST" && url.contains("uploadType=multipart") {
            assert!(cuerpo.contains(r#""parents": ["appDataFolder"]"#), "{cuerpo}");
            *g = Some(cuerpo.split("\r\n\r\n").nth(2).unwrap().split("\r\n--").next().unwrap().to_string());
            return Ok((200, br#"{"id": "f9"}"#.to_vec()));
        }
        if metodo == "PATCH" {
            *g = Some(cuerpo);
            return Ok((200, b"{}".to_vec()));
        }
        if url.ends_with("/files/f9?alt=media") {
            return Ok((200, g.clone().unwrap().into_bytes()));
        }
        Ok((401, vec![]))
    });
    let t = tempfile::tempdir().unwrap();
    let c = Cuenta::en(t.path().to_path_buf(), http);
    let dr = c.drive("t", cg::ARCHIVO);
    assert_eq!(dr.buscar().unwrap(), None);
    dr.crear(r#"{"formato": 1}"#).unwrap();
    assert_eq!(guardado.lock().unwrap().as_deref(), Some(r#"{"formato": 1}"#));
    assert_eq!(dr.buscar().unwrap().as_deref(), Some("f9"));
    dr.escribir("f9", r#"{"formato": 2}"#).unwrap();
    assert_eq!(dr.leer("f9").unwrap(), br#"{"formato": 2}"#);
    assert!(matches!(dr.usuario(), Err(ErrorGoogle::SinPermiso(_))));
}

#[test]
fn pkce_como_el_rfc() {
    let (v, reto) = cg::pkce();
    assert!(v.len() >= 43 && v.len() <= 128);
    assert_eq!(reto.len(), 43);
    assert!(!reto.contains('=') && !reto.contains('+') && !reto.contains('/'));
}
