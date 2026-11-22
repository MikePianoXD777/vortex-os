//! MPRIS en un bus de sesión propio (un `dbus-daemon` en una carpeta temporal): no toca el escritorio de quien
//! las corre ni su panel de música.

use serde_json::json;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use vortex_player::mpris::{EstadoMpris, Meta, Mpris, NOMBRE, OrdenMpris, RUTA, brinco, metadatos};
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

/// Vortex Player se abre sin canción (CanSeek = false) y luego suena una: tiene que avisar que ya se puede
/// adelantar. Si no, el panel de música se quedaba con CanSeek = false y su barra no dejaba elegir el tiempo
/// (Quickshell: «Cannot set position … because canSeek is false»). Y si la canción no trae su duración, va la
/// de mpv (sin duración la barra del panel no tiene a dónde moverse).
#[test]
fn avisa_que_ya_se_puede_adelantar() {
    use futures_util::StreamExt;
    let t = tempfile::tempdir().unwrap();
    let sock = t.path().join("bus");
    let Ok(mut d) = Command::new("dbus-daemon")
        .args([
            "--session",
            "--nofork",
            "--print-address=1",
            &format!("--address=unix:path={}", sock.display()),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        eprintln!("falta dbus-daemon: se salta");
        return;
    };
    let mut dir = String::new();
    BufReader::new(d.stdout.take().unwrap())
        .read_line(&mut dir)
        .unwrap();
    let dir = dir.trim().to_string();
    let (tx, _rx) = mpsc::channel();
    let m = Mpris::arrancar(Some(&dir), tx).expect("se registra en el bus");

    async_io::block_on(async {
        let c = zbus::connection::Builder::address(dir.as_str())
            .build()
            .await
            .unwrap();
        let p = zbus::Proxy::new(&c, NOMBRE, RUTA, "org.mpris.MediaPlayer2.Player")
            .await
            .unwrap();
        assert!(
            !p.get_property::<bool>("CanSeek").await.unwrap(),
            "sin canción no se puede adelantar"
        );
        let mut cambios = p.receive_property_changed::<bool>("CanSeek").await;
        let mut metas = p
            .receive_property_changed::<std::collections::HashMap<String, OwnedValue>>("Metadata")
            .await;
        m.poner(EstadoMpris {
            cancion: json!({"ruta": "/m/a.flac", "titulo": "Uno"}),
            duracion: 200.5,
            ..EstadoMpris::default()
        });
        // El primero puede ser el valor de antes (false): se espera hasta que diga true.
        let hasta_true = async {
            while let Some(a) = cambios.next().await {
                if a.get().await.unwrap() {
                    return true;
                }
            }
            false
        };
        let espera = async {
            async_io::Timer::after(Duration::from_secs(3)).await;
            false
        };
        let aviso = match futures_util::future::select(Box::pin(hasta_true), Box::pin(espera)).await {
            futures_util::future::Either::Left((a, _)) | futures_util::future::Either::Right((a, _)) => a,
        };
        assert!(aviso, "avisó que ya se puede adelantar");
        // La duración: la de mpv, porque la canción no traía.
        let mut meta = None;
        for _ in 0..3 {
            let Some(cambio) = metas.next().await else { break };
            let v = cambio.get().await.unwrap();
            if v.contains_key("mpris:length") {
                meta = Some(v);
                break;
            }
        }
        let meta = meta.expect("llegaron los metadatos");
        assert_eq!(i64::try_from(meta["mpris:length"].clone()).unwrap(), 200_500_000);
    });
    drop(m);
    let _ = d.kill();
    let _ = d.wait();
}

/// Cuándo la posición «brincó»: tocando avanza con el reloj; en pausa no; un salto de más de segundo y medio sí.
#[test]
fn sabe_cuando_la_posicion_brinco() {
    let c = json!({"ruta": "/m/a.flac"});
    let en = |p: f64, pausado: bool| EstadoMpris { cancion: c.clone(), posicion: p, pausado, ..EstadoMpris::default() };
    assert!(!brinco(&en(10.0, false), &en(10.25, false), 0.25), "tocando normal");
    assert!(!brinco(&en(10.0, false), &en(11.0, false), 0.2), "un segundo de diferencia todavía no");
    assert!(brinco(&en(7.0, false), &en(151.0, false), 0.25), "se adelantó desde Vortex Player");
    assert!(brinco(&en(90.0, false), &en(3.0, false), 0.25), "regresó");
    assert!(!brinco(&en(30.0, true), &en(30.0, true), 60.0), "en pausa no avanza");
    assert!(brinco(&en(30.0, true), &en(95.0, true), 60.0), "en pausa y se movió");
    let sin = EstadoMpris::default();
    assert!(!brinco(&en(30.0, false), &sin, 0.2), "sin canción no hay nada que avisar");
}

/// Si la canción brinca (se adelantó desde Vortex Player o arrancó donde se había quedado), MPRIS lo avisa con
/// `Seeked`: si no, el panel de música se queda en otro tiempo (le pasó a Mike: 00:07 en el panel y 2:31 en la app).
#[test]
fn avisa_cuando_la_cancion_brinca() {
    use futures_util::StreamExt;
    let t = tempfile::tempdir().unwrap();
    let sock = t.path().join("bus");
    let Ok(mut d) = Command::new("dbus-daemon")
        .args([
            "--session",
            "--nofork",
            "--print-address=1",
            &format!("--address=unix:path={}", sock.display()),
        ])
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
    let (tx, _rx) = mpsc::channel();
    let m = Mpris::arrancar(Some(&dir), tx).expect("se registra en el bus");
    let cancion = json!({"ruta": "/m/a.flac", "titulo": "Uno", "duracion": 164.0});
    m.poner(EstadoMpris { cancion: cancion.clone(), posicion: 7.0, ..EstadoMpris::default() });

    async_io::block_on(async {
        let c = zbus::connection::Builder::address(dir.as_str()).build().await.unwrap();
        let p = zbus::Proxy::new(&c, NOMBRE, RUTA, "org.mpris.MediaPlayer2.Player").await.unwrap();
        let mut saltos = p.receive_signal("Seeked").await.unwrap();
        m.poner(EstadoMpris { cancion: cancion.clone(), posicion: 7.2, ..EstadoMpris::default() });
        m.poner(EstadoMpris { cancion: cancion.clone(), posicion: 151.0, ..EstadoMpris::default() });
        let llega = async {
            let s = saltos.next().await?;
            s.body().deserialize::<i64>().ok()
        };
        let espera = async {
            async_io::Timer::after(Duration::from_secs(3)).await;
            None
        };
        let us = match futures_util::future::select(Box::pin(llega), Box::pin(espera)).await {
            futures_util::future::Either::Left((a, _)) | futures_util::future::Either::Right((a, _)) => a,
        };
        assert_eq!(us, Some(151_000_000), "avisó el brinco (y no el avance normal)");
    });
    drop(m);
    let _ = d.kill();
    let _ = d.wait();
}
