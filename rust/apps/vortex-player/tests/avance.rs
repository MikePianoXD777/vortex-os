//! El avance de `test_interfaz.py` con un mpv de mentira: anota sus argumentos, «abre el audio» y suena 1 s.
//! Si existe el archivo `falla`, el cliente visionos falla y tiene que intentarlo otra vez sin él.

use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use vortex_player::avance::{Avance, DeAvance};

fn esperar(mut cond: impl FnMut() -> bool, ms: u64) -> bool {
    let fin = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < fin {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    cond()
}

#[test]
fn suena_termina_se_para_y_reintenta_sin_cliente() {
    let t = tempfile::tempdir().unwrap();
    let log = t.path().join("avance.log");
    let falla = t.path().join("falla");
    let falso = t.path().join("mpv-avance");
    std::fs::write(
        &falso,
        format!(
            "#!/bin/sh\necho \"$*\" >>{}\ncase \"$*\" in *visionos*) [ -e {} ] && exit 2 ;; esac\nsleep 0.3; echo 'AO: [null] 48000Hz'; sleep 1\n",
            log.display(),
            falla.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&falso, std::fs::Permissions::from_mode(0o755)).unwrap();
    let avisos: Arc<Mutex<Vec<DeAvance>>> = Arc::default();
    let a2 = avisos.clone();
    let mut av = Avance::new(move |m| a2.lock().unwrap().push(m));
    av.programa = Some(falso.to_string_lossy().into_owned());
    let hubo = |m: &DeAvance| avisos.lock().unwrap().contains(m);

    av.tocar("recoaaaaaaa", "https://music.youtube.com/watch?v=recoaaaaaaa", 200.0, 80);
    assert!(hubo(&DeAvance::Empezo));
    assert_eq!((av.id_actual().as_str(), av.sonando()), ("recoaaaaaaa", false));
    assert!(esperar(|| av.sonando(), 3000));
    let args = std::fs::read_to_string(&log).unwrap();
    assert!(args.contains("--start=70 --end=100"), "{args}"); // desde el 35 %, 30 s
    assert!(args.contains("player_client=visionos") && args.contains("watch?v=recoaaaaaaa"));
    assert!(esperar(|| hubo(&DeAvance::Termino), 3000));
    assert_eq!(av.id_actual(), "");

    // otra vez, y lo paras
    avisos.lock().unwrap().clear();
    av.tocar("recoaaaaaaa", "https://music.youtube.com/watch?v=recoaaaaaaa", 60.0, 80);
    assert!(esperar(|| av.sonando(), 3000));
    av.parar();
    assert!(esperar(|| hubo(&DeAvance::Termino), 3000));
    assert!(!hubo(&DeAvance::Error("No pude escuchar el avance".into())));
    assert!(std::fs::read_to_string(&log).unwrap().contains("--start=0 --end=30"));

    // el cliente falla: se intenta sin él
    std::fs::write(&falla, "").unwrap();
    std::fs::write(&log, "").unwrap();
    avisos.lock().unwrap().clear();
    av.tocar("otrooooooo1", "https://music.youtube.com/watch?v=otrooooooo1", 200.0, 80);
    assert!(esperar(|| av.sonando(), 3000));
    let intentos: Vec<String> = std::fs::read_to_string(&log).unwrap().lines().map(String::from).collect();
    assert_eq!(intentos.len(), 2);
    assert!(intentos[0].contains("visionos") && !intentos[1].contains("visionos"));

    // cambiar de avance no suelta tu música en medio (no hay Termino del viejo)
    avisos.lock().unwrap().clear();
    av.tocar("terceroooo1", "https://music.youtube.com/watch?v=terceroooo1", 200.0, 80);
    assert!(!hubo(&DeAvance::Empezo));
    std::thread::sleep(Duration::from_millis(200));
    assert!(!hubo(&DeAvance::Termino));
    assert_eq!(av.id_actual(), "terceroooo1");
    av.parar();
    assert!(esperar(|| hubo(&DeAvance::Termino), 3000));

    // sin mpv: avisa
    avisos.lock().unwrap().clear();
    av.programa = Some(t.path().join("no-hay").to_string_lossy().into_owned());
    av.tocar("x", "https://music.youtube.com/watch?v=x", 10.0, 80);
    assert!(esperar(|| hubo(&DeAvance::Termino), 3000));
    assert!(hubo(&DeAvance::Error("No pude escuchar el avance".into())));
}
