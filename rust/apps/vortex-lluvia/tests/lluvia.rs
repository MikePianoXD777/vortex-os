//! La lluvia sin terminal: cuánto dura, qué dibuja y cuándo no corre.

use vortex_lluvia::{Celda, LLUEVE, Lluvia, TOPE, debe_correr};

fn correr(l: &mut Lluvia, hasta: f32) {
    while l.tiempo() < hasta && !l.terminada() {
        l.avanzar(1.0 / 120.0);
    }
}

#[test]
fn termina_sola_antes_del_tope() {
    for semilla in [1, 42, 2026, 987_654_321] {
        let mut l = Lluvia::new(120, 40, semilla);
        let mut cuadros = 0;
        while !l.terminada() {
            l.avanzar(1.0 / 120.0);
            l.dibujar();
            cuadros += 1;
            assert!(cuadros < 1000, "no terminó");
        }
        assert!(l.tiempo() >= LLUEVE, "terminó antes de llover: {}", l.tiempo());
        assert!(l.tiempo() <= TOPE + 0.01, "se pasó del tope: {}", l.tiempo());
    }
}

#[test]
fn llena_la_pantalla_a_media_lluvia() {
    let mut l = Lluvia::new(100, 30, 7);
    correr(&mut l, 0.5);
    let r = l.rejilla();
    let llenas = r.iter().filter(|c| c.ch != b' ').count();
    assert!(llenas > r.len() / 5, "casi vacía: {llenas} de {}", r.len());
    // Casi todas las columnas tienen algo.
    let cols_con_algo = (0..100)
        .filter(|&c| (0..30).any(|f| r[f * 100 + c].ch != b' '))
        .count();
    assert!(cols_con_algo > 70, "solo {cols_con_algo} columnas");
}

#[test]
fn solo_digitos_y_la_cabeza_es_blanca() {
    let mut l = Lluvia::new(80, 24, 3);
    correr(&mut l, 0.4);
    let r = l.rejilla();
    assert!(r.iter().all(|c| c.ch == b' ' || c.ch.is_ascii_digit()));
    assert!(
        r.iter().all(|c| (c.ch == b' ') == (c.luz == 0)),
        "espacio con luz o dígito apagado"
    );
    assert!(r.iter().any(|c| c.luz == 255), "ninguna cabeza blanca");
    assert!(
        r.iter().filter(|c| c.ch != b' ').all(|c| c.luz >= 40),
        "dígitos demasiado oscuros"
    );
}

#[test]
fn al_final_la_pantalla_queda_vacia() {
    let mut l = Lluvia::new(60, 20, 11);
    while !l.terminada() {
        l.avanzar(1.0 / 60.0);
    }
    if l.tiempo() < TOPE {
        assert!(l.rejilla().iter().all(|c| *c == Celda::VACIA));
    }
}

#[test]
fn dibujar_solo_manda_lo_que_cambio() {
    let mut l = Lluvia::new(40, 12, 5);
    correr(&mut l, 0.3);
    let primero = l.dibujar();
    assert!(!primero.is_empty());
    assert!(primero.contains("\x1b["), "sin códigos ANSI");
    // Sin avanzar, el siguiente cuadro no tiene nada que mandar.
    assert_eq!(l.dibujar(), "");
}

#[test]
fn colores_en_grises() {
    let mut l = Lluvia::new(50, 15, 9);
    correr(&mut l, 0.3);
    let s = l.dibujar();
    // Cada color es gris (r = g = b): la paleta de fábrica es negro, blanco y grises.
    for trozo in s.split("38;2;").skip(1) {
        let nums: Vec<&str> = trozo.split(['m']).next().unwrap().split(';').collect();
        assert_eq!(nums.len(), 3, "{trozo}");
        assert!(
            nums[0] == nums[1] && nums[1] == nums[2],
            "color no gris: {nums:?}"
        );
    }
}

#[test]
fn pantallas_raras_no_truenan() {
    for (a, h) in [(1, 1), (0, 0), (300, 2), (2, 120)] {
        let mut l = Lluvia::new(a, h, 1);
        for _ in 0..300 {
            l.avanzar(0.016);
            l.dibujar();
        }
        assert!(l.terminada());
    }
}

#[test]
fn cuando_no_corre() {
    assert!(debe_correr(true, true, 80, 24, false));
    assert!(
        !debe_correr(false, true, 80, 24, false),
        "salida a un archivo o a un tubo"
    );
    assert!(!debe_correr(true, false, 80, 24, false), "sin teclado");
    assert!(!debe_correr(true, true, 80, 24, true), "apagada por la persona");
    assert!(!debe_correr(true, true, 19, 24, false), "muy angosta");
    assert!(!debe_correr(true, true, 80, 7, false), "muy bajita");
}
