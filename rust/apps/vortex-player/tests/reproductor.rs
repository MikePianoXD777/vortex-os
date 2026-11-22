//! El reproductor sin mpv (eventos de mentira y un reloj que se mueve a mano) y, si hay mpv, uno de punta a
//! punta sin sonido (`VORTEX_PLAYER_AO=null`).

use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use vortex_player::reproductor::{Aviso, DeMpv, MpvProceso, Reproductor, canal};

static RELOJ: AtomicU64 = AtomicU64::new(0);
/// Las pruebas que mueven el reloj van una por una (el reloj es de todas).
static TURNO: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn turno() -> std::sync::MutexGuard<'static, ()> {
    TURNO.lock().unwrap_or_else(|e| e.into_inner())
}
fn reloj() -> f64 {
    f64::from_bits(RELOJ.load(Ordering::SeqCst))
}
fn poner_reloj(t: f64) {
    RELOJ.store(t.to_bits(), Ordering::SeqCst);
}

fn cancion(n: &str, dur: f64) -> Value {
    json!({"ruta": format!("/musica/{n}.mp3"), "titulo": n, "duracion": dur})
}

fn nuevo() -> Reproductor {
    let mut r = Reproductor::new();
    r.reloj = reloj;
    r
}

fn cargadas(r: &mut Reproductor) -> Vec<String> {
    r.salida
        .drain(..)
        .filter(|v| v["command"][0] == "loadfile")
        .map(|v| v["command"][1].as_str().unwrap().to_string())
        .collect()
}

fn fin(r: &mut Reproductor) {
    r.al_evento(&json!({"event": "end-file", "reason": "eof"}));
}

fn pos(r: &mut Reproductor, t: f64) {
    r.al_evento(&json!({"event": "property-change", "name": "time-pos", "data": t}));
}

fn titulos(v: &[Value]) -> Vec<&str> {
    v.iter().map(|c| c["titulo"].as_str().unwrap()).collect()
}

#[test]
fn toca_avanza_y_pide_radio_al_final() {
    let mut r = nuevo();
    let lista = [cancion("a", 200.0), cancion("b", 200.0), json!({"titulo": "sin ruta"})];
    r.tocar(&lista, 0);
    assert_eq!(cargadas(&mut r), ["/musica/a.mp3"]);
    assert!(r.avisos.contains(&Aviso::QuiereSonar) && r.avisos.contains(&Aviso::Cancion));
    assert_eq!(r.cola_restantes(), 1); // la que no tiene ruta no entra
    r.avisos.clear();
    fin(&mut r);
    assert_eq!(cargadas(&mut r), ["/musica/b.mp3"]);
    assert!(r.avisos.contains(&Aviso::SeAcaba)); // ya suena la última
    fin(&mut r);
    assert!(r.final_de_cola() && r.pausado());
    // la radio sigue sonando desde la que llega
    r.agregar_radio(&[cancion("radio", 180.0)]);
    assert_eq!(cargadas(&mut r), ["/musica/radio.mp3"]);
    assert!(!r.final_de_cola());
}

#[test]
fn repetir_aleatorio_y_anterior() {
    let mut r = nuevo();
    let lista: Vec<Value> = (0..6).map(|i| cancion(&format!("c{i}"), 100.0)).collect();
    r.alternar_repetir();
    r.alternar_repetir();
    assert_eq!(r.repetir(), "una");
    r.tocar(&lista, 2);
    cargadas(&mut r);
    fin(&mut r);
    assert_eq!(cargadas(&mut r), ["/musica/c2.mp3"]); // la misma
    r.alternar_repetir();
    assert_eq!(r.repetir(), "no");
    r.alternar_aleatorio();
    assert_eq!(r.cancion()["titulo"], "c2"); // la que suena queda primero
    assert_eq!(r.indice(), 0);
    let mut orden = titulos(&r.cola()).into_iter().map(String::from).collect::<Vec<_>>();
    orden.sort();
    assert_eq!(orden, ["c0", "c1", "c2", "c3", "c4", "c5"]);
    r.alternar_aleatorio();
    assert_eq!((r.indice(), r.cancion()["titulo"].clone()), (2, json!("c2")));
    // pasados 3 s, «anterior» vuelve al inicio de la misma
    pos(&mut r, 10.0);
    r.salida.clear();
    r.anterior();
    assert_eq!(r.salida, [json!({"command": ["seek", 0.0, "absolute"]})]);
    pos(&mut r, 1.0);
    r.anterior();
    assert_eq!(cargadas(&mut r), ["/musica/c1.mp3"]);
}

