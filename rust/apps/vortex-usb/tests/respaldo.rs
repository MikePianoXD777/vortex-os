//! El respaldo a la memoria, en carpetas temporales: solo lo nuevo, nunca borra, y se salta lo que no es tuyo.

use std::fs;
use std::path::Path;
use std::time::Duration;
use vortex_usb::respaldo::*;

fn poner(p: &Path, texto: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, texto).unwrap();
}

fn correr(home: &Path, usb: &Path) -> Resumen {
    let raiz = usb.join("Respaldo de prueba");
    let (lista, iguales) = plan(home, &["Documentos".into(), "Imágenes".into()], &raiz);
    copiar_todo(&lista, iguales, Duration::from_secs(60), &mut |_, _| {})
}

#[test]
fn solo_lo_nuevo_y_nunca_borra() {
    let t = tempfile::tempdir().unwrap();
    let (home, usb) = (t.path().join("home"), t.path().join("usb"));
    poner(&home.join("Documentos/tarea.md"), "uno");
    poner(&home.join("Documentos/escuela/física.pdf"), "pdf");
    poner(&home.join("Imágenes/foto.jpg"), "jpg");
    poner(&home.join("Música/no-se-respalda.mp3"), "mp3");
    fs::create_dir_all(&usb).unwrap();

    let r = correr(&home, &usb);
    assert_eq!((r.copiados, r.bytes, r.iguales, r.fallidos), (3, 9, 0, 0));
    let raiz = usb.join("Respaldo de prueba");
    assert_eq!(
        fs::read_to_string(raiz.join("Documentos/escuela/física.pdf")).unwrap(),
        "pdf"
    );
    assert!(!raiz.join("Música").exists());

    // La segunda vez no copia nada.
    let r = correr(&home, &usb);
    assert_eq!((r.copiados, r.iguales), (0, 3));

    // Cambias uno y borras otro: se copia el cambiado y el borrado sigue en la memoria.
    std::thread::sleep(Duration::from_millis(20));
    poner(&home.join("Documentos/tarea.md"), "uno y dos");
    fs::remove_file(home.join("Imágenes/foto.jpg")).unwrap();
    let r = correr(&home, &usb);
    assert_eq!((r.copiados, r.iguales), (1, 1));
    assert_eq!(
        fs::read_to_string(raiz.join("Documentos/tarea.md")).unwrap(),
        "uno y dos"
    );
    assert!(raiz.join("Imágenes/foto.jpg").exists());
    // Sin temporales tirados.
    assert!(!raiz.join("Documentos/tarea.md.vortex-tmp").exists());
}

#[test]
fn se_salta_lo_que_no_es_tuyo() {
    let t = tempfile::tempdir().unwrap();
    let (home, usb) = (t.path().join("home"), t.path().join("usb"));
    let d = home.join("Documentos");
    poner(&d.join("proyecto/main.rs"), "fn main() {}");
    poner(&d.join("proyecto/.git/HEAD"), "ref");
    poner(
        &d.join("proyecto/target/CACHEDIR.TAG"),
        "Signature: 8a477f597d28d172789f06886806bc55",
    );
    poner(&d.join("proyecto/target/debug/enorme"), "binario");
    poner(&d.join("web/node_modules/x/index.js"), "js");
    poner(&d.join(".oculto/secreto"), "x");
    std::os::unix::fs::symlink("/etc/hostname", d.join("enlace")).unwrap();
    fs::create_dir_all(&usb).unwrap();

    let r = correr(&home, &usb);
    assert_eq!(r.copiados, 1);
    let raiz = usb.join("Respaldo de prueba/Documentos");
    assert!(raiz.join("proyecto/main.rs").exists());
    for no in [
        "proyecto/.git",
        "proyecto/target",
        "web/node_modules",
        ".oculto",
        "enlace",
    ] {
        assert!(!raiz.join(no).exists(), "no debía copiar {no}");
    }
}

#[test]
fn la_marca_de_fabrica() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("Documentos")).unwrap();
    // Sin user-dirs.dirs propio en la prueba, se usan los nombres en español que existan.
    let c = carpetas_de_fabrica(t.path());
    assert!(c.iter().all(|x| t.path().join(x).is_dir()));
}
