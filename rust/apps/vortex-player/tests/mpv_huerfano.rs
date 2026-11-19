//! 4.0: si el Player muere de golpe (SIGKILL), su mpv no se queda huérfano. Esta prueba se lanza a sí misma
//! como «Player» (con `VORTEX_PRUEBA_HIJO`), que arranca un mpv falso y espera; el padre la mata con SIGKILL y
//! revisa que el mpv falso también se haya muerto.
#![cfg(target_os = "linux")]

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use vortex_player::reproductor::MpvProceso;

fn vivo(pid: i32) -> bool {
    // Un zombi (ya muerto, esperando a que lo recojan) cuenta como muerto.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map(|s| !s.split(") ").nth(1).is_some_and(|r| r.starts_with('Z')))
        .unwrap_or(false)
}

#[test]
fn mpv_muere_con_el_player() {
    if let Ok(carpeta) = std::env::var("VORTEX_PRUEBA_HIJO") {
        // Soy el «Player»: arranco el mpv falso y me quedo esperando a que me maten.
        let falso = Path::new(&carpeta).join("mpv");
        std::fs::write(
            &falso,
            format!("#!/bin/sh\necho $$ >{carpeta}/pid\nexec sleep 60\n"),
        )
        .unwrap();
        std::fs::set_permissions(&falso, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        let _mpv = MpvProceso::arrancar(50, |_| {});
        std::thread::sleep(Duration::from_secs(60));
        return;
    }

    let carpeta = tempfile::tempdir().unwrap();
    let mut hijo = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "mpv_muere_con_el_player",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("VORTEX_PRUEBA_HIJO", carpeta.path())
        .env("VORTEX_PLAYER_MPV", carpeta.path().join("mpv"))
        .spawn()
        .unwrap();
    let archivo = carpeta.path().join("pid");
    let inicio = Instant::now();
    let pid = loop {
        if let Some(p) = std::fs::read_to_string(&archivo)
            .ok()
            .and_then(|t| t.trim().parse::<i32>().ok())
        {
            break p;
        }
        assert!(
            inicio.elapsed() < Duration::from_secs(10),
            "el mpv falso nunca arrancó"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(vivo(pid), "el mpv falso debería estar vivo con el Player vivo");

    hijo.kill().unwrap(); // SIGKILL: no corre ningún Drop
    hijo.wait().unwrap();

    let inicio = Instant::now();
    while vivo(pid) {
        assert!(
            inicio.elapsed() < Duration::from_secs(5),
            "el mpv se quedó huérfano (pid {pid})"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
