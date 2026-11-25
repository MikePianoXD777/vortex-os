//! El gestor entero (`vortex-gestor-rs`) en un mundo falso: lo mismo que probaba `pruebas/gestor.sh` del
//! bash. Foco, scopes, nice, modo juego (propio y ajeno), modo IA (Miku fuera, CPU, CUDA, Vulkan, AMD y su
//! pelea con el modo juego), presión de memoria y salida. Ver `comun/mod.rs`.

mod comun;

use comun::{Mundo, esperar};
use std::time::Duration;

fn dormir(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// El mundo de siempre: kitty (con un hijo y un hilo), firefox de Flatpak, un juego de Wine, el shell y Steam.
fn mundo_base() -> Mundo {
    let m = Mundo::nuevo();
    m.proceso(4001, "kitty", &m.sesion, "30", &[4003], &[4002]);
    m.proceso(4002, "bash", &m.sesion, "30", &[], &[]);
    let ff = format!("{}/app.slice/app-flatpak-org.mozilla.firefox-1.scope", m.raiz);
    m.proceso(5001, "firefox", &ff, "0", &[], &[]);
    std::fs::create_dir_all(m.cgroup_dir(&ff)).unwrap();
    std::fs::write(m.cgroup_dir(&ff).join("cpu.weight"), "100\n").unwrap();
    m.proceso(6001, "wine64-preloader", &m.sesion, "30", &[6002], &[]);
    m.proceso(7001, "quickshell", &m.sesion, "30", &[], &[]);
    m.proceso(8001, "steam", &m.sesion, "30", &[], &[]);
    m
}

fn ff_peso(m: &Mundo) -> String {
    let ff = format!("{}/app.slice/app-flatpak-org.mozilla.firefox-1.scope", m.raiz);
    m.leer(&m.cgroup_dir(&ff).join("cpu.weight"))
}

#[test]
fn foco_scopes_nice_juego_presion_y_salida() {
    let mut m = mundo_base();
    *m.hypr.clientes.lock().unwrap() =
        r#"[{"address": "0xaaa1", "class": "kitty", "contentType": "none"}]"#.into();
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.conf("DEPURAR=1\nMEM_REVISAR_S=1\nIA_REVISAR_S=1\n");
    m.iniciar();
    let sc_kitty = m.scope("kitty", 4001);
    let sc_juego = m.scope("steam_app_570", 6001);
    esperar("kitty enfocada y con su scope", || {
        m.est("foco_pid=4001") && sc_kitty.is_dir()
    });

    assert!(
        m.en_log("busctl.log", "StartTransientUnit ssa(sv)a(sa(sv)) app-vortex-kitty-4001.scope fail 7 PIDs au 2 4001 4002 Slice s app.slice"),
        "kitty y su hijo van a su scope:\n{}",
        m.archivo("busctl.log")
    );
    assert!(
        m.en_log("busctl.log", "OOMPolicy s continue"),
        "el scope no cierra la app si el kernel mata un proceso"
    );
    assert!(
        m.en_log("busctl.log", "TimeoutStopUSec t 5000000 0"),
        "el scope se detiene en 5 s"
    );
    assert!(m.en_log("busctl.log", "Description s Vortex: kitty"));
    assert_eq!(m.leer(&sc_kitty.join("cpu.weight")), "500", "con el peso de foco");
    esperar("nice -5 en sus hilos", || m.nice_hilo(4001, 4003) == -5);
    assert!(
        m.nice_hilo(4001, 4001) == -5 && m.nice_hilo(4002, 4002) == -5,
        "nice -5 en sus hilos (sched_ext)"
    );
    assert!(
        m.en_log("renice.log", "--priority -5 -p"),
        "{}",
        m.archivo("renice.log")
    );
    assert!(m.est("foco_clase=kitty"));
    assert!(m.est(&format!(
        "foco_cgroup={}/app.slice/app-vortex-kitty-4001.scope",
        m.raiz
    )));
    assert!(
        m.en_log(
            "gestor.log",
            "foco: kitty (pid 4001) → app-vortex-kitty-4001.scope"
        ),
        "{}",
        m.log()
    );

    // firefox (flatpak): no se mueve, se le sube el peso; kitty vuelve al de fondo.
    m.hypr.activa(5001, "bbb2", "firefox", "none");
    m.hypr.evento(&["activewindowv2>>bbb2"]);
    esperar("firefox enfocada", || m.est("foco_pid=5001"));
    assert!(
        m.en_log(
            "busctl.log",
            "SetUnitProperties sba(sv) app-vortex-kitty-4001.scope true 2 CPUWeight t 50"
        ) && m.leer(&sc_kitty.join("cpu.weight")) == "50",
        "kitty vuelve al peso de fondo (en systemd, una vez)"
    );
    assert!(
        m.nice_hilo(4001, 4001) == 0 && m.nice_hilo(4001, 4003) == 0 && m.nice_hilo(4002, 4002) == 0,
        "y su nice vuelve a 0"
    );
    assert!(
        !m.en_log("busctl.log", "firefox") && ff_peso(&m) == "500",
        "firefox no se mueve: se le sube el peso"
    );
    assert!(
        m.nice_hilo(5001, 5001) == 0 && !m.en_log("renice.log", " 5001"),
        "sin RLIMIT_NICE no se toca su nice"
    );

    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.hypr.evento(&["activewindowv2>>aaa1"]);
    esperar("kitty otra vez", || m.est("foco_pid=4001"));
    assert_eq!(ff_peso(&m), "100", "firefox recupera su peso original");
    assert!(
        m.leer(&sc_kitty.join("cpu.weight")) == "500" && m.cuenta("busctl.log", "kitty") == 2,
        "kitty: peso de foco escrito directo (sin busctl)"
    );
    assert_eq!(m.leer(&sc_kitty.join("io.weight")), "default 500");

    // Un juego: modo juego, fondo en pausa y su propio scope.
    m.hypr.activa(6001, "ccc3", "steam_app_570", "none");
    m.hypr
        .evento(&["openwindow>>ccc3,1,steam_app_570,Dota 2", "activewindowv2>>ccc3"]);
    esperar("el juego enfocado", || m.est("foco_pid=6001"));
    esperar("el detector de tirones de Dota", || {
        m.en_log("tirones-llamado", "steam_app_570")
    });
    assert!(
        m.linea_exacta("tirones-llamado", "medir 6001 steam_app_570")
            && m.linea_exacta("guardados-llamado", "antes steam_app_570 6001"),
        "el juego abrió antes de enfocarse: los ayudantes esperan al foco y reciben su pid, no 0"
    );
    assert!(
        m.linea_exacta("vortex.log", "modo-juego on"),
        "juego: vortex modo-juego on"
    );
    assert!(
        m.en_log("qs.log", "call wallpaper setPlayback all pause"),
        "juego: fondo animado en pausa"
    );
    assert!(m.en_log("qs.log", "ipc -p /falso/Shell.qml call wallpaper"));
    assert!(
        sc_juego.is_dir() && m.leer(&sc_juego.join("cpu.weight")) == "500",
        "juego: su propio scope"
    );
    esperar("estado del juego", || {
        m.est("modo_juego=propio") && m.est("juegos=1")
    });
    esperar("la nota del juego", || m.en_log("aviso-juego", "steam_app_570"));
    assert!(
        m.linea_exacta("aviso-juego", "--juego steam_app_570 6001"),
        "el aviso de memoria del juego también recibe su pid (antes, 0)"
    );

    // Presión de memoria: el shell suelta memoria y se comprime lo de fondo (no el juego).
    m.psi_memoria();
    esperar("reclamos=1", || m.est("reclamos=1"));
    esperar("kitty comprimida", || {
        !m.leer(&sc_kitty.join("memory.reclaim")).is_empty()
    });
    assert_eq!(
        m.leer(&sc_kitty.join("memory.reclaim")),
        "64M",
        "presión: se comprime lo de kitty (fondo)"
    );
    assert!(
        m.leer(&sc_juego.join("memory.reclaim")).is_empty(),
        "presión: nada del juego enfocado"
    );
    esperar("el shell suelta memoria", || {
        m.en_log("qs.log", "call vortex soltarMemoria")
    });
    assert!(
        m.en_log("qs.log", "ipc -p /falso/Shell.qml show"),
        "se pregunta una vez qué sabe el shell"
    );
    assert!(m.est("shell_soltar=sí"));
    assert!(
        m.en_log("gestor.log", "presión de memoria (some avg10=3.10"),
        "presión: queda en el registro: {}",
        m.log()
    );
    m.psi_memoria();
    esperar("psi_eventos=2", || m.est("psi_eventos=2"));
    assert!(m.est("reclamos=1"), "presión: enfriamiento (no repite enseguida)");
    assert_eq!(m.cuenta("qs.log", " show"), 1);

    m.hypr.evento(&["closewindow>>ccc3"]);
    esperar("modo-juego off", || {
        m.linea_exacta("vortex.log", "modo-juego off")
    });
    esperar("el fondo sigue", || {
        m.en_log("qs.log", "call wallpaper setPlayback all play")
    });

    // Modo juego puesto por otro (gamemode): el gestor no lo pone ni lo quita.
    m.poner_marca("modo-juego");
    m.hypr.activa(8001, "fff6", "steam_app_1", "none");
    m.hypr
        .evento(&["openwindow>>fff6,1,steam_app_1,Juego", "activewindowv2>>fff6"]);
    esperar("pausa otra vez", || {
        m.cuenta("qs.log", "setPlayback all pause") == 2
    });
    esperar("juegos=1", || m.est("juegos=1") && m.est("modo_juego=ajeno"));
    m.hypr.evento(&["closewindow>>fff6"]);
    esperar("juegos=0", || m.est("juegos=0"));
    assert!(
        m.cuenta("vortex.log", "modo-juego") == 2 && m.marca("modo-juego").exists(),
        "modo juego ajeno: ni on ni off"
    );
    assert_eq!(
        m.cuenta("qs.log", "setPlayback all play"),
        1,
        "modo juego ajeno: el fondo sigue en pausa"
    );
    std::fs::remove_file(m.marca("modo-juego")).unwrap();

    // Juego por contentType (sin clase conocida).
    m.hypr.activa(4002, "abcd", "un.juego.raro", "game");
    m.hypr.evento(&["activewindowv2>>abcd"]);
    esperar("juegos=1 por contentType", || m.est("juegos=1"));
    assert_eq!(
        m.cuenta("vortex.log", "modo-juego on"),
        2,
        "juego por contentType=game"
    );
    esperar("la nota del juego raro", || {
        m.en_log("aviso-juego", "un.juego.raro")
    });
    assert!(
        m.linea_exacta("aviso-juego", "--juego un.juego.raro 4002"),
        "al abrir un juego mira si le alcanza la memoria, con su clase y su pid"
    );
    esperar("copia de partidas", || {
        m.en_log("guardados-llamado", "un.juego.raro")
    });
    assert!(
        m.linea_exacta("guardados-llamado", "antes un.juego.raro 4002"),
        "y respalda sus partidas guardadas antes de jugar (4.0)"
    );
    esperar("el detector de tirones", || {
        m.en_log("tirones-llamado", "un.juego.raro")
    });
    assert!(
        m.linea_exacta("tirones-llamado", "medir 4002 un.juego.raro"),
        "y empieza a medir los tirones de la partida, con el pid del juego (4.0)"
    );
    m.hypr.evento(&["closewindow>>abcd"]);
    esperar("modo-juego off 2", || {
        m.cuenta("vortex.log", "modo-juego off") == 2
    });

    // Poca memoria aunque el kernel todavía no avise de presión: el gestor la mira solo.
    std::fs::write(
        m.proc.join("meminfo"),
        "MemTotal: 8000000 kB\nMemAvailable: 600000 kB\n",
    )
    .unwrap();
    esperar("aviso de poca memoria", || {
        m.p().join("aviso-memoria").exists() && m.est("avisos_memoria=1")
    });
    std::fs::remove_file(m.proc.join("meminfo")).unwrap();

    // Shell y Hyprland nunca se mueven.
    m.hypr.activa(7001, "dead", "quickshell", "none");
    m.hypr.evento(&["activewindowv2>>dead"]);
    esperar("el shell enfocado", || m.est("foco_pid=7001"));
    assert!(
        !m.en_log("busctl.log", "7001") && m.est("foco_cgroup="),
        "el shell no se toca"
    );

    // El aviso de disco ya salió (una vez, con el ciclo ocioso).
    assert_eq!(
        m.cuenta("disco-llamado", "revisar"),
        1,
        "el aviso de disco, una vez cada 600 s"
    );

    // Salida: Hyprland se cierra.
    m.hypr.cerrar();
    assert_eq!(m.esperar_salida(), Some(0), "termina con 0 al cerrarse Hyprland");
    assert_eq!(
        m.leer(&sc_kitty.join("cpu.weight")),
        "100",
        "deja las apps con el peso normal"
    );
    assert!(!m.estado_archivo().exists(), "borra su estado");
    let log = m.log().to_lowercase();
    for malo in [
        "error",
        "command not found",
        "unbound",
        "sintaxis",
        "syntax",
        "panicked",
    ] {
        assert!(!log.contains(malo), "sin errores en su registro: {log}");
    }
    assert!(log.contains("hyprland se cerró: el gestor termina"));
}

#[test]
fn modo_ia() {
    let mut m = mundo_base();
    *m.hypr.clientes.lock().unwrap() = "[]".into();
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.conf("DEPURAR=1\nMEM_REVISAR_S=1\nIA_REVISAR_S=1\n");
    m.iniciar();
    let cg_kitty = format!("{}/app.slice/app-vortex-kitty-4001.scope", m.raiz);
    esperar("arranque", || m.est("foco_pid=4001"));
    // sin esto, la IA de la CPU del mundo falso no se ve: el modo IA necesita que haya NVIDIA
    std::fs::create_dir_all(m.proc.join("driver/nvidia")).unwrap();
    let marca = |c: &str| m.cuenta("vortex.log", c);

    // Miku: su voz con PyTorch y el Ollama que ella arranca (vortex-ollama.service) no cuentan.
    m.ia_proceso(
        9501,
        "python3",
        &format!("{}/app.slice/app-vortex-miku-9500.scope", m.raiz),
        &[
            &format!(
                "{}/home/.local/share/vortex/miku/natural/venv/bin/python",
                m.p().display()
            ),
            "-m",
            "miku.voz",
        ],
    );
    std::fs::write(
        m.proc.join("9501/maps"),
        "7f00-7f01 r-xp 0 0:0 0 /x/torch/lib/libtorch_cuda.so\n",
    )
    .unwrap();
    m.ia_proceso(
        9502,
        "ollama",
        &format!("{}/app.slice/vortex-ollama.service", m.raiz),
        &["/usr/bin/ollama", "runner", "--model", "/m", "--port", "4000"],
    );
    std::fs::write(m.p().join("nvsmi-apps"), "9501, 3000\n9502, 3400\n").unwrap();
    dormir(2500);
    assert!(
        !m.en_log("vortex.log", "modo-ia") && m.est("modo_ia=apagado"),
        "IA: Miku (su voz y su Ollama) no pone el modo IA"
    );
    assert!(
        !m.p().join("nvsmi.log").exists(),
        "IA: con solo Miku ni se le pregunta a nvidia-smi"
    );

    // llama-server en la CPU: parece IA, pero no sale en nvidia-smi ni abrió ninguna gráfica.
    m.ia_proceso(
        9601,
        "llama-server",
        &cg_kitty,
        &["/usr/bin/llama-server", "-m", "/m.gguf"],
    );
    esperar("se le pregunta a nvidia-smi por Vulkan", || {
        m.en_log("nvsmi.log", "-q -d PIDS")
    });
    dormir(1200);
    assert!(
        !m.en_log("vortex.log", "modo-ia"),
        "IA: llama-server en la CPU no cuenta"
    );
    assert!(
        m.en_log("nvsmi.log", "--query-compute-apps=pid,used_memory"),
        "IA: para confirmarlo, nvidia-smi (CUDA y luego Vulkan)"
    );
    // Con Vulkan no sale entre las apps de cómputo: solo en «nvidia-smi -q».
    std::fs::write(
        m.p().join("nvsmi-q"),
        "Processes\n    Process ID                        : 9601\n        Type                          : C+G\n        Used GPU Memory               : 2048 MiB\n",
    )
    .unwrap();
    esperar("llama.cpp por Vulkan", || {
        m.linea_exacta("vortex.log", "modo-ia on llama.cpp")
    });
    m.quitar_proceso(9601);
    std::fs::write(m.p().join("nvsmi-q"), "").unwrap();
    esperar("modo-ia off", || marca("modo-ia off") == 1);
    assert!(
        m.en_log("gestor.log", "llama.cpp ya no usa la gráfica"),
        "IA: termina llama.cpp: modo-ia off"
    );

    // Ollama con un modelo cargado, abierto desde la terminal (kitty).
    let sc_ia = m.scope("ia_Ollama", 9701);
    m.ia_proceso(
        9701,
        "ollama",
        &cg_kitty,
        &["/usr/bin/ollama", "runner", "--model", "/m", "--port", "4001"],
    );
    std::fs::write(m.p().join("nvsmi-apps"), "9501, 3000\n9502, 3400\n9701, 4096\n").unwrap();
    esperar("Ollama", || m.linea_exacta("vortex.log", "modo-ia on Ollama"));
    esperar("ia_pid=9701", || m.est("ia_pid=9701"));
    assert!(
        m.est("modo_ia=propio") && m.est("ia_programa=Ollama"),
        "IA: estado propio, Ollama, pid 9701"
    );
    assert!(
        sc_ia.is_dir() && m.leer(&sc_ia.join("cpu.weight")) == "500",
        "IA: Ollama a su propio scope con el peso de foco"
    );
    esperar("vortex-ia", || !m.archivo("vortex-ia.log").is_empty());
    assert!(
        m.linea_exacta("vortex-ia.log", "vigilar"),
        "IA: llama a vortex-ia vigilar"
    );
    m.hypr.activa(5001, "bbb2", "firefox", "none");
    m.hypr.evento(&["activewindowv2>>bbb2"]);
    esperar("firefox", || m.est("foco_pid=5001"));
    dormir(1200);
    assert!(
        marca("modo-ia on Ollama") == 1 && m.leer(&sc_ia.join("cpu.weight")) == "500",
        "IA: avisa una sola vez y el foco no le quita la prioridad"
    );

    // Empieza un juego: gana el modo juego.
    m.hypr.activa(6001, "ccc3", "steam_app_570", "none");
    m.hypr
        .evento(&["openwindow>>ccc3,1,steam_app_570,Dota 2", "activewindowv2>>ccc3"]);
    esperar("modo-ia off 2", || marca("modo-ia off") == 2);
    let v = m.archivo("vortex.log");
    let ult: Vec<&str> = v.lines().rev().take(2).collect();
    assert_eq!(
        ult,
        ["modo-ia off", "modo-juego on"],
        "IA: un juego: primero modo-juego on y luego modo-ia off"
    );
    assert_eq!(
        m.leer(&sc_ia.join("cpu.weight")),
        "50",
        "IA: con el juego, Ollama vuelve al peso de fondo"
    );
    dormir(1500);
    assert!(
        marca("modo-ia on") == 2 && m.est("modo_ia=apagado"),
        "IA: con el juego abierto el modo IA no vuelve"
    );
    m.hypr.evento(&["closewindow>>ccc3"]);
    esperar("el modo IA vuelve callado", || {
        m.linea_exacta("vortex.log", "modo-ia on Ollama --sin-aviso")
    });
    assert!(
        m.linea_exacta("vortex.log", "modo-juego off"),
        "IA: se cierra el juego y el modo IA vuelve, sin otro aviso"
    );
    esperar("Ollama recupera su prioridad", || {
        m.leer(&sc_ia.join("cpu.weight")) == "500"
    });

    // Ollama suelta el modelo (su runner termina).
    m.quitar_proceso(9701);
    std::fs::write(m.p().join("nvsmi-apps"), "9501, 3000\n9502, 3400\n").unwrap();
    esperar("modo-ia off 3", || marca("modo-ia off") == 3);
    esperar("peso de fondo", || m.leer(&sc_ia.join("cpu.weight")) == "50");
    assert!(
        m.est("modo_ia=apagado"),
        "IA: Ollama suelta el modelo: modo-ia off y su peso de fondo"
    );

    // AMD (sin NVIDIA): ComfyUI, por su carpeta y la VRAM de su fdinfo de DRM.
    std::fs::remove_dir_all(m.proc.join("driver")).unwrap();
    m.ia_proceso(9801, "python3", &cg_kitty, &["python", "main.py", "--listen"]);
    std::fs::create_dir_all(m.proc.join("9801/cwd/comfy")).unwrap();
    std::fs::write(
        m.proc.join("9801/fdinfo/5"),
        "pos:\t0\ndrm-driver:\tamdgpu\ndrm-client-id:\t7\ndrm-memory-vram:\t2097152 KiB\n",
    )
    .unwrap();
    esperar("ComfyUI", || m.linea_exacta("vortex.log", "modo-ia on ComfyUI"));
    // Modo juego a mano (Super+G) con el modo IA puesto: también manda.
    m.poner_marca("modo-juego");
    esperar("modo-ia off 4", || marca("modo-ia off") == 4);
    std::fs::remove_file(m.marca("modo-juego")).unwrap();
    esperar("vuelve callado", || {
        m.linea_exacta("vortex.log", "modo-ia on ComfyUI --sin-aviso")
    });
    m.quitar_proceso(9801);
    esperar("modo-ia off 5", || marca("modo-ia off") == 5);
    assert!(!m.marca("modo-ia").exists(), "IA: ComfyUI se cierra: modo-ia off");
}

#[test]
fn sigterm_y_systemd_que_no_deja_mover() {
    let mut m = Mundo::nuevo();
    for p in [9001, 9002, 9003] {
        m.proceso(p, "kitty", &m.sesion, "30", &[], &[]);
    }
    std::fs::write(m.p().join("busctl-falla"), "").unwrap();
    m.hypr.activa(9001, "a901", "kitty", "none");
    m.conf("DEPURAR=1\n");
    m.iniciar();
    esperar("9001", || m.est("foco_pid=9001"));
    for p in [9002, 9003] {
        m.hypr.activa(p, &format!("a{p}"), "kitty", "none");
        m.hypr.evento(&[&format!("activewindowv2>>a{p}")]);
        esperar("el foco cambia", || m.est(&format!("foco_pid={p}")));
    }
    assert!(
        m.en_log("gestor.log", "sigo sin moverlas"),
        "si systemd no deja mover: deja de intentarlo"
    );
    assert_eq!(m.nice_hilo(9003, 9003), -5, "y el nice sigue funcionando");
    // SIGTERM (systemctl stop): sale con 0 y devuelve el nice.
    let pid = m.pid_gestor();
    assert!(
        std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(m.esperar_salida(), Some(0), "SIGTERM: termina con 0");
    assert_eq!(m.nice_hilo(9003, 9003), 0, "SIGTERM: devuelve el nice");
    assert!(!m.estado_archivo().exists());
}

#[test]
fn sin_hyprland_sale_con_1() {
    let m = Mundo::nuevo();
    let vacio = m.p().join("vacio");
    std::fs::create_dir_all(&vacio).unwrap();
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_vortex-gestor-rs"))
        .env_clear()
        .env("XDG_RUNTIME_DIR", &vacio)
        .env("HOME", m.p().join("home"))
        .env("XDG_CONFIG_HOME", m.p().join("conf"))
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).contains("no encuentro Hyprland en"));
}

