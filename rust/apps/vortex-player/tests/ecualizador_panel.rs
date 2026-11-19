//! `test_mismo_estado_que_el_panel` de `test_motor.py`: el ecualizador del Player y el del panel del escritorio
//! son el mismo (el script `equalizer.sh` del shell). Con EasyEffects, systemctl, notify-send y pgrep de mentira
//! y todo en una carpeta temporal: no toca tu ecualizador. Va sola en su archivo porque cambia el entorno.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use vortex_player::ecualizador::{self, Banda};

fn ejecutable(ruta: &Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(ruta, format!("#!/usr/bin/env bash\n{texto}")).unwrap();
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
#[allow(unsafe_code)] // set_var: esta prueba corre sola en su proceso
fn mismo_estado_que_el_panel() {
    if std::process::Command::new("jq").arg("--version").output().is_err() {
        eprintln!("falta jq: se salta");
        return;
    }
    let t = tempfile::tempdir().unwrap();
    let home = t.path().join("home");
    let bin = t.path().join("bin");
    for n in ["easyeffects", "systemctl", "notify-send", "pgrep"] {
        ejecutable(&bin.join(n), "exit 0\n"); // notify-send: nada llega al escritorio
    }
    std::fs::create_dir_all(t.path().join("lv2/lsp-plugins.lv2")).unwrap(); // los plugins, de mentira
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    unsafe {
        std::env::set_var("HOME", &home);
        std::env::set_var("SERPANTINUM_DIR", repo.join("shell/src"));
        std::env::set_var("VORTEX_LV2_DIRS", t.path().join("lv2"));
        std::env::set_var("PATH", path);
        std::env::set_var("XDG_DATA_HOME", home.join(".local/share"));
        std::env::set_var("XDG_RUNTIME_DIR", t.path().join("run"));
        std::env::set_var("VORTEX_PLAYER_EQ", home.join(".local/state/serpantinum/music/eq_state.json"));
        std::env::remove_var("VORTEX_PLAYER_EQ_MPV");
    }
    assert!(ecualizador::script().is_some());
    let bandas = [
        Banda { f: 40.0, g: 4.0 },
        Banda { f: 1000.0, g: -2.5 },
        Banda { f: 12000.0, g: 6.0 },
    ];
    assert!(ecualizador::poner_bandas(&bandas));
    let e = ecualizador::estado();
    assert_eq!(e.bandas, bandas);
    assert_eq!((e.preset.as_str(), e.pending), ("Custom", false));
    assert!(ecualizador::poner_preset("Rock"));
    assert_eq!(ecualizador::estado().preset, "Rock");
    assert_eq!(ecualizador::estado().bandas.len(), 10);
    assert!(!ecualizador::poner_preset("Inventado"));
    // con los plugins no hay nada que avisar; sin ellos, la página dice qué falta y que no sonó
    assert_eq!(ecualizador::problema(), "");
    unsafe { std::env::set_var("VORTEX_LV2_DIRS", t.path().join("no-hay")) };
    assert!(ecualizador::problema().contains("lsp-plugins-lv2"));
    assert!(!ecualizador::poner_bandas(&bandas));
}
