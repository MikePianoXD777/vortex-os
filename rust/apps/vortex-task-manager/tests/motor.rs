//! Las pruebas de `pruebas/test_motor.py`, portadas, más las de lo que Python hacía con psutil y aquí se
//! lee de `/proc`: se usa un `/proc` y un `/sys` falsos (carpetas temporales). Solo se toca un proceso
//! que crean ellas mismas (un `sleep` propio); nunca uno de la persona.

#![allow(clippy::too_many_arguments)] // los ayudantes de las pruebas arman procesos con todos sus campos

use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use vortex_core::carpeta_de_prueba;
use vortex_task_manager::grupos::{self, Ventana, agrupar, coincide, ordenar};
use vortex_task_manager::lanzadores::Aplicaciones;
use vortex_task_manager::procesos::{Proceso, Procesos, nombre_de};
use vortex_task_manager::rendimiento::{self, Raices, Rendimiento};
use vortex_task_manager::vista::{Muestra, Vista};
use vortex_task_manager::vram::{self, MemoriaGrafica};
use vortex_task_manager::{acciones, ejecutar, historia::Historia, modelo, servicios, sys};

const MIB: u64 = 1024 * 1024;

fn proc(pid: i32, ppid: i32, nombre: &str, uid: u32, cpu: f64, mem: u64, estado: &str, vram: u64) -> Proceso {
    Proceso {
        pid,
        ppid,
        nombre: nombre.into(),
        comando: nombre.into(),
        uid,
        usuario: if uid == 1000 { "mike".into() } else { "root".into() },
        cpu,
        mem,
        hilos: 1,
        estado: estado.into(),
        disco: 0.0,
        vram,
    }
}

fn p(pid: i32, ppid: i32, nombre: &str) -> Proceso {
    proc(pid, ppid, nombre, 1000, 0.0, 0, "sleeping", 0)
}

fn v(pid: i32, clase: &str, titulo: &str, dir: &str) -> Ventana {
    Ventana {
        pid,
        clase: clase.into(),
        titulo: titulo.into(),
        direccion: dir.into(),
    }
}

fn s(x: &[&str]) -> Vec<String> {
    x.iter().map(|x| x.to_string()).collect()
}

// --- Nombres ---

#[test]
fn interpretes_muestran_su_script() {
    assert_eq!(
        nombre_de("python3", &s(&["python3", "-m", "vortex_player"])),
        "vortex_player"
    );
    assert_eq!(
        nombre_de(
            "python3",
            &s(&["/usr/bin/python3", "-u", "/opt/x/focus_daemon.py"])
        ),
        "focus_daemon.py"
    );
    assert_eq!(
        nombre_de("bash", &s(&["bash", "/usr/bin/vortex-clips", "servicio"])),
        "vortex-clips"
    );
    assert_eq!(
        nombre_de("python3", &s(&["python3", "-c", "print(1)"])),
        "python3"
    );
    assert_eq!(nombre_de("fish", &s(&["fish"])), "fish");
}

#[test]
fn comm_cortado_usa_la_linea_de_comando() {
    assert_eq!(
        nombre_de(
            "chrome_crashpad",
            &s(&["/opt/chrome/chrome_crashpad_handler", "--x"])
        ),
        "chrome_crashpad_handler"
    );
}

// --- Lanzadores ---

fn carpeta_de_lanzadores() -> PathBuf {
    let d = carpeta_de_prueba("lanzadores");
    fs::write(d.join("org.mozilla.firefox.desktop"), "[Desktop Entry]\nType=Application\nName=Firefox\nName[es]=Firefox ES\nIcon=org.mozilla.firefox\nExec=firefox %u\n").unwrap();
    fs::write(
        d.join("kitty-open.desktop"),
        "[Desktop Entry]\nType=Application\nName=kitty URL Launcher\nExec=kitty +open %U\nNoDisplay=true\n",
    )
    .unwrap();
    fs::write(
        d.join("kitty.desktop"),
        "[Desktop Entry]\nType=Application\nName=kitty\nIcon=kitty\nExec=kitty\n",
    )
    .unwrap();
    fs::write(
        d.join("steam.desktop"),
        "[Desktop Entry]\nType=Application\nName=Steam\nIcon=steam\nExec=env X=1 /usr/bin/steam %U\nStartupWMClass=Steam\n[Desktop Action x]\nName=Otra cosa\n",
    )
    .unwrap();
    d
}

#[test]
fn busca_por_clase_id_y_ejecutable() {
    let a = Aplicaciones::de_carpetas(&[carpeta_de_lanzadores()]);
    assert_eq!(a.de_clase("firefox").unwrap().icono, "org.mozilla.firefox"); // último pedazo del id
    assert_eq!(
        a.de_clase("org.mozilla.firefox").unwrap().id,
        "org.mozilla.firefox"
    );
    assert_eq!(a.de_clase("Steam").unwrap().nombre, "Steam"); // StartupWMClass, sin la acción
    assert_eq!(a.de_ejecutable("steam").unwrap().nombre, "Steam"); // Exec con env y ruta
    assert_eq!(a.de_ejecutable("kitty").unwrap().nombre, "kitty"); // el visible, no el oculto
    assert!(a.de_clase("nada").is_none());
}

// --- Agrupar ---

fn escenario() -> (Vec<Proceso>, Vec<Ventana>, Aplicaciones) {
    let filas = vec![
        proc(1, 0, "systemd", 0, 0.0, 0, "sleeping", 0),
        proc(100, 1, "kitty", 1000, 1.0, 50, "sleeping", 0),
        proc(101, 100, "fish", 1000, 0.0, 5, "sleeping", 0),
        proc(102, 101, "claude", 1000, 4.0, 300, "sleeping", 0),
        proc(200, 1, "firefox", 1000, 10.0, 900, "sleeping", 0),
        proc(201, 200, "firefox", 1000, 2.0, 400, "sleeping", 0),
        proc(300, 1, "pipewire", 1000, 0.0, 20, "sleeping", 0),
        proc(301, 1, "pipewire", 1000, 0.0, 10, "sleeping", 0),
        proc(400, 1, "sshd", 0, 0.0, 8, "sleeping", 0),
    ];
    let ventanas = vec![
        v(100, "kitty", "~", "0x1"),
        v(200, "firefox", "YouTube", "0x2"),
        v(200, "firefox", "Otra", "0x3"),
    ];
    (filas, ventanas, Aplicaciones::de_carpetas(&[]))
}

#[test]
fn apps_fondo_y_sistema() {
    let (filas, ventanas, apps) = escenario();
    let g = agrupar(&filas, &ventanas, &apps, 1000);
    let por_nombre = |l: &[grupos::Grupo], n: &str| l.iter().find(|x| x.nombre == n).unwrap().clone();
    let kitty = por_nombre(&g.apps, "kitty");
    let mut pids = kitty.pids.clone();
    pids.sort();
    assert_eq!(pids, vec![100, 101, 102]); // la terminal con lo que corre dentro
    assert_eq!(kitty.cpu, 5.0);
    let firefox = por_nombre(&g.apps, "firefox");
    assert_eq!(firefox.subtitulo, "2 ventanas");
    assert_eq!(firefox.ventanas, vec!["0x2", "0x3"]);
    assert_eq!(firefox.mem, 1300);
    let mut pw = por_nombre(&g.fondo, "pipewire").pids;
    pw.sort();
    assert_eq!(pw, vec![300, 301]); // juntos por nombre
    let sistema: Vec<&str> = g.sistema.iter().map(|x| x.nombre.as_str()).collect();
    assert_eq!(sistema, vec!["systemd", "sshd"]);
}

