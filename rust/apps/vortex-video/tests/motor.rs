//! Las 11 pruebas de `apps/vortex-video/pruebas/test_motor.py`, portadas, y algunas más.

use serde_json::json;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use vortex_video::ajustes::Ajustes;
use vortex_video::capturas::{self, Carpetas};
use vortex_video::estado::{self, Estado};
use vortex_video::formatos::{self, Formato};
use vortex_video::hora::FechaHora;
use vortex_video::{instancia, miniatura};

fn c920() -> Vec<Formato> {
    vec![
        Formato::new(640, 480, 30.0),
        Formato::new(1920, 1080, 5.0),
        Formato::new(1920, 1080, 30.0),
        Formato::new(1280, 720, 10.0),
        Formato::new(1280, 720, 30.0),
        Formato::new(2304, 1536, 2.0),
    ]
}

fn tocar(ruta: &Path, segundos: u64) {
    fs::write(ruta, b"").unwrap();
    let t = SystemTime::UNIX_EPOCH + Duration::from_secs(segundos);
    fs::File::options()
        .write(true)
        .open(ruta)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn carpeta(nombre: &str) -> std::path::PathBuf {
    vortex_core::carpeta_de_prueba(&format!("video-{nombre}"))
}

// --- Formatos (6 de Python) ---

#[test]
fn la_mayor_fluida_no_la_mayor_a_saltos() {
    let f = c920();
    assert_eq!(
        f[formatos::elegir(&f, "").unwrap()],
        Formato::new(1920, 1080, 30.0)
    );
}

#[test]
fn respeta_la_elegida() {
    let f = c920();
    let i = formatos::elegir(&f, "1280x720").unwrap();
    assert_eq!(f[i].fps, 30.0);
    assert_eq!(f[i].alto, 720);
}

#[test]
fn elegida_que_ya_no_existe() {
    let f = c920();
    assert_eq!(f[formatos::elegir(&f, "9x9").unwrap()].resolucion(), "1920x1080");
}

#[test]
fn camara_sin_nada_fluido() {
    let lenta = [Formato::new(640, 480, 10.0), Formato::new(320, 240, 15.0)];
    assert_eq!(formatos::elegir(&lenta, ""), Some(0));
}

#[test]
fn sin_formatos() {
    assert_eq!(formatos::elegir(&[], ""), None);
    assert_eq!(
        formatos::resoluciones(&c920()),
        ["1920x1080", "1280x720", "640x480"]
    );
}

#[test]
fn etiquetas() {
    assert_eq!(formatos::etiqueta("1920x1080"), "1920 × 1080 (Full HD)");
    assert_eq!(formatos::etiqueta("640x480"), "640 × 480");
    assert_eq!(formatos::etiqueta("3840x2160"), "3840 × 2160 (4K)");
}

// --- Archivos (5 de Python) ---

#[test]
fn nombre_no_pisa() {
    let d = carpeta("nombre");
    let ahora = FechaHora::new(2026, 11, 5, 14, 3, 21);
    let a = capturas::nombre_nuevo(&d, "Foto", ".jpg", ahora);
    assert_eq!(a.file_name().unwrap(), "Foto 2026-11-05 14.03.21.jpg");
    fs::write(&a, b"").unwrap();
    let b = capturas::nombre_nuevo(&d, "Foto", ".jpg", ahora);
    assert_eq!(b.file_name().unwrap(), "Foto 2026-11-05 14.03.21 (2).jpg");
    fs::write(&b, b"").unwrap();
    let c = capturas::nombre_nuevo(&d, "Foto", ".jpg", ahora);
    assert_eq!(c.file_name().unwrap(), "Foto 2026-11-05 14.03.21 (3).jpg");
}

#[test]
fn ultima_captura() {
    let d = carpeta("ultima");
    tocar(&d.join("a.jpg"), 1000);
    tocar(&d.join("b.mp4"), 2000);
    tocar(&d.join("nota.txt"), 3000);
    let no = d.join("no-existe");
    assert_eq!(
        capturas::ultima(&[&d, &no]).unwrap().file_name().unwrap(),
        "b.mp4"
    );
    assert_eq!(capturas::ultima(&[&no]), None);
    // Las extensiones en mayúsculas también cuentan (como en Python, que las baja).
    tocar(&d.join("C.JPG"), 2500);
    assert_eq!(capturas::ultima(&[&d]).unwrap().file_name().unwrap(), "C.JPG");
}

#[test]
fn ajustes() {
    let r = carpeta("ajustes").join("v").join("video.json");
    assert_eq!(Ajustes::leer_de(&r), Ajustes::default());
    let mut a = Ajustes::leer_de(&r);
    a.modo = "video".into();
    a.guardar_en(&r);
    assert_eq!(Ajustes::leer_de(&r).modo, "video");
}

#[test]
fn ajustes_rotos() {
    let r = carpeta("rotos").join("video.json");
    fs::write(&r, "no es json").unwrap();
    assert_eq!(Ajustes::leer_de(&r), Ajustes::default());
    fs::write(
        &r,
        json!({"modo": "raro", "temporizador": 7, "microfono": "si", "otra": 1}).to_string(),
    )
    .unwrap();
    assert_eq!(Ajustes::leer_de(&r), Ajustes::default());
}

#[test]
fn duracion() {
    assert_eq!(formatos::duracion(0), "00:00");
    assert_eq!(formatos::duracion(75_000), "01:15");
    assert_eq!(formatos::duracion(3_661_000), "1:01:01");
    assert_eq!(formatos::duracion(-5), "00:00");
}

// --- Extras: lo de puente.py ---

#[test]
fn lee_el_json_que_escribio_python() {
    // Así lo deja `guardar_ajustes` de Python (indent=2).
    let r = carpeta("python").join("video.json");
    fs::write(
        &r,
        "{\n  \"modo\": \"video\",\n  \"temporizador\": 5,\n  \"microfono\": false,\n  \"cuadricula\": true,\n  \"camara\": \"/dev/video0\",\n  \"resolucion\": \"1280x720\"\n}",
    )
    .unwrap();
    let a = Ajustes::leer_de(&r);
    assert_eq!(a.modo, "video");
    assert_eq!(a.temporizador, 5);
    assert!(!a.microfono && a.cuadricula);
    assert_eq!(a.camara, "/dev/video0");
    assert_eq!(a.resolucion, "1280x720");
}

#[test]
fn guardar_solo_claves_conocidas_y_su_tipo() {
    let d = carpeta("guardar");
    let r = d.join("video.json");
    let mut e = Estado::con(Carpetas::de_prueba(&d), r.clone());
    assert_eq!(e.ajuste("modo"), Some(json!("foto")));
    assert_eq!(e.ajuste("no-existe"), None);
    assert!(!e.guardar("no-existe", json!(1)));
    assert!(!e.guardar("microfono", json!("si")));
    assert!(!e.guardar("temporizador", json!(7)));
    assert!(
        !e.guardar("modo", json!("foto")),
        "igual que antes: no se escribe"
    );
    assert!(!r.exists());
    assert!(e.guardar("modo", json!("video")));
    assert!(e.guardar("temporizador", json!(10)));
    let leido: serde_json::Value = serde_json::from_str(&fs::read_to_string(&r).unwrap()).unwrap();
    assert_eq!(leido["modo"], "video");
    assert_eq!(leido["temporizador"], 10);
}

#[test]
fn rutas_nuevas_y_registrar() {
    let d = carpeta("rutas");
    let mut e = Estado::con(Carpetas::de_prueba(&d), d.join("video.json"));
    assert_eq!(e.ultima(), None);
    let ahora = FechaHora::new(2026, 11, 16, 9, 0, 0);
    let foto = e.ruta_foto(ahora).unwrap();
    assert_eq!(
        foto,
        d.join("PicturesLocation/Cámara/Foto 2026-11-16 09.00.00.jpg")
    );
    let video = e.ruta_video(ahora).unwrap();
    assert_eq!(
        video,
        d.join("MoviesLocation/Cámara/Video 2026-11-16 09.00.00.mp4")
    );
    assert!(!e.registrar(foto.to_str().unwrap()), "todavía no existe");
    fs::write(&video, b"").unwrap();
    let url = format!("file://{}", video.to_str().unwrap().replace(' ', "%20"));
    assert!(e.registrar(&url));
    assert_eq!(e.ultima(), Some(video.as_path()));
    assert!(e.ultima_es_video());
    // Al abrir de nuevo, la encuentra sola.
    let otra = Estado::con(Carpetas::de_prueba(&d), d.join("video.json"));
    assert_eq!(otra.ultima(), Some(video.as_path()));
    assert_eq!(
        e.carpeta_de("foto"),
        Some(d.join("PicturesLocation/Cámara").as_path())
    );
}

#[test]
fn urls() {
    assert_eq!(
        estado::desde_url("file:///home/ana/Foto%201.jpg"),
        "/home/ana/Foto 1.jpg"
    );
    assert_eq!(
        estado::desde_url("file:///home/ana/C%C3%A1mara/a.jpg"),
        "/home/ana/Cámara/a.jpg"
    );
    assert_eq!(estado::desde_url("/tmp/a.jpg"), "/tmp/a.jpg");
    assert_eq!(estado::desde_url("file:///a%zz%"), "/a%zz%");
    assert_eq!(estado::desde_url("file:///a%é"), "/a%é");
}

#[test]
fn carpetas_de_user_dirs() {
    let home = Path::new("/home/ana");
    let dirs = "# comentario\nXDG_PICTURES_DIR=\"$HOME/Imágenes\"\nXDG_VIDEOS_DIR=\"/datos/Vídeos\"\n";
    assert_eq!(
        capturas::carpeta_de_usuario(dirs, "XDG_PICTURES_DIR", home, "Pictures"),
        home.join("Imágenes")
    );
    assert_eq!(
        capturas::carpeta_de_usuario(dirs, "XDG_VIDEOS_DIR", home, "Videos"),
        Path::new("/datos/Vídeos")
    );
    assert_eq!(
        capturas::carpeta_de_usuario("", "XDG_VIDEOS_DIR", home, "Videos"),
        home.join("Videos")
    );
    // «$HOME/» solo (carpeta desactivada) no cuenta.
    let apagada = "XDG_VIDEOS_DIR=\"$HOME/\"\n";
    assert_eq!(
        capturas::carpeta_de_usuario(apagada, "XDG_VIDEOS_DIR", home, "Videos"),
        home.join("Videos")
    );
}

#[test]
fn sello_de_la_hora() {
    assert_eq!(FechaHora::new(2026, 1, 2, 3, 4, 5).sello(), "2026-01-02 03.04.05");
    let ahora = FechaHora::ahora();
    assert!(ahora.anio >= 2026 && (1..=12).contains(&ahora.mes));
}

#[test]
fn miniatura_con_ffmpeg_falso() {
    let d = carpeta("mini");
    let falso = d.join("ffmpeg");
    // Escribe el último argumento (la salida), como hace ffmpeg.
    fs::write(
        &falso,
        "#!/bin/sh\nfor a; do s=\"$a\"; done\nprintf 'jpg' > \"$s\"\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&falso, fs::Permissions::from_mode(0o755)).unwrap();
    let salida = d.join("cache/vortex-video/miniatura.jpg");
    assert!(miniatura::hacer_con(&falso, &d.join("v.mp4"), &salida));
    assert_eq!(fs::read(&salida).unwrap(), b"jpg");
    let malo = d.join("malo");
    fs::write(&malo, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&malo, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!miniatura::hacer_con(
        &malo,
        &d.join("v.mp4"),
        &d.join("otra.jpg")
    ));
    assert!(!miniatura::hacer_con(
        &d.join("no-hay"),
        &d.join("v.mp4"),
        &d.join("otra.jpg")
    ));
}

#[test]
fn una_sola_ventana() {
    let d = carpeta("instancia");
    let r = d.join("vortex-video-prueba");
    assert!(!instancia::avisar_a_la_abierta(&r), "no hay ninguna abierta");
    let escucha = instancia::Escucha::new(&r).unwrap();
    assert_eq!(escucha.revisar(), 0);
    assert!(instancia::avisar_a_la_abierta(&r));
    assert!(instancia::avisar_a_la_abierta(&r));
    std::thread::sleep(Duration::from_millis(50));
    assert_eq!(escucha.revisar(), 2);
    drop(escucha);
    assert!(!r.exists(), "al cerrar quita el socket");
    assert!(instancia::nombre().starts_with("vortex-video-"));
}