#[test]
fn la_cola_a_continuacion() {
    let mut r = nuevo();
    r.tocar(&[cancion("a", 100.0), cancion("b", 100.0), cancion("c", 100.0)], 0);
    r.tocar_despues(&cancion("x", 100.0));
    r.tocar_despues(&cancion("y", 100.0));
    r.agregar_a_cola(&cancion("z", 100.0));
    assert_eq!(titulos(&r.cola_siguientes()), ["x", "y", "b", "c", "z"]);
    r.quitar_de_cola_en(1); // la x (su lugar en la cola)
    assert_eq!(titulos(&r.cola_siguientes()), ["y", "b", "c", "z"]);
    r.tocar_despues(&cancion("w", 100.0)); // detrás de la y, que sigue «después»
    assert_eq!(titulos(&r.cola_siguientes()), ["y", "w", "b", "c", "z"]);
    r.quitar_de_cola("/musica/c.mp3");
    assert_eq!(titulos(&r.cola()), ["a", "b", "x", "y", "z", "w"]); // como Python: el orden se rehace con la cola
    r.quitar_de_cola_en(0); // la que suena no se quita
    assert_eq!(r.cola().len(), 6);
    r.saltar_a(5);
    assert_eq!(r.cancion()["titulo"], "w");
}

#[test]
fn cuenta_lo_escuchado_y_no_los_saltos() {
    let mut r = nuevo();
    r.tocar(&[cancion("a", 200.0), cancion("b", 20.0)], 0);
    r.al_evento(&json!({"event": "property-change", "name": "pause", "data": false}));
    let mut t = 0.0;
    while t < 29.0 {
        t += 0.5;
        pos(&mut r, t);
    }
    pos(&mut r, 150.0); // un salto: no cuenta
    pos(&mut r, 150.5);
    r.avisos.clear();
    r.siguiente();
    assert!(!r.avisos.iter().any(|a| matches!(a, Aviso::Escuchada(..)))); // 29.5 s < 30
    // la de 20 s cuenta con la mitad
    for k in 1..=21 {
        pos(&mut r, k as f64 * 0.5);
    }
    r.avisos.clear();
    fin(&mut r);
    let e: Vec<_> = r.avisos.iter().filter_map(|a| match a {
        Aviso::Escuchada(c, s) => Some((c["titulo"].clone(), *s)),
        _ => None,
    }).collect();
    assert_eq!(e, [(json!("b"), 10.5)]);
}

#[test]
fn youtube_reintenta_sin_cliente_y_luego_avisa() {
    let _t = turno();
    poner_reloj(100.0);
    let mut r = nuevo();
    let yt = json!({"ruta": "https://music.youtube.com/watch?v=abc", "titulo": "De la radio"});
    r.tocar(&[yt, cancion("b", 100.0)], 0);
    let s = std::mem::take(&mut r.salida);
    assert_eq!(s[0]["command"][4]["ytdl-raw-options"], "extractor-args=youtube:player_client=visionos");
    r.al_evento(&json!({"event": "end-file", "reason": "error"}));
    let s = std::mem::take(&mut r.salida);
    assert_eq!(s[0]["command"], json!(["loadfile", "https://music.youtube.com/watch?v=abc", "replace"]));
    r.avisos.clear();
    r.al_evento(&json!({"event": "end-file", "reason": "error"}));
    assert_eq!(r.avisos, [Aviso::Error("No se pudo tocar: De la radio".into())]);
    r.tic();
    assert!(cargadas(&mut r).is_empty()); // espera un momento
    poner_reloj(100.4);
    r.tic();
    assert_eq!(cargadas(&mut r), ["/musica/b.mp3"]);
}

#[test]
fn una_ruta_rara_no_llega_a_mpv() {
    let mut r = nuevo();
    r.tocar(&[json!({"ruta": "--script=malo.lua", "titulo": "rara"})], 0);
    assert!(cargadas(&mut r).is_empty());
    assert!(r.avisos.contains(&Aviso::Error("No se pudo tocar: rara".into())));
}

#[test]
fn temporizador_para_dormir() {
    let _t = turno();
    poner_reloj(1000.0);
    let mut r = nuevo();
    r.tocar(&[cancion("a", 900.0), cancion("b", 900.0)], 0);
    r.al_evento(&json!({"event": "property-change", "name": "pause", "data": false}));
    r.dormir(1);
    assert!(r.dormir_activo());
    assert_eq!(r.dormir_texto(), "1:00");
    r.salida.clear();
    poner_reloj(1055.0); // faltan 5 s: a la mitad del volumen
    r.tic();
    assert_eq!(r.salida, [json!({"command": ["set_property", "volume", 40]})]);
    r.salida.clear();
    poner_reloj(1061.0);
    r.tic();
    assert_eq!(
        r.salida,
        [
            json!({"command": ["set_property", "pause", true]}),
            json!({"command": ["set_property", "volume", 80]})
        ]
    );
    assert!(!r.dormir_activo());
    // «al terminar la canción»: queda la siguiente, en pausa
    r.dormir(-1);
    assert_eq!(r.dormir_texto(), "Al terminar la canción");
    r.salida.clear();
    fin(&mut r);
    let s = std::mem::take(&mut r.salida);
    assert_eq!(s[0]["command"][1], "/musica/b.mp3");
    assert_eq!(s[1], json!({"command": ["set_property", "pause", true]}));
}