#[test]
fn ordenar_y_buscar() {
    let (filas, ventanas, apps) = escenario();
    let g = agrupar(&filas, &ventanas, &apps, 1000);
    let nombres = |por: &str| {
        ordenar(&g.apps, por)
            .iter()
            .map(|x| x.nombre.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(nombres("mem"), vec!["firefox", "kitty"]);
    assert_eq!(nombres("nombre"), vec!["firefox", "kitty"]);
    let kitty = g.apps.iter().find(|x| x.nombre == "kitty").unwrap();
    assert!(coincide(kitty, "claude")); // por un proceso de adentro
    assert!(coincide(kitty, "102")); // por PID
    assert!(!coincide(kitty, "firefox"));
    assert!(coincide(kitty, "  "));
}

#[test]
fn suspendido_solo_si_todos() {
    let filas = vec![
        proc(10, 1, "a", 1000, 0.0, 0, "stopped", 0),
        proc(11, 10, "b", 1000, 0.0, 0, "stopped", 0),
    ];
    let g = agrupar(
        &filas,
        &[v(10, "a", "", "0x9")],
        &Aplicaciones::de_carpetas(&[]),
        1000,
    );
    assert!(g.apps[0].suspendido);
    let filas = vec![
        proc(10, 1, "a", 1000, 0.0, 0, "stopped", 0),
        proc(11, 10, "b", 1000, 0.0, 0, "sleeping", 0),
    ];
    assert!(
        !agrupar(
            &filas,
            &[v(10, "a", "", "0x9")],
            &Aplicaciones::de_carpetas(&[]),
            1000
        )
        .apps[0]
            .suspendido
    );
}

#[test]
fn lanzador_da_nombre_e_icono() {
    let apps = Aplicaciones::de_carpetas(&[carpeta_de_lanzadores()]);
    let filas = vec![p(500, 1, "steam"), p(501, 1, "otra-cosa")];
    let g = agrupar(&filas, &[v(500, "Steam", "Biblioteca", "0x5")], &apps, 1000);
    assert_eq!(
        (
            g.apps[0].nombre.as_str(),
            g.apps[0].icono.as_str(),
            g.apps[0].subtitulo.as_str()
        ),
        ("Steam", "steam", "Biblioteca")
    );
    let g = agrupar(&[p(600, 1, "kitty")], &[], &apps, 1000);
    assert_eq!(
        (g.fondo[0].nombre.as_str(), g.fondo[0].subtitulo.as_str()),
        ("kitty", "")
    ); // mismo nombre: sin subtítulo
}

#[test]
fn vram_se_suma_por_app_y_se_ordena() {
    let filas = vec![
        proc(100, 1, "kitty", 1000, 0.0, 50, "sleeping", 0),
        proc(101, 100, "llama-server", 1000, 0.0, 0, "sleeping", 6000 * MIB),
        proc(200, 1, "firefox", 1000, 0.0, 900, "sleeping", 300 * MIB),
        proc(201, 200, "firefox", 1000, 0.0, 0, "sleeping", 100 * MIB),
        proc(300, 1, "pipewire", 1000, 0.0, 0, "sleeping", 0),
    ];
    let ventanas = vec![v(100, "kitty", "~", "0x1"), v(200, "firefox", "YouTube", "0x2")];
    let g = agrupar(&filas, &ventanas, &Aplicaciones::de_carpetas(&[]), 1000);
    let apps: HashMap<&str, &grupos::Grupo> = g.apps.iter().map(|x| (x.nombre.as_str(), x)).collect();
    assert_eq!(apps["kitty"].vram, 6000 * MIB); // la IA que corre dentro de la terminal
    assert_eq!(apps["firefox"].vram, 400 * MIB);
    assert_eq!(g.fondo[0].vram, 0);
    let por = |o: &str| {
        ordenar(&g.apps, o)
            .iter()
            .map(|x| x.nombre.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(por("vram"), vec!["kitty", "firefox"]);
    assert_eq!(por("mem"), vec!["firefox", "kitty"]);
    assert!(grupos::ORDENES.contains(&"vram"));
}

// --- VRAM ---

const NVIDIA_PIDS: &str = "
==============NVSMI LOG==============

Timestamp                                              : Sat Nov  7 19:04:28 2026
Driver Version                                         : 615.78.08

Attached GPUs                                          : 2
GPU 00000000:01:00.0
    Processes
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 1226
            Type                          : G
            Name                          : Hyprland
            Used GPU Memory               : 107 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 2874
            Type                          : C+G
            Name                          : /usr/lib/claude/claude --type=gpu-process --x=a:b
            Used GPU Memory               : 90 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5000
            Type                          : C
            Name                          : ollama
            Used GPU Memory               : 6144 MiB
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5001
            Type                          : G
            Name                          : raro
            Used GPU Memory               : Not available
GPU 00000000:02:00.0
    Processes
        GPU instance ID                   : N/A
        Compute instance ID               : N/A
        Process ID                        : 5000
            Type                          : C
            Name                          : ollama
            Used GPU Memory               : 2048 MiB
";

const FDINFO_AMD: &str = "pos:\t0\nflags:\t02100002\nmnt_id:\t26\nino:\t1046\ndrm-driver:\tamdgpu\ndrm-client-id:\t17\ndrm-pdev:\t0000:03:00.0\npasid:\t32790\ndrm-memory-vram:\t524288 KiB\ndrm-memory-gtt: \t2048 KiB\ndrm-memory-cpu: \t0 KiB\ndrm-engine-gfx:\t123456 ns\n";
const FDINFO_AMD_NUEVO: &str = "drm-driver:\tamdgpu\ndrm-client-id:\t18\ndrm-pdev:\t0000:03:00.0\ndrm-memory-vram:\t999 KiB\ndrm-total-vram:\t300 MiB\ndrm-resident-vram:\t256 MiB\ndrm-total-gtt:\t8 MiB\n";
const FDINFO_I915: &str = "drm-driver:\ti915\ndrm-client-id:\t5\ndrm-pdev:\t0000:00:02.0\ndrm-total-system0:\t64 MiB\ndrm-resident-system0:\t48 MiB\ndrm-total-stolen-system0:\t0\n";
const FDINFO_XE: &str = "drm-driver:\txe\ndrm-client-id:\t9\ndrm-pdev:\t0000:03:00.0\ndrm-total-system:\t10 MiB\ndrm-resident-system:\t10 MiB\ndrm-total-vram0:\t200 MiB\ndrm-resident-vram0:\t128 MiB\n";
const FDINFO_NVIDIA_DRM: &str =
    "pos:\t0\nflags:\t02100002\ndrm-driver:\tnvidia-drm\ndrm-client-id:\t3\ndrm-pdev:\t0000:01:00.0\n";

#[test]
fn a_bytes() {
    assert_eq!(vram::a_bytes("865 MiB"), Some(865 * MIB));
    assert_eq!(vram::a_bytes("524288 KiB"), Some(512 * MIB));
    assert_eq!(vram::a_bytes("4096"), Some(4096));
    assert_eq!(vram::a_bytes("2 GiB"), Some(2048 * MIB));
    assert_eq!(vram::a_bytes("1.5 KiB"), Some(1536));
    assert_eq!(vram::a_bytes("N/A"), None);
    assert_eq!(vram::a_bytes("Not available"), None);
    assert_eq!(vram::a_bytes("5 XiB"), None);
    assert_eq!(vram::a_bytes(""), None);
}

#[test]
fn nvidia_trae_graficos_y_computo_y_suma_tarjetas() {
    let r = vram::parsear_pids_nvidia(NVIDIA_PIDS);
    let esperado: HashMap<i32, u64> = [(1226, 107 * MIB), (2874, 90 * MIB), (5000, 8192 * MIB)].into();
    assert_eq!(r, esperado); // sin «Not available»
    assert!(vram::parsear_pids_nvidia("GPU 00000000:01:00.0\n    Processes : None\n").is_empty());
}

#[test]
fn fdinfo_segun_el_driver() {
    let t = |x| vram::parsear_fdinfo_drm(x);
    assert_eq!(t(FDINFO_AMD), Some(("0000:03:00.0/17".into(), Some(512 * MIB))));
    assert_eq!(
        t(FDINFO_AMD_NUEVO),
        Some(("0000:03:00.0/18".into(), Some(256 * MIB)))
    ); // resident gana
    assert_eq!(t(FDINFO_XE), Some(("0000:03:00.0/9".into(), Some(128 * MIB)))); // vram, no system
    assert_eq!(t(FDINFO_I915), Some(("0000:00:02.0/5".into(), Some(48 * MIB))));
    assert_eq!(t(FDINFO_NVIDIA_DRM), Some(("0000:01:00.0/3".into(), None))); // no dice
    assert_eq!(t("pos:\t0\nflags:\t02\nmnt_id:\t1\n"), None); // no es de la gráfica
}

fn proc_falso_vram() -> PathBuf {
    let raiz = carpeta_de_prueba("vram");
    let abrir = |pid: i32, fd: i32, destino: &str, info: &str| {
        fs::create_dir_all(raiz.join(pid.to_string()).join("fd")).unwrap();
        fs::create_dir_all(raiz.join(pid.to_string()).join("fdinfo")).unwrap();
        symlink(
            destino,
            raiz.join(pid.to_string()).join("fd").join(fd.to_string()),
        )
        .unwrap();
        fs::write(
            raiz.join(pid.to_string()).join("fdinfo").join(fd.to_string()),
            info,
        )
        .unwrap();
    };
    abrir(10, 3, "/dev/dri/renderD128", FDINFO_AMD);
    abrir(10, 4, "/dev/dri/renderD128", FDINFO_AMD); // el mismo cliente duplicado (dup)
    abrir(10, 5, "/dev/dri/card1", FDINFO_AMD_NUEVO); // otro cliente del mismo proceso
    abrir(10, 6, "/home/mike/x.txt", FDINFO_AMD); // no es de /dev/dri: no cuenta
    abrir(11, 3, "/dev/dri/renderD128", FDINFO_AMD); // hijo de un fork con el fd heredado
    abrir(12, 3, "/dev/dri/renderD128", FDINFO_NVIDIA_DRM);
    fs::create_dir_all(raiz.join("13")).unwrap(); // sin fd/: de otro usuario
    raiz
}

#[test]
fn fdinfo_suma_por_proceso_sin_duplicados() {
    let raiz = proc_falso_vram();
    let (r, supo) = vram::vram_fdinfo(&[13, 12, 11, 10, 99], &raiz);
    assert!(supo);
    assert_eq!(r, HashMap::from([(10, 512 * MIB + 256 * MIB)]));
    let (r, supo) = vram::vram_fdinfo(&[12, 13], &raiz); // solo nvidia-drm: no se sabe
    assert!(r.is_empty() && !supo);
}

#[test]
fn memoria_grafica_mide_cada_3_segundos() {
    let mut m = MemoriaGrafica::con(3.0, true, false);
    m.falsa_nvidia = Some(HashMap::from([(5000, 6144 * MIB)]));
    assert!(!m.disponible);
    assert_eq!(m.medir_en(&[5000], 100.0)[&5000], 6144 * MIB);
    assert!(m.disponible);
    // Entre medidas devuelve la última, aunque ahora diga otra cosa.
    m.falsa_nvidia = Some(HashMap::from([(5000, 1)]));
    assert_eq!(m.medir_en(&[5000], 102.9)[&5000], 6144 * MIB);
    assert_eq!(m.medir_en(&[5000], 103.0)[&5000], 1);
}

#[test]
fn sin_forma_de_saber_no_hay_columna() {
    let mut m = MemoriaGrafica::con(3.0, false, false);
    assert!(m.medir_en(&[1], 0.0).is_empty());
    assert!(!m.disponible);
}

#[test]
fn nvidia_y_amd_juntas() {
    let raiz = proc_falso_vram();
    let mut m = MemoriaGrafica::con(3.0, true, true).con_proc(&raiz);
    m.falsa_nvidia = Some(HashMap::from([(10, 100 * MIB), (5000, 6144 * MIB)]));
    let r = m.medir_en(&[10, 5000], 0.0).clone();
    assert_eq!(r, HashMap::from([(10, 868 * MIB), (5000, 6144 * MIB)]));
}

// --- Procesos: un /proc falso ---

fn escribir_proc(
    raiz: &Path,
    pid: i32,
    comm: &str,
    estado: char,
    ppid: i32,
    jiffies: u64,
    hilos: u32,
    uid: u32,
    cmd: &[&str],
    rss_paginas: u64,
    comp_paginas: u64,
    io: Option<(u64, u64)>,
) {
    let d = raiz.join(pid.to_string());
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("stat"),
        format!("{pid} ({comm}) {estado} {ppid} 0 0 0 0 0 0 0 0 0 {jiffies} 0 0 0 20 0 {hilos} 0 0 0 0\n"),
    )
    .unwrap();
    fs::write(
        d.join("statm"),
        format!("1000 {rss_paginas} {comp_paginas} 0 0 0 0\n"),
    )
    .unwrap();
    let mut c = cmd.join("\0");
    c.push('\0');
    fs::write(d.join("cmdline"), c).unwrap();
    fs::write(
        d.join("status"),
        format!("Name:\t{comm}\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n"),
    )
    .unwrap();
    if let Some((l, e)) = io {
        fs::write(
            d.join("io"),
            format!("rchar: 0\nwchar: 0\nread_bytes: {l}\nwrite_bytes: {e}\n"),
        )
        .unwrap();
    }
}

fn passwd_falso(dir: &Path) -> PathBuf {
    let f = dir.join("passwd");
    fs::write(
        &f,
        "root:x:0:0::/root:/bin/bash\nmike:x:1000:1000::/home/mike:/bin/fish\n",
    )
    .unwrap();
    f
}

#[test]
fn lee_un_proc_falso() {
    let raiz = carpeta_de_prueba("proc");
    let passwd = passwd_falso(&raiz);
    let pagina = sys::tam_pagina();
    escribir_proc(
        &raiz,
        1,
        "systemd",
        'S',
        0,
        100,
        1,
        0,
        &["/sbin/init"],
        10,
        4,
        None,
    );
    escribir_proc(&raiz, 2, "kthreadd", 'S', 0, 0, 1, 0, &[], 0, 0, None); // el kernel: no sale
    escribir_proc(&raiz, 3, "rcu_gp", 'I', 2, 0, 1, 0, &[], 0, 0, None); // hijo de kthreadd: no sale
    escribir_proc(
        &raiz,
        500,
        "python3",
        'S',
        1,
        1000,
        4,
        1000,
        &["python3", "-m", "vortex_player"],
        100,
        30,
        Some((1000, 2000)),
    );
    escribir_proc(
        &raiz,
        501,
        "mi (raro) prog",
        'T',
        500,
        0,
        2,
        1000,
        &["x"],
        50,
        80,
        None,
    ); // paréntesis en el nombre; compartida > rss
    fs::create_dir_all(raiz.join("self")).unwrap(); // no es un número: se ignora
    fs::write(raiz.join("meminfo"), "x").unwrap();
    let mut pr = Procesos::en(&raiz, &passwd);
    let primera = pr.muestrear_en(10.0);
    let n: Vec<i32> = primera.iter().map(|x| x.pid).collect();
    assert_eq!(n, vec![1, 500, 501]);
    let f: HashMap<i32, Proceso> = primera.into_iter().map(|x| (x.pid, x)).collect();
    assert_eq!(f[&500].nombre, "vortex_player");
    assert_eq!(f[&500].comando, "python3 -m vortex_player");
    assert_eq!(f[&500].usuario, "mike");
    assert_eq!(f[&1].usuario, "root");
    assert_eq!(f[&500].mem, 70 * pagina); // RSS menos compartida
    assert_eq!(f[&501].mem, 0); // sin pasar de cero
    assert_eq!(f[&500].hilos, 4);
    assert_eq!(f[&500].estado, "sleeping");
    assert_eq!(f[&501].estado, "stopped");
    assert_eq!(f[&501].nombre, "mi (raro) prog");
    assert_eq!((f[&500].cpu, f[&500].disco), (0.0, 0.0)); // la primera vez no hay con qué comparar

    // Un segundo después: 100 ticks más = un núcleo entero = 100 / nº de CPUs por ciento; 3000 bytes de disco.
    escribir_proc(
        &raiz,
        500,
        "python3",
        'S',
        1,
        1100,
        4,
        1000,
        &["python3", "-m", "vortex_player"],
        100,
        30,
        Some((2000, 4000)),
    );
    let segunda: HashMap<i32, Proceso> = pr.muestrear_en(11.0).into_iter().map(|x| (x.pid, x)).collect();
    let cpus = sys::procesadores() as f64;
    let ticks = if sys::ticks_por_segundo() == 100.0 {
        100.0
    } else {
        sys::ticks_por_segundo()
    };
    assert!(
        (segunda[&500].cpu - 100.0 / ticks * 100.0 / cpus).abs() < 1e-9,
        "{}",
        segunda[&500].cpu
    );
    assert_eq!(segunda[&500].disco, 3000.0);
    // Si un proceso se cierra entre medidas, simplemente ya no sale.
    fs::remove_dir_all(raiz.join("501")).unwrap();
    assert_eq!(pr.muestrear_en(12.0).len(), 2);
}

#[test]
fn proc_roto_no_truena() {
    let raiz = carpeta_de_prueba("proc-roto");
    fs::create_dir_all(raiz.join("77")).unwrap();
    fs::write(raiz.join("77").join("stat"), "basura").unwrap();
    fs::create_dir_all(raiz.join("78")).unwrap(); // sin archivos
    let mut pr = Procesos::en(&raiz, raiz.join("no-hay-passwd"));
    assert!(pr.muestrear_en(1.0).is_empty());
    let mut vacio = Procesos::en(raiz.join("no-existe"), raiz.join("x"));
    assert!(vacio.muestrear_en(1.0).is_empty());
}

// --- Procesos reales: solo un «sleep» propio ---

#[test]
fn muestrea_y_controla_un_proceso_propio() {
    let mut hijo = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = hijo.id() as i32;
    let mut pr = Procesos::new();
    pr.muestrear();
    let filas: HashMap<i32, Proceso> = pr.muestrear().into_iter().map(|x| (x.pid, x)).collect();
    assert_eq!(filas[&pid].nombre, "sleep");
    assert_eq!(filas[&(std::process::id() as i32)].usuario, filas[&pid].usuario);
    assert_eq!(acciones::suspender(&[pid], true), None);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let estado = pr.muestrear().into_iter().find(|x| x.pid == pid).unwrap().estado;
    assert_eq!(estado, "stopped");
    assert_eq!(acciones::suspender(&[pid], false), None);
    assert_eq!(acciones::prioridad(&[pid], "baja"), None);
    assert_eq!(acciones::prioridad_de(pid), "baja");
    assert_eq!(acciones::suspender(&[pid], true), None); // en pausa también termina
    assert_eq!(acciones::terminar(&[pid], false), None);
    assert_eq!(hijo.wait().unwrap().signal(), Some(15));
    // Ya no existe: no es un error.
    assert_eq!(acciones::terminar(&[pid], true), None);
}

#[test]
fn forzar_cierre_manda_sigkill() {
    let mut hijo = Command::new("sleep").arg("30").spawn().unwrap();
    assert_eq!(acciones::terminar(&[hijo.id() as i32], true), None);
    assert_eq!(hijo.wait().unwrap().signal(), Some(9));
}

#[test]
fn prioridad_desconocida_no_hace_nada() {
    assert_eq!(
        acciones::prioridad(&[std::process::id() as i32], "rarisima"),
        None
    );
    assert!(!acciones::prioridad_de(std::process::id() as i32).is_empty()); // depende de cómo corra la prueba
    assert_eq!(acciones::prioridad_de(-5), "");
}

// --- Ventanas de Hyprland ---

#[test]
fn ventanas_de_hyprland() {
    let json = r#"[{"pid":10,"class":"kitty","title":"~","address":"0x1","mapped":true},
        {"pid":0,"class":"x","address":"0x2"},
        {"pid":11,"class":"","initialClass":"Firefox","title":"t","address":"0x3","mapped":true},
        {"pid":12,"class":"oculta","address":"0x4","mapped":false}]"#;
    let r = acciones::parsear_ventanas(json);
    assert_eq!(r, vec![v(10, "kitty", "~", "0x1"), v(11, "Firefox", "t", "0x3")]);
    assert!(acciones::parsear_ventanas("").is_empty());
    assert!(acciones::parsear_ventanas("no es json").is_empty());
}

// --- Rendimiento: un /proc y un /sys falsos ---

fn mundo_falso() -> Raices {
    let t = carpeta_de_prueba("rend");
    let (proc, sys) = (t.join("proc"), t.join("sys"));
    fs::create_dir_all(proc.join("net")).unwrap();
    fs::write(proc.join("cpuinfo"), "processor\t: 0\nmodel name\t: AMD Ryzen 7 5800X 8-Core Processor\nphysical id\t: 0\ncore id\t: 0\n\nprocessor\t: 1\nmodel name\t: AMD Ryzen 7 5800X 8-Core Processor\nphysical id\t: 0\ncore id\t: 0\n\nprocessor\t: 2\nmodel name\t: AMD Ryzen 7 5800X 8-Core Processor\nphysical id\t: 0\ncore id\t: 1\n\nprocessor\t: 3\nmodel name\t: x\nphysical id\t: 0\ncore id\t: 1\n").unwrap();
    fs::write(proc.join("stat"), "cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 25 0 25 200 0 0 0 0 0 0\ncpu1 25 0 25 200 0 0 0 0 0 0\ncpu2 25 0 25 200 0 0 0 0 0 0\ncpu3 25 0 25 200 0 0 0 0 0 0\nintr 1\nbtime 1700000000\n").unwrap();
    fs::write(proc.join("meminfo"), "MemTotal:       16000000 kB\nMemFree:         1000000 kB\nMemAvailable:    4000000 kB\nBuffers:          100000 kB\nCached:          3000000 kB\nSwapTotal:       2000000 kB\nSwapFree:        1500000 kB\nSReclaimable:     200000 kB\n").unwrap();
    fs::write(proc.join("diskstats"), "   259       0 nvme0n1 100 0 2000 0 50 0 4000 0 0 100 0\n   259       1 nvme0n1p1 1 0 1 0 1 0 1 0 0 1 0\n").unwrap();
    fs::write(proc.join("net/dev"), "Inter-|   Recibido\n face |bytes\n  wlan0: 1000 0 0 0 0 0 0 0 500 0 0 0 0 0 0 0\n    lo: 9 0 0 0 0 0 0 0 9 0 0 0 0 0 0 0\n").unwrap();
    fs::write(
        proc.join("filesystems"),
        "nodev\tsysfs\nnodev\ttmpfs\n\text4\n\tbtrfs\n",
    )
    .unwrap();
    fs::write(
        proc.join("mounts"),
        "/dev/nvme0n1p1 / ext4 rw 0 0\ntmpfs /tmp tmpfs rw 0 0\n",
    )
    .unwrap();
    // Un disco, uno que es solo un loop, uno que no tiene tarjeta, un USB.
    let disco = |n: &str, modelo: &str, size: &str, rot: &str, rem: &str| {
        let d = sys.join("block").join(n);
        fs::create_dir_all(d.join("device")).unwrap();
        fs::create_dir_all(d.join("queue")).unwrap();
        fs::write(d.join("device/model"), format!("{modelo}\n")).unwrap();
        fs::write(d.join("size"), format!("{size}\n")).unwrap();
        fs::write(d.join("queue/rotational"), format!("{rot}\n")).unwrap();
        fs::write(d.join("removable"), format!("{rem}\n")).unwrap();
    };
    disco("nvme0n1", "WD Black SN850", "1953525168", "0", "0");
    disco("sda", "Barracuda", "976773168", "1", "0");
    disco("sdb", "Lector", "0", "0", "1");
    disco("sdc", "Pendrive", "15000000", "0", "1");
    fs::create_dir_all(sys.join("block/loop0/device")).unwrap();
    // Redes: wlan0 (Wi-Fi, arriba), eth0 (cable, abajo), lo y docker0 (sin /device).
    let red = |n: &str, wifi: bool, flags: &str, estado: &str| {
        let d = sys.join("class/net").join(n);
        fs::create_dir_all(d.join("device")).unwrap();
        if wifi {
            fs::create_dir_all(d.join("wireless")).unwrap();
        }
        fs::write(d.join("flags"), format!("{flags}\n")).unwrap();
        fs::write(d.join("operstate"), format!("{estado}\n")).unwrap();
        fs::write(d.join("speed"), "-1\n").unwrap();
    };
    red("wlan0", true, "0x1003", "up");
    red("eth0", false, "0x1003", "down"); // sin cable
    fs::create_dir_all(sys.join("class/net/lo")).unwrap();
    // Temperatura: un k10temp con Tctl, y otro chip que no cuenta.
    let hw = sys.join("class/hwmon/hwmon2");
    fs::create_dir_all(&hw).unwrap();
    fs::write(hw.join("name"), "k10temp\n").unwrap();
    fs::write(hw.join("temp1_input"), "55250\n").unwrap();
    fs::write(hw.join("temp1_label"), "Tctl\n").unwrap();
    fs::write(hw.join("temp2_input"), "40000\n").unwrap();
    fs::write(hw.join("temp2_label"), "Tccd1\n").unwrap();
    let hw = sys.join("class/hwmon/hwmon1");
    fs::create_dir_all(&hw).unwrap();
    fs::write(hw.join("name"), "nvme\n").unwrap();
    fs::write(hw.join("temp1_input"), "99000\n").unwrap();
    // Frecuencia: 2 núcleos con cpufreq.
    for (i, f) in [(0, 3_000_000), (1, 4_000_000)] {
        let c = sys.join(format!("devices/system/cpu/cpu{i}/cpufreq"));
        fs::create_dir_all(&c).unwrap();
        fs::write(c.join("scaling_cur_freq"), format!("{f}\n")).unwrap();
        fs::write(c.join("cpuinfo_max_freq"), "4700000\n").unwrap();
    }
    // Una tarjeta AMD.
    let g = sys.join("class/drm/card0/device");
    fs::create_dir_all(&g).unwrap();
    fs::write(g.join("vendor"), "0x1002\n").unwrap();
    fs::write(g.join("gpu_busy_percent"), "37\n").unwrap();
    fs::write(g.join("mem_info_vram_used"), "1073741824\n").unwrap();
    fs::write(g.join("mem_info_vram_total"), "8589934592\n").unwrap();
    Raices { proc, sys }
}

#[test]
fn rendimiento_de_un_mundo_falso() {
    let r = mundo_falso();
    let mut rend = Rendimiento::en(r.clone(), false);
    assert_eq!(rend.cpu_nombre, "AMD Ryzen 7 5800X");
    assert_eq!((rend.logicos, rend.fisicos), (4, 2));
    // Un segundo después: el núcleo 0 hizo 50 ticks ocupados de 100; el 1, 0 de 100; los otros nada.
    fs::write(r.proc.join("stat"), "cpu  100 0 100 800 0 0 0 0 0 0\ncpu0 50 0 50 250 0 0 0 0 0 0\ncpu1 25 0 25 300 0 0 0 0 0 0\ncpu2 25 0 25 200 0 0 0 0 0 0\ncpu3 25 0 25 200 0 0 0 0 0 0\nbtime 1700000000\n").unwrap();
    fs::write(
        r.proc.join("diskstats"),
        "   259       0 nvme0n1 100 0 4000 0 50 0 8000 0 0 600 0\n",
    )
    .unwrap();
    fs::write(r.proc.join("net/dev"), "Inter-|   Recibido\n face |bytes\n  wlan0: 3000 0 0 0 0 0 0 0 1500 0 0 0 0 0 0 0\n    lo: 9 0 0 0 0 0 0 0 9 0 0 0 0 0 0 0\n").unwrap();
    rend.muestrear_en(0.0, 0, 0);
    // Se vuelve a escribir lo de «antes» para medir bien: la primera llamada fija el antes.
    let d = rend.muestrear_en(1.0, 7, 21);
    assert_eq!(d["cpu"]["nucleos"], json!([0.0, 0.0, 0.0, 0.0])); // sin cambio entre la 1ª y la 2ª muestra
    assert_eq!(d["cpu"]["nombre"], "AMD Ryzen 7 5800X");
    assert_eq!(
        (d["cpu"]["procesos"].clone(), d["cpu"]["hilos"].clone()),
        (json!(7), json!(21))
    );
    assert!((d["cpu"]["temp"].as_f64().unwrap() - 55.25).abs() < 1e-9);
    assert!((d["cpu"]["ghz"].as_f64().unwrap() - 3.5).abs() < 1e-9);
    assert!((d["cpu"]["maxGhz"].as_f64().unwrap() - 4.7).abs() < 1e-9);
    assert!(d["cpu"]["encendido"].as_f64().unwrap() > 1e6);
    let mem = &d["mem"];
    assert_eq!(mem["total"], 16_000_000u64 * 1024);
    assert_eq!(mem["usada"], 12_000_000u64 * 1024);
    assert_eq!(mem["uso"], 75.0);
    assert_eq!(mem["cache"], (3_000_000u64 + 200_000 + 100_000) * 1024);
    assert_eq!(
        (mem["swapTotal"].clone(), mem["swapUsada"].clone()),
        (json!(2_000_000u64 * 1024), json!(500_000u64 * 1024))
    );
    // Solo discos de verdad (sin loop, sin el lector vacío), y de qué tipo son.
    let ids: Vec<&str> = d["discos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec!["nvme0n1", "sda", "sdc"]);
    let tipos: Vec<&str> = d["discos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["tipo"].as_str().unwrap())
        .collect();
    assert_eq!(tipos, vec!["NVMe", "HDD", "USB"]);
    assert_eq!(d["discos"][0]["capacidad"], 1_953_525_168u64 * 512);
    assert_eq!(d["discos"][0]["modelo"], "WD Black SN850");
    // Redes: solo wlan0 está arriba y con cable (eth0 sin cable: operstate «down»).
    assert_eq!(d["redes"].as_array().unwrap().len(), 1);
    assert_eq!(
        (d["redes"][0]["id"].clone(), d["redes"][0]["tipo"].clone()),
        (json!("wlan0"), json!("Wi-Fi"))
    );
    assert_eq!(d["redes"][0]["velocidad"], 0);
    // Tarjeta AMD de sysfs.
    assert_eq!(d["gpus"][0]["id"], "card0");
    assert_eq!(d["gpus"][0]["uso"], 37.0);
    assert_eq!(d["gpus"][0]["vramTotal"], 8_589_934_592_f64);
}

#[test]
fn velocidades_y_porcentajes_entre_muestras() {
    let r = mundo_falso();
    let mut rend = Rendimiento::en(r.clone(), false);
    let antes = rend.muestrear_en(0.0, 0, 0);
    // Todo en cero la primera vez.
    assert_eq!(antes["redes"][0]["envio"], 0.0);
    fs::write(r.proc.join("stat"), "cpu  0 0 0 0 0 0 0 0 0 0\ncpu0 50 0 50 250 0 0 0 0 0 0\ncpu1 25 0 25 300 0 0 0 0 0 0\ncpu2 25 0 25 200 0 0 0 0 0 0\ncpu3 25 0 25 200 0 0 0 0 0 0\n").unwrap();
    fs::write(
        r.proc.join("diskstats"),
        "   259       0 nvme0n1 100 0 4000 0 50 0 8000 0 0 600 0\n",
    )
    .unwrap();
    fs::write(
        r.proc.join("net/dev"),
        "Inter-|   Recibido\n face |bytes\n  wlan0: 3000 0 0 0 0 0 0 0 1500 0 0 0 0 0 0 0\n",
    )
    .unwrap();
    let d = rend.muestrear_en(2.0, 0, 0);
    // cpu0: 50 ocupados de 100 totales = 50 %; cpu1: nada ocupado de 50 = 0 %.
    assert_eq!(d["cpu"]["nucleos"], json!([50.0, 0.0, 0.0, 0.0]));
    assert_eq!(d["cpu"]["uso"], 12.5);
    // Disco: 2000 sectores más leídos en 2 s = 512 000 B/s; 4000 escritos = 1 024 000 B/s; 500 ms ocupado de 2000 ms = 25 %.
    assert_eq!(d["discos"][0]["lectura"], 512_000.0);
    assert_eq!(d["discos"][0]["escritura"], 1_024_000.0);
    assert_eq!(d["discos"][0]["activo"], 25.0);
    // Red: 2000 bytes recibidos y 1000 enviados en 2 s.
    assert_eq!(d["redes"][0]["recepcion"], 1000.0);
    assert_eq!(d["redes"][0]["envio"], 500.0);
}

#[test]
fn nombres_de_cpu() {
    let n = |x: &str| rendimiento::nombre_cpu_de(&format!("processor : 0\nmodel name\t: {x}\n"));
    assert_eq!(n("AMD Ryzen 7 5800X 8-Core Processor"), "AMD Ryzen 7 5800X");
    assert_eq!(n("Intel(R) Core(TM) i7-8700 CPU @ 3.20GHz"), "Intel Core i7-8700");
    assert_eq!(n("AMD Athlon(tm) Dual-Core Processor"), "AMD Athlon(tm)");
    assert_eq!(n("ARM  Cortex   Processor"), "ARM Cortex");
    assert_eq!(n("Cosa Rara"), "Cosa Rara");
    assert_eq!(rendimiento::nombre_cpu_de("sin nada"), "Procesador");
}

#[test]
fn gpus_nvidia_de_nvidia_smi() {
    let g = rendimiento::parsear_gpus_nvidia(
        "0, NVIDIA GeForce RTX 3080, 12, 1024, 10240, 45, 85.5\nlinea corta, 1\n",
    );
    assert_eq!(g.len(), 1);
    assert_eq!(g[0]["id"], "nvidia0");
    assert_eq!(g[0]["nombre"], "GeForce RTX 3080");
    assert_eq!(g[0]["vramUsada"], 1024.0 * 1048576.0);
    assert_eq!(g[0]["watts"], 85.5);
}

#[test]
fn rendimiento_real_da_cosas_con_sentido() {
    let mut r = Rendimiento::en(Raices::default(), false);
    r.muestrear(1, 1);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let d = r.muestrear(10, 20);
    assert!(d["mem"]["total"].as_u64().unwrap() > 0);
    let uso = d["cpu"]["uso"].as_f64().unwrap();
    assert!((0.0..=100.0).contains(&uso));
    assert_eq!(d["cpu"]["nucleos"].as_array().unwrap().len(), sys::procesadores());
    assert_eq!(
        (d["cpu"]["procesos"].clone(), d["cpu"]["hilos"].clone()),
        (json!(10), json!(20))
    );
    for disco in d["discos"].as_array().unwrap() {
        assert!(disco["capacidad"].as_u64().unwrap() > 0);
    }
}

// --- Servicios ---

#[test]
fn unir_servicios() {
    let unidades = vec![
        json!({"unit": "a.service", "load": "loaded", "active": "active", "sub": "running", "description": "A"}),
        json!({"unit": "b.service", "load": "loaded", "active": "failed", "sub": "failed", "description": "B"}),
        json!({"unit": "dbus-:1.14-x@0.service", "load": "loaded", "active": "active", "sub": "running", "description": "dbus-:1.14-x@0.service"}),
        json!({"unit": "c.socket", "load": "loaded", "active": "active", "sub": "listening", "description": "C"}),
        json!({"unit": "z.service", "load": "not-found", "active": "inactive", "sub": "dead", "description": "z"}),
    ];
    let archivos = vec![
        json!({"unit_file": "a.service", "state": "enabled"}),
        json!({"unit_file": "d.service", "state": "disabled"}),
        json!({"unit_file": "b.service", "state": "static"}),
        json!({"unit_file": "t@.service", "state": "disabled"}),
    ];
    let filas = servicios::unir(&unidades, &archivos);
    let nombres: Vec<&str> = filas.iter().map(|f| f["nombre"].as_str().unwrap()).collect();
    assert_eq!(nombres, vec!["b", "a", "d"]); // falló, corriendo, detenido
    assert_eq!(filas[1]["cambiable"], true);
    assert_eq!(filas[0]["cambiable"], false);
    assert_eq!(filas[2]["activo"], "inactive");
}

#[test]
fn accion_rechaza_cosas_raras() {
    assert_eq!(
        servicios::accion("--help", "start", true).as_deref(),
        Some("Acción no válida.")
    );
    assert_eq!(
        servicios::accion("a.service", "mask", true).as_deref(),
        Some("Acción no válida.")
    );
    assert_eq!(
        servicios::accion("a", "start", true).as_deref(),
        Some("Acción no válida.")
    );
}

// --- El modelo de la lista ---

fn claves(x: &str) -> Vec<String> {
    x.chars().map(String::from).collect()
}

#[test]
fn solo_cambia_lo_que_cambio() {
    let ops = modelo::diferencias(&claves("abcd"), &claves("abxcd"));
    assert_eq!(
        ops,
        vec![modelo::Operacion {
            i1: 2,
            i2: 2,
            j1: 2,
            j2: 3
        }]
    ); // un solo «insertar»
    let ops = modelo::diferencias(&claves("abxcd"), &claves("bxd"));
    assert_eq!(ops.len(), 2);
    assert!(ops.iter().all(|o| o.j1 == o.j2)); // solo se quitan
    assert_eq!(
        modelo::aplicar(&claves("abxcd"), &claves("bxd"), &ops),
        claves("bxd")
    );
    assert!(modelo::diferencias(&claves("abc"), &claves("abc")).is_empty());
    assert!(modelo::diferencias(&[], &[]).is_empty());
}

#[test]
fn las_operaciones_siempre_arman_la_lista_nueva() {
    // Un generador simple y fijo (sin crates de azar): mezclas, repeticiones de largo y listas vacías.
    let mut x: u64 = 0x9E3779B97F4A7C15;
    let mut sig = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for _ in 0..300 {
        let mut fabricar = |max: u64| -> Vec<String> {
            let mut v: Vec<String> = (0..sig() % max).map(|_| format!("k{}", sig() % 12)).collect();
            v.sort();
            v.dedup();
            // Se desordena un poco (claves únicas, orden distinto).
            for _ in 0..(sig() % 4) {
                if v.len() > 1 {
                    let (a, b) = ((sig() as usize) % v.len(), (sig() as usize) % v.len());
                    v.swap(a, b);
                }
            }
            v
        };
        let (a, b) = (fabricar(14), fabricar(14));
        let ops = modelo::diferencias(&a, &b);
        assert_eq!(modelo::aplicar(&a, &b, &ops), b, "{a:?} → {b:?} con {ops:?}");
    }
}

// --- La historia ---

#[test]
fn la_historia_guarda_60() {
    let mut h = Historia::new();
    let muestra = |i: f64| {
        json!({"cpu": {"uso": i, "nucleos": [i, i + 1.0]}, "mem": {"uso": 10.0},
            "discos": [{"id": "nvme0n1", "activo": 1.0, "lectura": 2.0, "escritura": 3.0}],
            "redes": [{"id": "wlan0", "envio": 4.0, "recepcion": 5.0}],
            "gpus": [{"id": "nvidia0", "uso": -1.0, "vramUsada": 25.0, "vramTotal": 100.0}]})
    };
    for i in 0..75 {
        h.guardar(&muestra(f64::from(i)));
    }
    let v = &h.valor;
    assert_eq!(v["cpu"].as_array().unwrap().len(), 60);
    assert_eq!(v["cpu"][59], 74.0);
    assert_eq!(v["cpu"][0], 15.0);
    assert_eq!(v["nucleos"].as_array().unwrap().len(), 2);
    assert_eq!(v["nucleos"][1][59], 75.0);
    assert_eq!(v["discos"]["nvme0n1"]["escritura"][0], 3.0);
    assert_eq!(v["redes"]["wlan0"]["recepcion"][0], 5.0);
    assert_eq!(v["gpus"]["nvidia0"]["uso"][0], 0.0); // sin dato (-1) se dibuja en cero
    assert_eq!(v["gpus"]["nvidia0"]["vram"][0], 25.0);
}

// --- La vista (lo que puente.py guardaba) ---

fn muestra_de(grupos: grupos::Grupos) -> Muestra {
    let n = grupos.todos().map(|g| g.procesos.len()).sum();
    Muestra {
        grupos: Some(grupos),
        rend: json!({"cpu": {"uso": 12.5, "procesos": n}, "mem": {"uso": 40.0, "usada": 100, "total": 400},
            "gpus": [{"nombre": "GeForce RTX 3080", "vramUsada": 1.0, "vramTotal": 10.0}, {"nombre": "Radeon X", "vramUsada": 1.0, "vramTotal": 5.0}],
            "discos": [], "redes": []}),
        vram_disponible: true,
        hilos: 0,
        procesos: n,
    }
}

#[test]
fn la_vista_arma_secciones_filtra_ordena_y_abre() {
    let (filas, ventanas, apps) = escenario();
    let g = agrupar(&filas, &ventanas, &apps, 1000);
    let mut vista = Vista::new();
    let mut valido = |_: &str| false;
    let a = vista.al_llegar(muestra_de(g), 12345, &mut valido).unwrap();
    let nombres: Vec<String> = a
        .filas
        .iter()
        .map(|f| f["nombre"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(nombres[0], "Apps");
    assert!(nombres.contains(&"Segundo plano".to_string()) && nombres.contains(&"Del sistema".to_string()));
    assert_eq!(vista.totales["procesos"], 9);
    assert_eq!(vista.resumen["gpu"]["nombre"], "RTX 3080"); // la de más VRAM, sin la marca
    assert_eq!(vista.resumen["topCpu"]["nombre"], "firefox");
    assert_eq!(vista.resumen["topMem"]["valor"], 1300);
    assert_eq!(vista.resumen["topVram"], json!({})); // nadie usa VRAM
    // Al abrir un grupo salen sus procesos; al volver a tocarlo, se van.
    assert!(vista.hay_grupos());
    vista.alternar_grupo("a:200");
    let con = vista.reconstruir(&mut valido);
    assert_eq!(con.filas.iter().filter(|f| f["tipo"] == "proceso").count(), 2);
    assert_eq!(con.ops.len(), 1); // solo se insertaron dos filas juntas
    vista.alternar_grupo("a:200");
    assert_eq!(
        vista
            .reconstruir(&mut valido)
            .filas
            .iter()
            .filter(|f| f["tipo"] == "proceso")
            .count(),
        0
    );
    // La búsqueda deja solo lo que coincide.
    assert!(vista.poner_filtro("claude"));
    let f = vista.reconstruir(&mut valido).filas;
    assert_eq!(f.iter().filter(|x| x["tipo"] == "grupo").count(), 1);
    assert_eq!(
        f.iter().find(|x| x["tipo"] == "grupo").unwrap()["nombre"],
        "kitty"
    );
    assert!(!vista.poner_filtro("claude"));
    vista.poner_filtro("");
    // Orden: solo los conocidos.
    assert!(vista.poner_orden("mem"));
    assert!(!vista.poner_orden("mem"));
    assert!(!vista.poner_orden("rarisimo"));
    assert_eq!(vista.orden, "mem");
    // Secciones colapsadas: sus grupos no salen (pero con búsqueda sí).
    vista.alternar_seccion("fondo");
    let f = vista.reconstruir(&mut valido).filas;
    assert!(f.iter().any(|x| x["tipo"] == "seccion" && x["colapsada"] == true));
    assert!(!f.iter().any(|x| x["tipo"] == "grupo" && x["seccion"] == "fondo"));
    vista.poner_filtro("pipewire");
    assert!(
        vista
            .reconstruir(&mut valido)
            .filas
            .iter()
            .any(|x| x["tipo"] == "grupo" && x["seccion"] == "fondo")
    );
}

#[test]
fn la_vista_da_pids_y_detalles() {
    let (filas, ventanas, apps) = escenario();
    let mut vista = Vista::new();
    vista.al_llegar(
        muestra_de(agrupar(&filas, &ventanas, &apps, 1000)),
        99999,
        &mut |_| false,
    );
    let mut pids = vista.pids("a:100");
    pids.sort();
    assert_eq!(pids, vec![100, 101, 102]);
    assert_eq!(vista.pids("a:100/101"), vec![101]);
    assert!(vista.pids("a:no-hay").is_empty());
    let d = vista.detalles("a:100");
    assert_eq!(d["ventana"], true);
    assert_eq!(d["usuario"], "mike");
    assert_eq!(vista.detalles("a:100/101")["ventana"], false); // un proceso suelto no se «cierra» como ventana
    assert_eq!(vista.detalles("a:100/101")["pid"], 101);
    assert_eq!(vista.detalles("nada"), json!({}));
    assert_eq!(vista.ventanas_de("a:200"), vec!["0x2", "0x3"]);
    // Una muestra sin procesos (otra página abierta) solo cambia CPU y memoria.
    let mut m = muestra_de(grupos::Grupos::default());
    m.grupos = None;
    assert!(vista.al_llegar(m, 99999, &mut |_| false).is_none());
    assert_eq!(vista.totales["cpu"], 12.5);
    assert_eq!(vista.totales["procesos"], 9);
}

#[test]
fn la_vista_no_se_acusa_a_si_misma_y_revisa_iconos() {
    let filas = vec![
        p(100, 1, "vortex-task-manager"),
        proc(200, 1, "gamer", 1000, 5.0, 10, "sleeping", 0),
    ];
    let mut vista = Vista::new();
    let mut revisados = Vec::new();
    let g = agrupar(
        &filas,
        &[v(100, "firefox", "yo", "0x1"), v(200, "Firefox", "otra", "0x2")],
        &Aplicaciones::de_carpetas(&[]),
        1000,
    );
    let a = {
        let mut valido = |n: &str| {
            revisados.push(n.to_string());
            n == "firefox"
        };
        vista.al_llegar(muestra_de(g), 100, &mut valido).unwrap()
    };
    assert_eq!(vista.resumen["topCpu"]["nombre"], "Firefox"); // no yo, aunque me toque la CPU
    // El ícono de «firefox» existe en el tema; el de «Firefox» se busca también en minúsculas.
    let iconos: Vec<&str> = a
        .filas
        .iter()
        .filter(|f| f["tipo"] == "grupo")
        .map(|f| f["icono"].as_str().unwrap())
        .collect();
    assert_eq!(iconos, vec!["firefox", "Firefox"]);
    // Con los nombres ya revisados, reconstruir no vuelve a preguntarle al tema de íconos.
    let antes = revisados.len();
    let mut otro = |_: &str| panic!("ya estaba en la caché");
    vista.reconstruir(&mut otro);
    assert!(antes > 0);
}

// --- Ejecutar con límite ---

#[test]
fn ejecutar_con_limite() {
    let r = ejecutar::con_limite(
        "sh",
        &["-c", "echo hola; echo mal >&2; exit 3"],
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!((r.codigo, r.stdout.trim(), r.stderr.trim()), (3, "hola", "mal"));
    let t = std::time::Instant::now();
    assert!(ejecutar::con_limite("sleep", &["10"], std::time::Duration::from_millis(200)).is_none());
    assert!(t.elapsed() < std::time::Duration::from_secs(3));
    assert!(
        ejecutar::con_limite(
            "programa-que-no-existe-vtm",
            &[],
            std::time::Duration::from_secs(1)
        )
        .is_none()
    );
    assert!(ejecutar::existe("sh") && !ejecutar::existe("programa-que-no-existe-vtm"));
    // Mucha salida no tapa la tubería.
    let r = ejecutar::con_limite(
        "sh",
        &["-c", "head -c 300000 /dev/zero | tr '\\0' x"],
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    assert_eq!(r.stdout.len(), 300_000);
}

#[test]
fn ubicacion_de_un_proceso() {
    let raiz = carpeta_de_prueba("ubicacion");
    fs::create_dir_all(raiz.join("42")).unwrap();
    symlink("/usr/bin/sleep (deleted)", raiz.join("42/exe")).unwrap();
    assert_eq!(
        vortex_task_manager::procesos::ubicacion_en(&raiz, 42),
        "/usr/bin/sleep"
    );
    assert_eq!(vortex_task_manager::procesos::ubicacion_en(&raiz, 43), "");
}