#[test]
fn presion_que_no_se_puede_vigilar() {
    use std::os::unix::fs::PermissionsExt;
    let mut m = Mundo::nuevo();
    // legible pero no se puede escribir el disparador: el kernel de verdad también lo rechazaría
    std::fs::set_permissions(m.p().join("psi"), std::fs::Permissions::from_mode(0o444)).unwrap();
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.proceso(4001, "kitty", &m.sesion, "30", &[], &[]);
    m.iniciar_con(false);
    esperar("el aviso de que no puede vigilar", || {
        m.est("psi=no (Permission denied)")
    });
    assert!(m.en_log(
        "gestor.log",
        "no puedo vigilar la presión de memoria: Permission denied"
    ));
}

#[test]
fn en_reposo_no_despierta_la_cpu() {
    let mut m = Mundo::nuevo();
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.proceso(4001, "kitty", &m.sesion, "30", &[], &[]);
    m.iniciar();
    esperar("arranque", || m.est("foco_pid=4001"));
    let ticks = |pid: u32| -> u64 {
        let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
        let c: Vec<&str> = s[s.rfind(") ").unwrap() + 2..].split_whitespace().collect();
        c[11].parse::<u64>().unwrap() + c[12].parse::<u64>().unwrap()
    };
    dormir(1000);
    let pid = m.pid_gestor();
    let t1 = ticks(pid);
    dormir(2500);
    assert_eq!(ticks(pid), t1, "sin despertar la CPU en reposo");
}

