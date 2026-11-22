//! El cargador de paquetes Shimeji con imágenes de mentira (rectángulos de colores, 64×64).
use vortex_mascota::{aspecto::Aspecto, mundo::Anim, png};

fn imagen(color: u32) -> Vec<u8> {
    let mut px = vec![0u32; 64 * 64];
    for y in 16..64 {
        for x in 20..44 {
            px[y * 64 + x] = color;
        }
    }
    png::codificar(64, 64, &px, 1)
}

#[test]
fn carga_un_paquete_y_usa_cada_imagen_donde_va() {
    let dir = tempfile::tempdir().unwrap();
    let img = dir.path().join("img");
    std::fs::create_dir_all(&img).unwrap();
    for (n, color) in [
        (1, 0xFF11_0000),
        (2, 0xFF22_0000),
        (3, 0xFF33_0000),
        (4, 0xFF44_0000),
        (5, 0xFF55_0000),
        (11, 0xFF0B_0000),
    ] {
        std::fs::write(img.join(format!("shime{n}.png")), imagen(color)).unwrap();
    }
    let a = Aspecto::shimeji(dir.path()).unwrap().expect("hay shime1.png");
    let rojo = |anim, t: f32| (a.cuadro(anim, t, -1).px[40 * 64 + 30] >> 16) & 0xFF;
    assert_eq!(rojo(Anim::Parada, 0.0), 0x11);
    assert_eq!(rojo(Anim::Caminando, 0.2), 0x22);
    assert_eq!(rojo(Anim::Caminando, 0.5), 0x33);
    assert_eq!(rojo(Anim::Cayendo, 0.0), 0x44);
    assert_eq!(rojo(Anim::Agarrada, 0.0), 0x55);
    // Sin las 31-33 usa la 11; sin la 21 (dormida), la parada.
    assert_eq!(rojo(Anim::Sentada, 0.0), 0x0B);
    assert_eq!(rojo(Anim::Dormida, 0.0), 0x11);
    // Pisa abajo al centro, y a la derecha va al espejo.
    let c = a.cuadro(Anim::Parada, 0.0, 1);
    assert_eq!((c.ancla_x, c.ancla_y), (32, 64));
}

#[test]
fn mascota_json_cambia_la_escala() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("shime1.png"), imagen(0xFF11_0000)).unwrap();
    std::fs::write(dir.path().join("mascota.json"), r#"{"escala": 2}"#).unwrap();
    let a = Aspecto::shimeji(dir.path()).unwrap().unwrap();
    assert_eq!(a.cuadro(Anim::Parada, 0.0, -1).ancho, 128);
}

#[test]
fn sin_paquete_no_hay_shimeji() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Aspecto::shimeji(dir.path()).unwrap().is_none());
}
