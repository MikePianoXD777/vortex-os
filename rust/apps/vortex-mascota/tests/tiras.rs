//! El cargador de paquetes de tiras (animations.json + spritesheets_128/) con tiras de mentira: cada
//! cuadro es un rectángulo de 16×16 con su propio rojo, y el de `walk_side` está cargado a la derecha.
use vortex_mascota::{aspecto::Aspecto, mundo::Anim, png};

const C: usize = 16;

/// Una tira de `n` cuadros; el cuadro k tiene rojo `base + k`.
fn tira(n: usize, base: u32, a_la_derecha: bool) -> Vec<u8> {
    let mut px = vec![0u32; n * C * C];
    for k in 0..n {
        let (x0, x1) = if a_la_derecha { (9, 15) } else { (4, 12) };
        for y in 4..C {
            for x in x0..x1 {
                px[y * n * C + k * C + x] = 0xFF00_0000 | (base + k as u32) << 16;
            }
        }
    }
    png::codificar(n * C, C, &px, 1)
}

fn paquete(dir: &std::path::Path) {
    let tiras = dir.join("spritesheets_128");
    std::fs::create_dir_all(&tiras).unwrap();
    let anims = [
        ("idle", 4, 0x10, false),
        ("blink", 2, 0x20, false),
        ("walk_side", 6, 0x30, true),
        ("sleep_sit", 4, 0x40, false),
        ("headpat", 4, 0x50, false),
        ("jump", 5, 0x60, false),
        ("coffee", 3, 0x70, false),
        ("phone", 2, 0x80, false),
    ];
    let mut json = String::from(r#"{"anchor": "pies en la fila inferior", "animations": {"#);
    for (i, (nombre, n, base, derecha)) in anims.iter().enumerate() {
        std::fs::write(tiras.join(format!("{nombre}.png")), tira(*n, *base, *derecha)).unwrap();
        let ms = vec!["100"; *n].join(",");
        json += &format!(
            r#"{}"{nombre}": {{"frames": {n}, "durations_ms": [{ms}], "loop": true}}"#,
            if i > 0 { "," } else { "" }
        );
    }
    // Una que nombra el json pero no trae tira: se ignora.
    json += r#","dance": {"frames": 4, "durations_ms": [100,100,100,100]}}}"#;
    std::fs::write(dir.join("animations.json"), json).unwrap();
}

fn rojo(a: &Aspecto, anim: Anim, t: f32, dir: i8) -> u32 {
    let c = a.cuadro(anim, t, dir);
    c.px.iter().map(|p| (p >> 16) & 0xFF).find(|&r| r != 0).unwrap()
}

#[test]
fn cada_animacion_usa_su_tira_y_sus_tiempos() {
    let dir = tempfile::tempdir().unwrap();
    paquete(dir.path());
    let a = Aspecto::tiras(dir.path()).unwrap().expect("hay animations.json");
    // Parada: idle tres veces (0,1 s cada cuadro) y luego el parpadeo.
    assert_eq!(rojo(&a, Anim::Parada, 0.05, 1), 0x10);
    assert_eq!(rojo(&a, Anim::Parada, 0.15, 1), 0x11);
    assert_eq!(rojo(&a, Anim::Parada, 1.25, 1), 0x20);
    assert_eq!(rojo(&a, Anim::Parada, 1.35, 1), 0x21);
    assert_eq!(rojo(&a, Anim::Dormida, 0.0, 1), 0x40);
    assert_eq!(rojo(&a, Anim::Caricia, 0.25, 1), 0x52);
    // Al aterrizar, del cuarto cuadro del salto en adelante.
    assert_eq!(rojo(&a, Anim::Aterrizando, 0.0, 1), 0x63);
    // Lo que no trae: saludar y despedirse usan la parada; agarrada también.
    assert_eq!(rojo(&a, Anim::Despedida, 0.0, 1), 0x10);
    assert_eq!(rojo(&a, Anim::Agarrada, 0.0, 1), 0x10);
    // Pisa en la fila de abajo, al centro.
    let c = a.cuadro(Anim::Parada, 0.0, 1);
    assert_eq!((c.ancla_x, c.ancla_y, c.alto), (C / 2, C, C));
    // Dos pasatiempos, en el orden de OCIOS (phone y coffee; dance no tiene tira).
    assert_eq!(a.ocios(), 2);
    assert_eq!(rojo(&a, Anim::Ocio(0), 0.0, 1), 0x80);
    assert_eq!(rojo(&a, Anim::Ocio(1), 0.0, 1), 0x70);
}

#[test]
fn camina_volteando_y_lo_de_frente_no_se_voltea() {
    let dir = tempfile::tempdir().unwrap();
    paquete(dir.path());
    let a = Aspecto::tiras(dir.path()).unwrap().unwrap();
    // walk_side mira a la derecha: a la derecha va tal cual, a la izquierda al espejo.
    assert!(a.cuadro(Anim::Caminando, 0.0, 1).caja.0 >= 8);
    assert!(a.cuadro(Anim::Caminando, 0.0, -1).caja.2 <= 8);
    // idle es de frente: igual en las dos direcciones.
    assert_eq!(
        a.cuadro(Anim::Parada, 0.0, 1).px,
        a.cuadro(Anim::Parada, 0.0, -1).px
    );
}

#[test]
fn la_zeta_y_los_corazones_no_se_repiten() {
    use vortex_mascota::arte::Emote;
    let dir = tempfile::tempdir().unwrap();
    paquete(dir.path());
    let a = Aspecto::tiras(dir.path()).unwrap().unwrap();
    assert_eq!(a.emote(Anim::Dormida, Some(Emote::Zeta)), None);
    assert_eq!(a.emote(Anim::Caricia, Some(Emote::Corazon)), None);
    assert_eq!(a.emote(Anim::Sentada, Some(Emote::Nota)), Some(Emote::Nota));
    // La original sí los dibuja.
    let o = Aspecto::original(3.0);
    assert_eq!(o.emote(Anim::Dormida, Some(Emote::Zeta)), Some(Emote::Zeta));
}

#[test]
fn sin_idle_no_sirve_y_sin_json_no_es_de_tiras() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Aspecto::tiras(dir.path()).unwrap().is_none());
    std::fs::create_dir_all(dir.path().join("spritesheets_128")).unwrap();
    std::fs::write(dir.path().join("animations.json"), r#"{"animations": {}}"#).unwrap();
    assert!(Aspecto::tiras(dir.path()).is_err());
}
