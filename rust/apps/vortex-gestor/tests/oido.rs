//! El oído de Hyprland (lo que probaba `pruebas/gestor.sh` del ayudante en C): ventanas abiertas al
//! arrancar, foco inicial, antirrebote, abre y cierra, pid 0 sin ventana, aviso de presión registrado en el
//! kernel y salida cuando Hyprland se cierra. Con un Hyprland falso (sockets UNIX en una carpeta temporal).

mod comun;

use comun::FalsoHypr;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};
use vortex_gestor::hyprland::{Evento, Oido, Vuelta};

fn abre(dir: &str, tipo: &str, clase: &str) -> Evento {
    Evento::Abre {
        dir: dir.into(),
        tipo: tipo.into(),
        clase: clase.into(),
    }
}

fn foco(pid: u32, dir: &str, tipo: &str, clase: &str) -> Evento {
    Evento::Foco {
        pid,
        dir: dir.into(),
        tipo: tipo.into(),
        clase: clase.into(),
    }
}

/// Espera hasta `seg` segundos y junta los eventos que lleguen hasta que `basta` diga que ya.
fn juntar(
    o: &mut Oido,
    mudo: &UnixStream,
    seg: u64,
    basta: impl Fn(&[Evento]) -> bool,
) -> (Vec<Evento>, bool) {
    let fin = Instant::now() + Duration::from_secs(seg);
    let mut todos = Vec::new();
    while Instant::now() < fin {
        match o
            .esperar(mudo, Some(Instant::now() + Duration::from_millis(100)))
            .0
        {
            // la presión de memoria de verdad (el kernel de esta PC) puede avisar en cualquier momento
            Vuelta::Eventos(e) => todos.extend(e.into_iter().filter(|e| *e != Evento::PsiMemoria)),
            Vuelta::Cerrado => return (todos, true),
            Vuelta::Nada => {}
        }
        if basta(&todos) {
            break;
        }
    }
    (todos, false)
}

#[test]
fn oido_de_hyprland() {
    let t = tempfile::tempdir().unwrap();
    let hd = t.path().join("hypr");
    let h = FalsoHypr::nuevo(&hd);
    *h.clientes.lock().unwrap() = r#"[{
    "address": "0xaaa1",
    "class": "kitty",
    "contentType": "none",
    "grouped": ["0xaaa1"],
    "swallowing": "0x0"
},{
    "address": "0xeee5",
    "class": "steam_app_1",
    "contentType": "game"
}]"#
    .into();
    h.activa(4001, "aaa1", "kitty", "none");
    let (_mudo_a, mudo) = UnixStream::pair().unwrap(); // un descriptor que nunca despierta (las señales)
    let (mut o, arranque) = Oido::conectar(
        &hd.join(".socket2.sock"),
        &hd.join(".socket.sock"),
        Some(Path::new("/proc/pressure/memory")),
        150_000,
        2_000_000,
        80,
        None,
    )
    .unwrap();
    // el aviso de presión se registra en el kernel de verdad (si el kernel tiene PSI y deja)
    let psi_no: Vec<_> = arranque
        .iter()
        .filter(|e| matches!(e, Evento::PsiNo(_)))
        .collect();
    if !Path::new("/proc/pressure/memory").exists() {
        eprintln!("sin PSI en este kernel: {psi_no:?}");
    } else {
        assert!(
            psi_no.is_empty(),
            "aviso de presión registrado en el kernel: {psi_no:?}"
        );
    }
    let abiertas: Vec<_> = arranque
        .iter()
        .filter(|e| matches!(e, Evento::Abre { .. }))
        .cloned()
        .collect();
    assert_eq!(
        abiertas,
        [abre("aaa1", "none", "kitty"), abre("eee5", "game", "steam_app_1")],
        "ventanas abiertas al arrancar"
    );
    let (ev, _) = juntar(&mut o, &mudo, 5, |e| {
        e.iter().any(|e| matches!(e, Evento::Foco { .. }))
    });
    assert_eq!(
        ev,
        [foco(4001, "aaa1", "none", "kitty")],
        "foco inicial con pid, dirección y clase"
    );

    // Mismo foco (cambio de título): no se pregunta nada.
    h.evento(&["activewindowv2>>aaa1", "windowtitlev2>>aaa1,otro"]);
    let (ev, _) = juntar(&mut o, &mudo, 1, |_| false);
    assert!(ev.is_empty(), "el mismo foco no genera nada: {ev:?}");

    // Foco que pasa rápido por una ventana: solo cuenta donde se queda.
    h.activa(5001, "bbb2", "firefox", "none");
    h.evento(&["activewindowv2>>ccc3", "activewindowv2>>bbb2"]);
    let (ev, _) = juntar(&mut o, &mudo, 1, |_| false);
    assert_eq!(
        ev,
        [foco(5001, "bbb2", "none", "firefox")],
        "antirrebote: un solo foco nuevo"
    );

    // abre y cierra
    h.evento(&["openwindow>>ddd4,2,steam_app_570,Dota 2", "closewindow>>ddd4"]);
    let (ev, _) = juntar(&mut o, &mudo, 2, |e| e.len() >= 2);
    assert_eq!(
        ev,
        [
            abre("ddd4", "", "steam_app_570"),
            Evento::Cierra { dir: "ddd4".into() }
        ],
        "abre y cierra"
    );

    // Se cierra la enfocada y no queda ninguna.
    h.sin_ventana();
    h.evento(&["closewindow>>bbb2", "activewindowv2>>"]);
    let (ev, _) = juntar(&mut o, &mudo, 3, |e| {
        e.iter().any(|e| matches!(e, Evento::Foco { .. }))
    });
    assert_eq!(
        ev,
        [Evento::Cierra { dir: "bbb2".into() }, foco(0, "", "", "")],
        "se cierra la enfocada y no queda ninguna: pid 0"
    );

    // Hyprland se cierra: el oído lo sabe.
    h.cerrar();
    let (_, cerrado) = juntar(&mut o, &mudo, 3, |_| false);
    assert!(cerrado, "termina cuando Hyprland se cierra");
}

#[test]
fn sin_hyprland_no_conecta() {
    let t = tempfile::tempdir().unwrap();
    let falso = t.path().join("no-existe.sock");
    assert!(Oido::conectar(&falso, &falso, None, 150_000, 2_000_000, 120, None).is_err());
}
