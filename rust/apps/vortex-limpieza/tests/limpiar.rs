//! La limpieza en carpetas temporales: borra solo lo viejo de las cachés y nunca la papelera sin preguntar.

use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, SystemTime};
use vortex_limpieza::*;

fn archivo(p: &Path, bytes: usize, dias: u64) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![b'x'; bytes]).unwrap();
    let t = SystemTime::now() - Duration::from_secs(dias * 86_400);
    File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

fn hace(dias: u64) -> SystemTime {
    SystemTime::now() - Duration::from_secs(dias * 86_400)
}

#[test]
fn solo_lo_viejo_archivo_por_archivo() {
    let t = tempfile::tempdir().unwrap();
    let c = t.path().join("thumbnails");
    archivo(&c.join("large/viejo.png"), 1000, 40);
    archivo(&c.join("large/nuevo.png"), 500, 2);
    archivo(&c.join("normal/viejo2.png"), 200, 90);
    let (lista, total) = viejos(&c, hace(DIAS));
    assert_eq!(lista.len(), 2);
    assert_eq!(total, 1200);
    assert_eq!(borrar(&lista, &c), 1200);
    assert!(c.join("large/nuevo.png").exists());
    assert!(!c.join("normal").exists(), "la carpeta que quedó vacía se va");
    assert!(c.exists(), "la raíz de la caché se queda");
}

#[test]
fn los_repositorios_de_yay_enteros_o_nada() {
    let t = tempfile::tempdir().unwrap();
    let c = t.path().join("yay");
    archivo(&c.join("viejo/.git/objects/aa"), 3000, 60);
    archivo(&c.join("viejo/PKGBUILD"), 100, 60);
    archivo(&c.join("usado/.git/objects/bb"), 3000, 60);
    archivo(&c.join("usado/PKGBUILD"), 100, 1); // se compiló hace poco: no se toca nada de él
    let (lista, total) = carpetas_viejas(&c, hace(DIAS));
    assert_eq!(lista, vec![c.join("viejo")]);
    assert_eq!(total, 3100);
    assert_eq!(borrar_carpetas(&lista), 3100);
    assert!(!c.join("viejo").exists());
    assert!(c.join("usado/.git/objects/bb").exists());
}

#[test]
fn la_papelera_solo_lo_de_hace_mas_de_un_mes() {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("Trash");
    let hoy = dias_desde_1970(2026, 11, 19);
    let poner = |nombre: &str, fecha: &str, carpeta: bool| {
        if carpeta {
            archivo(&p.join("files").join(nombre).join("dentro.txt"), 700, 0);
        } else {
            archivo(&p.join("files").join(nombre), 400, 0);
        }
        fs::write(
            p.join("info").join(format!("{nombre}.trashinfo")),
            format!("[Trash Info]\nPath=/home/a/{nombre}\nDeletionDate={fecha}T10:00:00\n"),
        )
        .unwrap();
    };
    fs::create_dir_all(p.join("info")).unwrap();
    poner("viejo.txt", "2026-09-01", false);
    poner("carpeta vieja", "2026-08-15", true);
    poner("reciente.txt", "2026-11-10", false);
    let (lista, total) = papelera_vieja(&p, hoy);
    assert_eq!(lista.len(), 2);
    assert_eq!(total, 1100);
    vaciar(&lista);
    assert!(!p.join("files/viejo.txt").exists());
    assert!(!p.join("files/carpeta vieja").exists());
    assert!(!p.join("info/viejo.txt.trashinfo").exists());
    assert!(p.join("files/reciente.txt").exists());
    assert!(p.join("info/reciente.txt.trashinfo").exists());
}

#[test]
fn sin_carpetas_no_truena() {
    let t = tempfile::tempdir().unwrap();
    let nada = t.path().join("no-existe");
    assert_eq!(viejos(&nada, hace(1)).0.len(), 0);
    assert_eq!(carpetas_viejas(&nada, hace(1)).0.len(), 0);
    assert_eq!(papelera_vieja(&nada, 0).0.len(), 0);
    assert_eq!(peso(&nada), 0);
}

/// 4.4: de Descargas solo las sobras (instaladores, comprimidos, descargas a medias) de hace más de un mes, y
/// nunca las que dijiste que se quedan.
#[test]
fn descargas_solo_sobras_viejas() {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join("Descargas");
    archivo(&d.join("Discord-0.0.80.tar.gz"), 1000, 45);
    archivo(&d.join("setup.EXE"), 500, 60);
    archivo(&d.join("guardame.zip"), 300, 90);
    archivo(&d.join("nuevo.zip"), 700, 3);
    archivo(&d.join("foto.jpg"), 4000, 200);
    archivo(&d.join("tarea.pdf"), 4000, 200);
    archivo(&d.join("carpeta/dentro.zip"), 4000, 200);
    let guardadas = vec![d.join("guardame.zip")];
    let (lista, total) = descargas_viejas(&d, hace(DIAS), &guardadas);
    assert_eq!(lista, vec![d.join("Discord-0.0.80.tar.gz"), d.join("setup.EXE")]);
    assert_eq!(total, 1500);

    let g = t.path().join("estado/descargas-que-se-quedan");
    guardar_guardadas(&g, &[d.join("guardame.zip"), d.join("ya-no-existe.zip")]);
    assert_eq!(leer_guardadas(&g), vec![d.join("guardame.zip")]);
}

#[test]
fn la_carpeta_de_descargas() {
    let t = tempfile::tempdir().unwrap();
    let home = t.path();
    let config = home.join(".config");
    fs::create_dir_all(home.join("Bajadas")).unwrap();
    fs::create_dir_all(home.join("Downloads")).unwrap();
    fs::create_dir_all(&config).unwrap();
    assert_eq!(carpeta_descargas(home, &config), Some(home.join("Downloads")));
    fs::write(
        config.join("user-dirs.dirs"),
        "XDG_DOWNLOAD_DIR=\"$HOME/Bajadas\"\n",
    )
    .unwrap();
    assert_eq!(carpeta_descargas(home, &config), Some(home.join("Bajadas")));
    // Si apunta al home mismo (sin carpeta de descargas), no se toca nada del home.
    fs::write(config.join("user-dirs.dirs"), "XDG_DOWNLOAD_DIR=\"$HOME/\"\n").unwrap();
    assert_eq!(carpeta_descargas(home, &config), Some(home.join("Downloads")));
}