#[test]
fn practicar_velocidad_y_tramo() {
    let mut r = nuevo();
    r.tocar(&[cancion("a", 300.0), cancion("b", 300.0)], 0);
    r.poner_eq("equalizer=f=40:g=3");
    r.poner_velocidad(0.7543);
    assert_eq!(r.velocidad(), 0.75);
    assert_eq!(r.filtros(), "equalizer=f=40:g=3,scaletempo2");
    pos(&mut r, 10.0);
    r.marcar_ab();
    pos(&mut r, 20.0);
    r.marcar_ab();
    assert_eq!((r.ab_a(), r.ab_b()), (10.0, 20.0));
    assert!(r.salida.contains(&json!({"command": ["seek", 10.0, "absolute"]})));
    r.siguiente(); // otra canción: se quita el tramo
    assert_eq!((r.ab_a(), r.ab_b()), (-1.0, -1.0));
    r.salida.clear();
    r.al_conectar(); // mpv se volvió a levantar: los filtros otra vez
    assert_eq!(r.salida, [json!({"command": ["set_property", "af", "equalizer=f=40:g=3,scaletempo2"]})]);
}

#[test]
fn crossfade_lanza_la_que_se_va_aparte() {
    let _t = turno();
    poner_reloj(5000.0);
    let mut r = nuevo();
    r.poner_fundido(5.0);
    r.tocar(&[cancion("a", 100.0), cancion("b", 100.0)], 0);
    r.al_evento(&json!({"event": "property-change", "name": "pause", "data": false}));
    r.al_evento(&json!({"event": "property-change", "name": "duration", "data": 100.0}));
    poner_reloj(5100.0);
    pos(&mut r, 90.0);
    assert!(r.fundidos.is_empty()); // todavía no
    r.salida.clear();
    pos(&mut r, 95.5);
    assert_eq!(r.fundidos.len(), 1);
    let f = &r.fundidos[0];
    assert!(f.contains(&"--start=95.50".to_string()));
    assert!(f.contains(&"--af=lavfi=[afade=t=out:st=95.50:d=4.50]".to_string()));
    assert_eq!(f.last().unwrap(), "/musica/a.mp3");
    let s = std::mem::take(&mut r.salida);
    assert_eq!(
        s[0]["command"],
        json!(["loadfile", "/musica/b.mp3", "replace", -1, {"af": "lavfi=[afade=t=in:d=4.5]"}])
    );
    assert_eq!(r.cancion()["titulo"], "b");
    // en pausa se paran los crossfade
    r.al_evento(&json!({"event": "property-change", "name": "pause", "data": true}));
    assert!(r.parar_fundidos);
}

#[test]
fn seguir_donde_te_quedaste() {
    let mut r = nuevo();
    let lista: Vec<Value> = (0..4).map(|i| cancion(&format!("c{i}"), 100.0)).collect();
    r.alternar_aleatorio();
    r.alternar_repetir();
    r.tocar(&lista, 1);
    r.siguiente();
    pos(&mut r, 42.0);
    r.poner_origen("Mi playlist");
    let s = r.sesion();
    let sonaba = r.cancion();
    let orden = titulos(&r.cola()).into_iter().map(String::from).collect::<Vec<_>>();

    let mut otro = nuevo();
    assert!(otro.restaurar(&lista, &s));
    assert_eq!(otro.cancion(), sonaba);
    assert_eq!(titulos(&otro.cola()), orden);
    assert!(otro.pausado() && otro.aleatorio());
    assert_eq!((otro.repetir(), otro.origen()), ("todo", "Mi playlist"));
    let carga = otro.salida.iter().find(|v| v["command"][0] == "loadfile").unwrap();
    assert_eq!(carga["command"][4]["start"], "42.00");

    // si borraste canciones, el orden se rehace y se sigue en la que estaba (o en la primera)
    let quedan: Vec<Value> = lista.iter().filter(|c| **c != sonaba).cloned().collect();
    let mut tercero = nuevo();
    assert!(tercero.restaurar(&quedan, &s));
    assert_eq!(tercero.indice(), 0);
    assert!(!tercero.restaurar(&[], &s));
}