#[test]
fn reinicio_recupera_el_modo_juego_propio() {
    // El gestor anterior dejó puesto el modo juego y el fondo en pausa y ya no hay juegos: al primer foco
    // se deshace todo.
    let mut m = Mundo::nuevo();
    m.proceso(4001, "kitty", &m.sesion, "30", &[], &[]);
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    std::fs::create_dir_all(m.estado_archivo().parent().unwrap()).unwrap();
    std::fs::write(m.estado_archivo(), "modo_juego=propio\nfondo_pausado=1\n").unwrap();
    m.poner_marca("modo-juego");
    m.iniciar();
    esperar("modo-juego off", || {
        m.linea_exacta("vortex.log", "modo-juego off")
    });
    esperar("el fondo vuelve", || {
        m.en_log("qs.log", "call wallpaper setPlayback all play")
    });
    assert!(!m.marca("modo-juego").exists());
}

/// 4.4: al compartir la pantalla (Discord, OBS, el navegador) el shell calla las notificaciones, y al terminar
/// (o si el gestor se apaga a medias) las devuelve.
#[test]
fn compartir_pantalla_calla_las_notificaciones() {
    let mut m = mundo_base();
    *m.hypr.clientes.lock().unwrap() =
        r#"[{"address": "0xaaa1", "class": "kitty", "contentType": "none"}]"#.into();
    m.hypr.activa(4001, "aaa1", "kitty", "none");
    m.conf("DEPURAR=1\n");
    m.iniciar();
    esperar("kitty enfocada", || m.est("foco_pid=4001"));

    m.hypr.evento(&["screencast>>1,0"]);
    esperar("se avisa al shell que se comparte", || {
        m.en_log("qs.log", "call notificaciones compartiendo true")
    });
    assert!(m.est("compartiendo=1"), "{}", m.estado());
    // Repetido no vuelve a llamar al shell.
    m.hypr.evento(&["screencast>>1,1"]);
    dormir(300);
    assert_eq!(m.cuenta("qs.log", "compartiendo true"), 1);

    m.hypr.evento(&["screencast>>0,0"]);
    esperar("se le devuelven las notificaciones", || {
        m.en_log("qs.log", "call notificaciones compartiendo false")
    });
    assert!(m.est("compartiendo=0"), "{}", m.estado());

    // Si el gestor se apaga mientras se comparte, no deja al shell callado.
    m.hypr.evento(&["screencast>>1,0"]);
    esperar("compartiendo otra vez", || {
        m.cuenta("qs.log", "compartiendo true") == 2
    });
    m.hypr.cerrar();
    esperar("al salir devuelve las notificaciones", || {
        m.cuenta("qs.log", "compartiendo false") == 2
    });
}

