//! La medición de punta a punta con un /proc y un cgroup falsos: un «juego» (pid 123) en su scope y Chrome
//! con 2,4 GB. Se le sube la presión de memoria al cgroup del juego y se espera un tirón por la memoria.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const BIN: &str = env!("CARGO_BIN_EXE_vortex-tirones");
const CG: &str = "user.slice/app.slice/app-vortex-steam_app_480-123.scope";

fn psi(total_full: u64) -> String {
    format!(
        "some avg10=0.00 avg60=0.00 avg300=0.00 total={total_full}\nfull avg10=0.00 avg60=0.00 avg300=0.00 total={total_full}\n"
    )
}

fn proceso(raiz: &Path, pid: u32, comm: &str, ppid: u32, rss_paginas: u64) {
    let d = raiz.join("proc").join(pid.to_string());
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("stat"),
        format!("{pid} ({comm}) S {ppid} 1 1 0 -1 0 0 0 0 0 100 10 0 0 20 0 1 0 1 1 {rss_paginas} 0\n"),
    )
    .unwrap();
    fs::write(d.join("cgroup"), format!("0::/{CG}\n")).unwrap();
}

fn preparar() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let r = t.path();
    proceso(r, 123, "game.exe", 1, 100_000);
    proceso(r, 200, "chrome", 1, 629_146); // 2,4 GB
    proceso(r, 201, "Discord", 1, 50_000);
    fs::create_dir_all(r.join("proc/pressure")).unwrap();
    fs::write(r.join("proc/pressure/memory"), psi(0)).unwrap();
    fs::write(r.join("proc/pressure/io"), psi(0)).unwrap();
    fs::write(r.join("proc/vmstat"), "pgfault 1\npswpin 10\n").unwrap();
    let cg = r.join("cg").join(CG);
    fs::create_dir_all(&cg).unwrap();
    for f in ["memory.pressure", "io.pressure", "cpu.pressure"] {
        fs::write(cg.join(f), psi(0)).unwrap();
    }
    fs::write(cg.join("cgroup.procs"), "123\n").unwrap();
    fs::create_dir_all(r.join("hwmon")).unwrap();
    (t, cg)
}

fn orden(raiz: &Path, args: &[&str]) -> Command {
    let mut c = Command::new(BIN);
    c.args(args)
        .env("VORTEX_TIRONES_PROC", raiz.join("proc"))
        .env("VORTEX_TIRONES_CGROOT", raiz.join("cg"))
        .env("VORTEX_HWMON", raiz.join("hwmon"))
        .env("XDG_STATE_HOME", raiz.join("estado"))
        .env("VORTEX_TIRONES_PASO_MS", "20");
    c
}

fn medir(raiz: &Path) -> Child {
    orden(raiz, &["medir", "123", "steam_app_480"])
        .stdout(Stdio::null())
        .spawn()
        .unwrap()
}

fn esperar_candado(raiz: &Path) {
    let c = raiz.join("estado/vortex/tirones/medidor.lock");
    let limite = Instant::now() + Duration::from_secs(5);
    while !c.exists() && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(100));
}

#[test]
fn un_tiron_por_la_memoria_con_su_culpable() {
    let (t, cg) = preparar();
    let r = t.path();
    let mut hijo = medir(r);
    esperar_candado(r);
    // Otra medición mientras sigue esta: no hace nada y sale ya.
    let otra = orden(r, &["medir", "123"]).status().unwrap();
    assert!(otra.success());
    // 200 ms con todo el juego esperando memoria, y páginas que vuelven del swap.
    fs::write(cg.join("memory.pressure"), psi(200_000)).unwrap();
    fs::write(r.join("proc/vmstat"), "pgfault 1\npswpin 90\n").unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let salida = orden(r, &["terminar", "--desde", "0"]).output().unwrap();
    let texto = String::from_utf8_lossy(&salida.stdout);
    assert_eq!(
        texto,
        "1 tirón por la memoria\nSe llenó la memoria y Google Chrome tenía 2,4 GB: ciérralo antes de jugar.\n"
    );
    assert!(hijo.wait().unwrap().success(), "el medidor termina al pedírselo");
    let ver = orden(r, &["ver"]).output().unwrap();
    let ver = String::from_utf8_lossy(&ver.stdout);
    assert!(ver.starts_with("steam_app_480 · 0 min"), "{ver}");
    assert!(
        ver.contains("ms  por la memoria (Google Chrome, 2,4 GB)"),
        "{ver}"
    );
    // Un resumen de una partida de antes no se repite en la siguiente.
    let viejo = orden(r, &["terminar", "--desde", "99999999999"])
        .output()
        .unwrap();
    assert!(viejo.stdout.is_empty());
}

#[test]
fn el_sistema_ahogado_no_cuenta_si_el_juego_estaba_bien() {
    let (t, _cg) = preparar();
    let r = t.path();
    let mut hijo = medir(r);
    esperar_candado(r);
    // Todo el sistema 300 ms parado, pero el scope del juego (el gestor lo protege) en 0: no hubo tirón.
    fs::write(r.join("proc/pressure/memory"), psi(300_000)).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    let salida = orden(r, &["terminar", "--desde", "0"]).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&salida.stdout),
        "Sin tirones por la memoria, el disco o el procesador\n"
    );
    assert!(hijo.wait().unwrap().success());
}

#[test]
fn termina_solo_cuando_se_cierra_el_juego_y_sin_tirones() {
    let (t, _cg) = preparar();
    let r = t.path();
    let mut hijo = medir(r);
    esperar_candado(r);
    fs::remove_dir_all(r.join("proc/123")).unwrap();
    let limite = Instant::now() + Duration::from_secs(3);
    let estado = loop {
        if let Some(e) = hijo.try_wait().unwrap() {
            break e;
        }
        assert!(Instant::now() < limite, "el medidor sigue con el juego cerrado");
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(estado.success());
    let salida = orden(r, &["terminar", "--desde", "0"]).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&salida.stdout),
        "Sin tirones por la memoria, el disco o el procesador\n"
    );
}

#[test]
fn ordenes_raras() {
    let (t, _cg) = preparar();
    assert_eq!(orden(t.path(), &["medir", "x"]).status().unwrap().code(), Some(2));
    assert_eq!(orden(t.path(), &["hola"]).status().unwrap().code(), Some(2));
    let ver = orden(t.path(), &["ver"]).output().unwrap();
    assert!(String::from_utf8_lossy(&ver.stdout).starts_with("Todavía no hay partidas medidas"));
}