/// Un WAV de silencio de `segundos`.
fn wav(ruta: &std::path::Path, segundos: f64) {
    let muestras = (8000.0 * segundos) as u32;
    let mut b = Vec::new();
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + muestras * 2).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&8000u32.to_le_bytes());
    b.extend_from_slice(&16000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(muestras * 2).to_le_bytes());
    b.resize(b.len() + muestras as usize * 2, 0);
    std::fs::write(ruta, b).unwrap();
}

#[test]
#[allow(unsafe_code)] // set_var antes de arrancar hilos
fn de_punta_a_punta_con_mpv() {
    if std::process::Command::new("mpv").arg("--version").output().is_err() {
        eprintln!("falta mpv: se salta");
        return;
    }
    unsafe { std::env::set_var("VORTEX_PLAYER_AO", "null") };
    let t = tempfile::tempdir().unwrap();
    let (a, b) = (t.path().join("a.wav"), t.path().join("b.wav"));
    wav(&a, 1.0);
    wav(&b, 1.0);
    let lista = [
        json!({"ruta": a.to_string_lossy(), "titulo": "a"}),
        json!({"ruta": b.to_string_lossy(), "titulo": "b"}),
    ];
    let (enviar, rx) = canal();
    let mut mpv = MpvProceso::arrancar(80, enviar);
    let mut r = Reproductor::new();
    r.tocar(&lista, 0);
    mpv.despachar(&mut r); // se queda pendiente hasta que conecte
    let limite = std::time::Instant::now() + Duration::from_secs(20);
    let mut conecto = false;
    while std::time::Instant::now() < limite && !(r.final_de_cola()) {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(DeMpv::Conectado) => {
                conecto = true;
                r.al_conectar();
            }
            Ok(DeMpv::Evento(ev)) => r.al_evento(&ev),
            Ok(m) => panic!("mpv: {m:?}"),
            Err(_) => {}
        }
        r.tic();
        mpv.despachar(&mut r);
    }
    assert!(conecto);
    assert!(r.final_de_cola(), "no llegó al final: {:?}", r.cancion());
    assert_eq!(r.cancion()["titulo"], "b");
    mpv.cerrar();
}

/// La barra de tiempo: adelantar a un punto (buscar) tiene que llevar a mpv ahí, sonando o en pausa.
#[test]
#[allow(unsafe_code)] // set_var antes de arrancar hilos
fn buscar_lleva_a_mpv_al_punto() {
    if std::process::Command::new("mpv").arg("--version").output().is_err() {
        eprintln!("falta mpv: se salta");
        return;
    }
    unsafe { std::env::set_var("VORTEX_PLAYER_AO", "null") };
    let t = tempfile::tempdir().unwrap();
    let a = t.path().join("larga.wav");
    wav(&a, 30.0);
    let lista = [json!({"ruta": a.to_string_lossy(), "titulo": "larga", "duracion": 30.0})];
    let (enviar, rx) = canal();
    let mut mpv = MpvProceso::arrancar(80, enviar);
    let mut r = Reproductor::new();
    r.tocar(&lista, 0);
    mpv.despachar(&mut r);
    let mut esperar = |r: &mut Reproductor, mpv: &mut MpvProceso, hasta: &dyn Fn(&Reproductor) -> bool| {
        let limite = std::time::Instant::now() + Duration::from_secs(15);
        while std::time::Instant::now() < limite && !hasta(r) {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(DeMpv::Conectado) => r.al_conectar(),
                Ok(DeMpv::Evento(ev)) => r.al_evento(&ev),
                Ok(_) | Err(_) => {}
            }
            r.tic();
            mpv.despachar(r);
        }
        hasta(r)
    };
    assert!(esperar(&mut r, &mut mpv, &|r| r.posicion() > 0.3), "no empezó a sonar");
    r.buscar(20.0);
    mpv.despachar(&mut r);
    // Que mpv lo confirme: la posición que reporta pasa de 20 (y no regresa al principio).
    std::thread::sleep(Duration::from_millis(600));
    assert!(esperar(&mut r, &mut mpv, &|r| r.posicion() >= 20.0 && r.posicion() < 29.0), "sonando: quedó en {}", r.posicion());
    // En pausa también.
    r.pausar();
    mpv.despachar(&mut r);
    r.buscar(5.0);
    mpv.despachar(&mut r);
    std::thread::sleep(Duration::from_millis(600));
    assert!(esperar(&mut r, &mut mpv, &|r| (r.posicion() - 5.0).abs() < 1.0), "en pausa: quedó en {}", r.posicion());
    mpv.cerrar();
}
