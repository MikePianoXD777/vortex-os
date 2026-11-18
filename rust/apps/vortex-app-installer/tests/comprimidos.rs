//! Descomprimir sin peligro: rutas con «../», enlaces que se salen, no anidar, no pisar, contraseñas
//! (`test_comprimidos.py`).
mod comun;

use comun::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use vortex_app_installer::{Control, Error, motor};

fn listado(d: &Path) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()
}

fn ruta_de(v: &Value, k: &str) -> PathBuf {
    PathBuf::from(texto(v, k))
}

#[test]
fn rutas_peligrosas_no_salen() {
    let c = Casa::nueva();
    let trampa = c.tmp.path().join("trampa");
    std::fs::create_dir(&trampa).unwrap();
    let origen = peligroso_tar(&trampa.join("mal.tar.gz"));
    let antes = (listado(c.tmp.path()), listado(&trampa));
    let info = c.detectar(&origen);
    assert!(!texto(&motor::para_mostrar(&c.ctx, &info), "nota").is_empty(), "se avisa antes de descomprimir");
    let r = c.ejecutar(&info, "descomprimir", json!({})).unwrap();
    let destino = trampa.join("bien");
    assert_eq!(ruta_de(&r, "carpeta"), destino);
    assert!(r["saltados"].as_u64().unwrap() >= 3);
    assert!(!c.tmp.path().join("fuera.txt").exists() && !trampa.join("fuera.txt").exists());
    let mut despues_trampa = listado(&trampa);
    despues_trampa.remove("bien");
    assert_eq!((listado(c.tmp.path()), despues_trampa), antes);
    // nada dentro apunta fuera de la carpeta
    let real_destino = destino.canonicalize().unwrap();
    for e in todo(&destino) {
        let p = destino.join(&e);
        if p.is_symlink() {
            let real = p.canonicalize().unwrap_or_else(|_| p.clone());
            assert!(real.starts_with(&real_destino), "{}", p.display());
        }
    }
    assert_eq!(std::fs::read_to_string(destino.join("a.txt")).unwrap(), "a");
    assert!(destino.join("ok").is_symlink(), "el enlace bueno (a.txt, dentro) se queda");
    assert!(!destino.join("sube").exists() && !destino.join("absoluto").is_symlink());
    assert!(listado(&trampa).iter().all(|n| !n.starts_with(".vortex-descomprimiendo-")), "sin restos");
}

#[test]
fn zip_con_rutas_peligrosas() {
    let c = Casa::nueva();
    let ruta = zip_(
        &c.descargas.join("malo.zip"),
        &[("../../escapado.txt", b"x"), ("/absoluto.txt", b"y"), ("normal/ok.txt", b"z")],
    );
    let info = c.detectar(&ruta);
    let r = c.ejecutar(&info, "descomprimir", json!({})).unwrap();
    assert!(!c.home.join("escapado.txt").exists() && !c.tmp.path().join("escapado.txt").exists());
    assert_eq!(std::fs::read_to_string(c.descargas.join("malo/normal/ok.txt")).unwrap(), "z");
    assert_eq!(r["saltados"], 1);
}

#[test]
fn una_sola_carpeta_no_se_anida() {
    let c = Casa::nueva();
    let ruta = tar(
        &c.descargas.join("Proyecto-2.0.tar.gz"),
        &[carpeta("Proyecto-2.0/"), arch("Proyecto-2.0/a.txt", b"a")],
        "gz",
    );
    c.ejecutar(&c.detectar(&ruta), "descomprimir", json!({})).unwrap();
    assert!(c.descargas.join("Proyecto-2.0/a.txt").exists());
    assert!(!c.descargas.join("Proyecto-2.0/Proyecto-2.0").exists());
}

#[test]
fn varias_cosas_van_a_una_carpeta_con_su_nombre() {
    let c = Casa::nueva();
    let ruta = zip_(&c.descargas.join("fotos.zip"), &[("a.jpg", b"a"), ("b.jpg", b"b")]);
    c.ejecutar(&c.detectar(&ruta), "descomprimir", json!({})).unwrap();
    assert_eq!(todo(&c.descargas.join("fotos")), ["a.jpg", "b.jpg"]);
}

#[test]
fn no_pisa_lo_que_existe() {
    let c = Casa::nueva();
    let ruta = zip_(&c.descargas.join("fotos.zip"), &[("a.jpg", b"nuevo")]);
    std::fs::create_dir(c.descargas.join("fotos")).unwrap();
    std::fs::write(c.descargas.join("fotos/a.jpg"), b"mio").unwrap();
    let info = c.detectar(&ruta);
    let sugerida = match c.ejecutar(&info, "descomprimir", json!({})) {
        Err(Error::Conflicto { sugerida, .. }) => sugerida,
        otro => panic!("se esperaba un conflicto: {otro:?}"),
    };
    assert_eq!(sugerida.file_name().unwrap(), "fotos (2)");
    assert_eq!(std::fs::read(c.descargas.join("fotos/a.jpg")).unwrap(), b"mio");
    c.ejecutar(&info, "descomprimir", json!({"nombre": "fotos (2)"})).unwrap();
    assert_eq!(std::fs::read(c.descargas.join("fotos (2)/a.jpg")).unwrap(), b"nuevo");
}

#[test]
fn en_otra_carpeta() {
    let c = Casa::nueva();
    let ruta = zip_(&c.descargas.join("x.zip"), &[("x/1.txt", b"1")]);
    let otra = c.home.join("Documentos");
    let r = c.ejecutar(&c.detectar(&ruta), "descomprimir", json!({"carpeta": otra.to_string_lossy()})).unwrap();
    assert!(otra.join("x/1.txt").exists());
    assert_eq!(ruta_de(&r, "carpeta"), otra.join("x"));
}

