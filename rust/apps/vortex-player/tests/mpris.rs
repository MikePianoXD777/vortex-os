//! MPRIS en un bus de sesión propio (un `dbus-daemon` en una carpeta temporal): no toca el escritorio de quien
//! las corre ni su panel de música.

use serde_json::json;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use vortex_player::mpris::{EstadoMpris, Meta, Mpris, NOMBRE, OrdenMpris, RUTA, metadatos};
use zbus::OwnedVariant as OwnedValue;

#[test]
fn los_metadatos() {
    let c = json!({"ruta": "/música/Bohemia [dQw4w9WgXcQ].opus", "titulo": "Bohemia", "artista": "Queen, David Bowie",
                   "album": "Opera", "duracion": 354.5});
    let m = metadatos(&c, "file:///tmp/p.jpg");
    let d = |k: &str| m.iter().find(|(a, _)| *a == k).map(|(_, v)| v.clone()).unwrap();
    assert!(matches!(d("mpris:trackid"), Meta::Ruta(r) if r.starts_with("/org/vortex/player/cancion/")));
    assert_eq!(d("mpris:length"), Meta::Entero(354_500_000));
    assert_eq!(d("xesam:artist"), Meta::Lista(vec!["Queen".into(), "David Bowie".into()]));
    assert_eq!(d("xesam:url"), Meta::Texto("file:///m%C3%BAsica/Bohemia%20%5BdQw4w9WgXcQ%5D.opus".into()));
    assert_eq!(d("mpris:artUrl"), Meta::Texto("file:///tmp/p.jpg".into()));
    assert_eq!(metadatos(&c, "")[0], m[0]); // el mismo id para la misma canción
    let sin = metadatos(&json!({"ruta": "/m/mi canción.flac"}), "");
    assert!(sin.iter().any(|(k, v)| *k == "xesam:title" && *v == Meta::Texto("mi canción".into())));
    assert!(sin.iter().any(|(k, v)| *k == "xesam:artist" && *v == Meta::Lista(vec![String::new()])));
    assert!(!sin.iter().any(|(k, _)| *k == "mpris:artUrl"));
    assert!(metadatos(&json!({}), "").is_empty());
}

#[test]
fn en_un_bus_propio() {
    let t = tempfile::tempdir().unwrap();
    let sock = t.path().join("bus");
    let Ok(mut d) = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--print-address=1", &format!("--address=unix:path={}", sock.display())])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        eprintln!("falta dbus-daemon: se salta");
        return;
    };
    let mut dir = String::new();
    BufReader::new(d.stdout.take().unwrap()).read_line(&mut dir).unwrap();
    let dir = dir.trim().to_string();

    let (tx, rx) = mpsc::channel();
    let m = Mpris::arrancar(Some(&dir), tx).expect("se registra en el bus");
    m.poner(EstadoMpris {
        cancion: json!({"ruta": "/m/a.flac", "titulo": "Uno", "duracion": 100.0}),
        pausado: false,
        repetir: "todo".into(),
        volumen: 80,
        posicion: 10.0,
        hay_siguiente: true,
        hay_anterior: true,
        ..EstadoMpris::default()
    });

    async_io::block_on(async {
        let c = zbus::connection::Builder::address(dir.as_str()).build().await.unwrap();
        let p = zbus::Proxy::new(&c, NOMBRE, RUTA, "org.mpris.MediaPlayer2.Player").await.unwrap();
        assert_eq!(p.get_property::<String>("PlaybackStatus").await.unwrap(), "Playing");
        assert_eq!(p.get_property::<String>("LoopStatus").await.unwrap(), "Playlist");
        assert_eq!(p.get_property::<f64>("Volume").await.unwrap(), 0.8);
        assert_eq!(p.get_property::<i64>("Position").await.unwrap(), 10_000_000);
        let meta: std::collections::HashMap<String, OwnedValue> = p.get_property("Metadata").await.unwrap();
        assert_eq!(String::try_from(meta["xesam:title"].clone()).unwrap(), "Uno");
        p.call_method("Next", &()).await.unwrap();
        p.call_method("PlayPause", &()).await.unwrap();
        p.call_method("Seek", &(5_000_000i64,)).await.unwrap();
        p.set_property("Volume", 0.5f64).await.unwrap();
        p.set_property("Shuffle", true).await.unwrap();
        p.set_property("LoopStatus", "Track").await.unwrap();
        let r = zbus::Proxy::new(&c, NOMBRE, RUTA, "org.mpris.MediaPlayer2").await.unwrap();
        assert_eq!(r.get_property::<String>("Identity").await.unwrap(), "Vortex Player");
        r.call_method("Raise", &()).await.unwrap();
    });
    let llegaron: Vec<OrdenMpris> = (0..7).filter_map(|_| rx.recv_timeout(Duration::from_secs(3)).ok()).collect();
    assert_eq!(
        llegaron,
        [
            OrdenMpris::Siguiente,
            OrdenMpris::Alternar,
            OrdenMpris::Buscar(15.0),
            OrdenMpris::Volumen(50),
            OrdenMpris::Aleatorio(true),
            OrdenMpris::Repetir("una".into()),
            OrdenMpris::Levantar
        ]
    );
    drop(m);
    let _ = d.kill();
    let _ = d.wait();
}