#[test]
fn anota_las_apps_cerradas_para_reabrirlas() {
    let mut m = mundo_base();
    m.iniciar();
    let cerradas = m.run.join("vortex/cerradas");
    m.hypr.evento(&[
        "openwindow>>b001,1,firefox,Firefox",
        "openwindow>>b002,1,firefox,Otra ventana",
        "openwindow>>b003,1,org.gnome.Nautilus,Archivos",
        "openwindow>>b004,1,org.quickshell,Panel",
    ]);
    dormir(3200);
    // Un diálogo que vive menos de 3 s no cuenta.
    m.hypr
        .evento(&["openwindow>>b005,1,zenity,Pregunta", "closewindow>>b005"]);
    // Cerrar una de dos ventanas de Firefox no anota nada: la app sigue abierta.
    m.hypr.evento(&["closewindow>>b001", "closewindow>>b004"]);
    dormir(300);
    assert!(!cerradas.exists(), "{:?}", std::fs::read_to_string(&cerradas));
    m.hypr.evento(&["closewindow>>b003", "closewindow>>b002"]);
    esperar("se anotan las dos apps cerradas", || {
        std::fs::read_to_string(&cerradas).is_ok_and(|t| t == "org.gnome.Nautilus\nfirefox\n")
    });
}

#[test]
fn avisa_quien_usa_la_camara() {
    let mut m = mundo_base();
    let dev = m.p().join("dev");
    std::fs::create_dir_all(&dev).unwrap();
    std::fs::write(dev.join("video0"), "").unwrap();
    m.iniciar();
    let arch = m.run.join("vortex/camara");
    esperar("al arrancar, nadie la usa", || {
        std::fs::read_to_string(&arch).is_ok_and(|t| t.is_empty())
    });
    // Discord la abre (un proceso con un descriptor hacia la cámara).
    let pid = m.proc.join("7001");
    std::fs::create_dir_all(pid.join("fd")).unwrap();
    std::fs::write(pid.join("comm"), "Discord\n").unwrap();
    std::os::unix::fs::symlink(dev.join("video0"), pid.join("fd/20")).unwrap();
    drop(std::fs::File::open(dev.join("video0")).unwrap());
    esperar("se anota quién la usa", || {
        std::fs::read_to_string(&arch).is_ok_and(|t| t == "Discord\n")
    });
    // La suelta.
    std::fs::remove_file(pid.join("fd/20")).unwrap();
    drop(std::fs::File::open(dev.join("video0")).unwrap());
    esperar("ya nadie", || {
        std::fs::read_to_string(&arch).is_ok_and(|t| t.is_empty())
    });
    // Una cámara USB nueva también se vigila.
    std::fs::write(dev.join("video2"), "").unwrap();
    dormir(1800);
    std::os::unix::fs::symlink(dev.join("video2"), pid.join("fd/21")).unwrap();
    drop(std::fs::File::open(dev.join("video2")).unwrap());
    esperar("la nueva también", || {
        std::fs::read_to_string(&arch).is_ok_and(|t| t == "Discord\n")
    });
}