#[test]
fn contrasena() {
    let c = Casa::nueva();
    let fuente = c.tmp.path().join("secreto");
    std::fs::create_dir(&fuente).unwrap();
    std::fs::write(fuente.join("s.txt"), "secreto").unwrap();
    let ruta = c.descargas.join("c.zip");
    let ok = Command::new("bsdtar")
        .args(["--format", "zip", "--options", "zip:encryption=aes256", "--passphrase", "clave", "-cf"])
        .arg(&ruta)
        .arg("-C")
        .arg(&fuente)
        .arg("s.txt")
        .status()
        .unwrap();
    assert!(ok.success());
    let info = c.detectar(&ruta);
    match c.ejecutar(&info, "descomprimir", json!({})) {
        Err(Error::Contrasena { incorrecta, .. }) => assert!(!incorrecta),
        otro => panic!("se esperaba pedir contraseña: {otro:?}"),
    }
    match c.ejecutar(&info, "descomprimir", json!({"contrasena": "mala"})) {
        Err(Error::Contrasena { incorrecta, .. }) => assert!(incorrecta),
        otro => panic!("se esperaba contraseña incorrecta: {otro:?}"),
    }
    assert!(!c.descargas.join("c").exists(), "lo de a medias no se queda");
    c.ejecutar(&info, "descomprimir", json!({"contrasena": "clave"})).unwrap();
    assert_eq!(std::fs::read_to_string(c.descargas.join("c/s.txt")).unwrap(), "secreto");
}

fn aleatorio(n: usize) -> Vec<u8> {
    let mut v = vec![0u8; n];
    std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut v)).unwrap();
    v
}

#[test]
fn suelto_y_progreso() {
    let c = Casa::nueva();
    let ruta = c.descargas.join("grande.txt.gz");
    std::fs::write(&ruta, comprimir(&aleatorio(3 << 20), "gz")).unwrap();
    let avances = Arc::new(std::sync::Mutex::new(Vec::<f64>::new()));
    let copia = avances.clone();
    let control = Control::nuevo(move |f| copia.lock().unwrap().push(f), Arc::default());
    motor_ejecutar_con(&c, &c.detectar(&ruta), "descomprimir", json!({}), &control).unwrap();
    assert_eq!(std::fs::metadata(c.descargas.join("grande.txt")).unwrap().len(), 3 << 20);
    let a = avances.lock().unwrap();
    assert!(!a.is_empty() && (a[a.len() - 1] - 1.0).abs() <= 0.01);
    assert!(a.windows(2).all(|w| w[0] <= w[1]), "el avance solo sube");
}

#[test]
fn cancelar_no_deja_restos() {
    let c = Casa::nueva();
    let datos = aleatorio(1 << 20);
    let entradas: Vec<E> = (0..20).map(|i| arch(&format!("p/{i}.bin"), &datos)).collect();
    let ruta = tar(&c.descargas.join("pesado.tar"), &entradas, "");
    let cancelar = Arc::new(AtomicBool::new(false));
    cancelar.store(true, Ordering::SeqCst);
    let control = Control::con_cancelar(cancelar);
    let r = motor_ejecutar_con(&c, &c.detectar(&ruta), "descomprimir", json!({}), &control);
    assert!(matches!(r, Err(Error::Cancelado)), "{r:?}");
    assert!(!c.descargas.join("p").exists());
    assert!(listado(&c.descargas).iter().all(|n| !n.starts_with(".vortex-")));
}

#[test]
fn iso_de_solo_lectura_se_puede_quitar() {
    let c = Casa::nueva();
    let fuente = c.tmp.path().join("f");
    std::fs::create_dir_all(fuente.join("dir")).unwrap();
    std::fs::write(fuente.join("dir/a.txt"), "a").unwrap();
    let ruta = c.descargas.join("disco.iso");
    let ok = Command::new("bsdtar")
        .args(["--format", "iso9660", "-cf"])
        .arg(&ruta)
        .arg("-C")
        .arg(&fuente)
        .arg("dir")
        .status()
        .unwrap();
    assert!(ok.success());
    let r = c.ejecutar(&c.detectar(&ruta), "descomprimir", json!({})).unwrap();
    let destino = texto(&r, "carpeta");
    let e = c.registro().remove(0);
    assert_eq!(e["tipo"], "descomprimido");
    assert_eq!(e["rutas"], json!([destino]));
    motor::deshacer(&c.ctx, e["id"].as_str().unwrap()).unwrap();
    assert!(!Path::new(&destino).exists());
    let papelera = c.home.join(".local/share/Trash/files");
    assert!(std::fs::read_dir(papelera).unwrap().next().is_some(), "a la papelera, no borrado");
}

#[test]
fn falta_7z_lo_dice_con_el_comando() {
    let mut c = Casa::nueva();
    // Un PATH sin 7z, 7zz, 7za ni unrar (solo lo que hace falta para leer el archivo).
    c.solo(&["bsdtar", "head", "file", "sh", "cat"]);
    let ruta = c.descargas.join("raro.7z");
    let mut datos = b"7z\xbc\xaf\x27\x1c".to_vec();
    datos.extend_from_slice(&[0u8; 30]);
    std::fs::write(&ruta, datos).unwrap();
    let info = c.detectar(&ruta);
    assert_eq!(info.falta_comando, "sudo pacman -S 7zip");
    assert_eq!(texto(&motor::para_mostrar(&c.ctx, &info), "comando"), "sudo pacman -S 7zip");
}
