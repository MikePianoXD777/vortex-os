//! El modo viaje y la línea de comandos, con programas falsos (nmcli, warp-cli, systemctl, notify-send) en
//! un PATH temporal, un servidor propio en 127.0.0.1 en vez de internet y una carpeta de configuración
//! temporal. Es lo de `test_interfaz.py` que no es la ventana (`estado` sin aceptar no toca WARP) más lo que
//! `viaje.py` no probaba.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use vortex_connect::motor::{Ajustes, Candado, Conexion, Connect, Situacion};
use vortex_connect::proteccion::{ErrorProteccion, Modo, Proteccion};
use vortex_connect::redes::{Actual, ErrorRed, Red, Redes};
use vortex_connect::sistema::Sistema;
use vortex_connect::viaje::{self, Avisos, Vigia, Vuelta};

static UNA_A_LA_VEZ: Mutex<()> = Mutex::new(());

fn guion(d: &Path, nombre: &str, cuerpo: &str) {
    let r = d.join(nombre);
    std::fs::write(&r, cuerpo).unwrap();
    std::fs::set_permissions(&r, std::fs::Permissions::from_mode(0o755)).unwrap();
}

const NMCLI: &str = r#"#!/bin/sh
case "$*" in
  *"TYPE,STATE"*) echo "wifi:connected" ;;
  *"-f TYPE device"*) echo "wifi" ;;
  *"wifi list"*) printf '*:Cafe\\: Centro:70:\n :Aeropuerto:40:\n :Casa:99:WPA2\n' ;;
  monitor) while true; do echo "cambio"; sleep 4; done ;;
  *) ;;
esac
"#;

/// Los guiones y las rutas de una prueba.
struct Casa {
    d: tempfile::TempDir,
    sistema: Sistema,
    _a_la_vez: std::sync::MutexGuard<'static, ()>,
}

impl Casa {
    fn nueva() -> Self {
        let a_la_vez = UNA_A_LA_VEZ.lock().unwrap_or_else(|e| e.into_inner());
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let ruta = |n: &str| p.join(n).to_string_lossy().into_owned();
        guion(p, "nmcli", NMCLI);
        guion(
            p,
            "warp-cli",
            &format!(
                r#"#!/bin/sh
echo "$*" >> "{reg}"
case "$*" in
  *status*) cat "{estado}" ;;
  *"registration show"*) echo "Account" ;;
  *" connect"*) echo "Status update: Connected" > "{estado}" ;;
  *disconnect*) echo "Status update: Disconnected" > "{estado}" ;;
esac
"#,
                reg = ruta("reg"),
                estado = ruta("estado"),
            ),
        );
        guion(
            p,
            "systemctl",
            &format!(
                "#!/bin/sh\necho \"$*\" >> \"{}\"\ncase \"$*\" in *is-active*) [ -f \"{}\" ] ;; *\" start\"*) : > \"{}\" ;; esac\n",
                ruta("reg-svc"),
                ruta("svc"),
                ruta("svc")
            ),
        );
        guion(p, "resolvectl", "#!/bin/sh\nexit 0\n");
        guion(p, "ip", "#!/bin/sh\nexit 0\n");
        guion(
            p,
            "notify-send",
            &format!("#!/bin/sh\necho \"$*\" >> \"{}\"\n", ruta("avisos")),
        );
        std::fs::write(p.join("estado"), "Status update: Disconnected\n").unwrap();
        let sistema = Sistema::con_path(format!("{}:/usr/bin:/bin", p.display()));
        Casa {
            d,
            sistema,
            _a_la_vez: a_la_vez,
        }
    }

    fn p(&self, n: &str) -> PathBuf {
        self.d.path().join(n)
    }

    fn leer(&self, n: &str) -> String {
        std::fs::read_to_string(self.p(n)).unwrap_or_default()
    }

    fn ajustes(&self) -> PathBuf {
        self.p("config/vortex/connect.json")
    }
}

/// Un «internet» propio: /204 da 204 y /traza dice «warp=on» cuando el warp-cli falso está conectado.
fn servidor(estado: PathBuf) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let estado = estado.clone();
            std::thread::spawn(move || {
                let mut b = [0u8; 2048];
                let n = s.read(&mut b).unwrap_or(0);
                let pedido = String::from_utf8_lossy(&b[..n]);
                let ruta = pedido.split_whitespace().nth(1).unwrap_or("/").to_string();
                let r = if ruta == "/traza" {
                    let t = std::fs::read_to_string(&estado).unwrap_or_default();
                    let on = t.contains("Connected") && !t.contains("Disc");
                    format!(
                        "HTTP/1.0 200 OK\r\n\r\n{}\n",
                        if on { "warp=on" } else { "warp=off" }
                    )
                } else {
                    "HTTP/1.0 204 No Content\r\n\r\n".to_string()
                };
                let _ = s.write_all(r.as_bytes());
            });
        }
    });
    base
}

fn comando(c: &Casa, exe: &str, base: &str) -> Command {
    let mut cmd = Command::new(exe);
    cmd.env_clear()
        .env("PATH", c.sistema.path.clone().unwrap())
        .env("HOME", c.d.path())
        .env("LANG", "C.UTF-8")
        .env("XDG_CONFIG_HOME", c.p("config"))
        .env("XDG_STATE_HOME", c.p("estado-xdg"))
        .env("BROWSER", "true")
        .env("VORTEX_CONNECT_PRUEBA", format!("{base}/204"))
        .env("VORTEX_CONNECT_TRAZA", format!("{base}/traza"))
        .env("VORTEX_CONNECT_COLORES", c.p("no-hay.json"))
        .stdin(Stdio::null());
    cmd
}

// --- La línea de comandos ---

#[test]
fn estado_sin_aceptar_no_toca_warp() {
    let c = Casa::nueva();
    let base = servidor(c.p("estado"));
    let r = comando(&c, env!("CARGO_BIN_EXE_vortex-connect-motor"), &base)
        .arg("estado")
        .output()
        .unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert_eq!(
        String::from_utf8_lossy(&r.stdout).trim(),
        "sin_proteccion wifi Cafe: Centro"
    );
    assert!(!c.p("reg").exists(), "warp-cli no se llama sin aceptar");
    assert!(
        !c.leer("reg-svc").contains("start"),
        "el servicio de WARP no se prende"
    );
}

#[test]
fn redes_lista_las_abiertas() {
    let c = Casa::nueva();
    let base = servidor(c.p("estado"));
    let r = comando(&c, env!("CARGO_BIN_EXE_vortex-connect-motor"), &base)
        .arg("redes")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&r.stdout),
        " 70 %  * Cafe: Centro\n 40 %    Aeropuerto\n"
    );
}

#[test]
fn version_y_orden_desconocida() {
    let c = Casa::nueva();
    let exe = env!("CARGO_BIN_EXE_vortex-connect-motor");
    let r = comando(&c, exe, "http://127.0.0.1:9")
        .arg("--version")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&r.stdout),
        "Vortexine Connect 1.0.0 (Rust)\n"
    );
    let r = comando(&c, exe, "http://127.0.0.1:9")
        .arg("bailar")
        .output()
        .unwrap();
    assert_eq!(r.status.code(), Some(2));
}

#[test]
fn vigilar_apagado_sale_de_una() {
    // Sin modo viaje (o sin condiciones aceptadas) el servicio termina sin tocar nada.
    let c = Casa::nueva();
    let base = servidor(c.p("estado"));
    for texto in [
        None,
        Some(r#"{"modo_viaje": true}"#),
        Some(r#"{"condiciones": true}"#),
    ] {
        if let Some(t) = texto {
            std::fs::create_dir_all(c.ajustes().parent().unwrap()).unwrap();
            std::fs::write(c.ajustes(), t).unwrap();
        }
        let r = comando(&c, env!("CARGO_BIN_EXE_vortex-connect-motor"), &base)
            .arg("vigilar")
            .output()
            .unwrap();
        assert_eq!(r.status.code(), Some(0));
    }
    assert!(!c.p("reg").exists());
    assert!(!c.p("avisos").exists());
}

#[test]
fn el_servicio_de_viaje_cifra_avisa_y_se_apaga_solo() {
    let c = Casa::nueva();
    let base = servidor(c.p("estado"));
    std::fs::create_dir_all(c.ajustes().parent().unwrap()).unwrap();
    std::fs::write(c.ajustes(), r#"{"condiciones": true, "modo_viaje": true}"#).unwrap();
    let mut hijo = comando(&c, env!("CARGO_BIN_EXE_vortex-connect-motor"), &base)
        .arg("vigilar")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let limite = Instant::now() + Duration::from_secs(40);
    while !c.leer("avisos").contains("va cifrado") && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    let avisos = c.leer("avisos");
    assert!(
        avisos.contains("Entraste a «Cafe: Centro» y lo que mandas va cifrado."),
        "avisos: {avisos:?}"
    );
    assert!(avisos.contains("-u normal"));
    assert!(c.leer("reg").contains("--accept-tos mode warp"));
    // la persona lo apaga: el servicio se entera en la siguiente vuelta y termina
    std::fs::write(
        c.ajustes(),
        r#"{"condiciones": true, "modo_viaje": false, "protegio_connect": true}"#,
    )
    .unwrap();
    let fin = Instant::now() + Duration::from_secs(30);
    let codigo = loop {
        if let Some(s) = hijo.try_wait().unwrap() {
            break s.code();
        }
        assert!(Instant::now() < fin, "el servicio no terminó");
        std::thread::sleep(Duration::from_millis(100));
    };
    assert_eq!(codigo, Some(0));
    let mut dicho = String::new();
    hijo.stdout.take().unwrap().read_to_string(&mut dicho).unwrap();
    assert!(dicho.contains("Protegido: lo que mandas va cifrado."), "{dicho}");
}

// --- Una vuelta (con falsos en memoria) ---

struct RedFija(Actual);

impl Redes for RedFija {
    fn actual(&self) -> Actual {
        self.0.clone()
    }
    fn lista(&self, _: bool) -> Result<Vec<Red>, ErrorRed> {
        Ok(vec![])
    }
    fn hay_wifi(&self) -> bool {
        true
    }
    fn conectar(&self, _: &str) -> Result<(), ErrorRed> {
        Ok(())
    }
}

struct WarpQueCifra(bool);

impl Proteccion for WarpQueCifra {
    fn instalado(&self) -> bool {
        true
    }
    fn conectado(&self) -> bool {
        false
    }
    fn modo(&self) -> Modo {
        Modo::Ninguno
    }
    fn preparar(&self) -> Result<(), ErrorProteccion> {
        Ok(())
    }
    fn activar_cifrado(&self) -> bool {
        self.0
    }
    fn activar_solo_dns(&self) -> bool {
        false
    }
    fn blindar_dns(&self) -> bool {
        true
    }
    fn apagar(&self) {}
}

fn vigia(c: &Casa, cifra: bool, ajustes: Ajustes) -> Vigia {
    let connect = Connect::nuevo(
        Box::new(RedFija(Actual::wifi("Cafe", true))),
        Box::new(WarpQueCifra(cifra)),
        Box::new(|| (Conexion::Internet, String::new())),
        Box::new(|_| {}),
        ajustes,
    )
    .con_ruta_ajustes(c.ajustes());
    connect.cambiar_ajustes(|_| {}); // deja el archivo igual a lo que se le dio
    Vigia::new(
        connect,
        Avisos::new(c.sistema.clone()),
        Candado::en(c.p("connect.candado")),
    )
}

fn viaje_prendido() -> Ajustes {
    Ajustes {
        condiciones: true,
        modo_viaje: true,
        protegio_connect: false,
    }
}

#[test]
fn una_vuelta_cifra_y_avisa() {
    let c = Casa::nueva();
    let mut v = vigia(&c, true, viaje_prendido());
    let Vuelta::Hecha(e) = v.una_vuelta() else {
        panic!("tenía que cifrar")
    };
    assert_eq!(e.situacion, Situacion::Protegido);
    assert!(
        c.leer("avisos")
            .contains("Entraste a «Cafe» y lo que mandas va cifrado.")
    );
}

#[test]
fn si_no_pudo_no_insiste_con_cada_cambio() {
    let c = Casa::nueva();
    let mut v = vigia(&c, false, viaje_prendido());
    assert!(matches!(v.una_vuelta(), Vuelta::Hecha(e) if e.situacion == Situacion::SinProteccion));
    // el enfriamiento (5 min): ahora no hace nada aunque cambie la red
    assert_eq!(v.una_vuelta(), Vuelta::Nada);
}

#[test]
fn si_el_modo_viaje_se_apaga_termina() {
    let c = Casa::nueva();
    let mut v = vigia(&c, true, viaje_prendido());
    Ajustes::default().guardar_en(&c.ajustes());
    assert_eq!(v.una_vuelta(), Vuelta::Terminar);
}

#[test]
fn si_la_ventana_esta_trabajando_espera() {
    let c = Casa::nueva();
    let mut v = vigia(&c, true, viaje_prendido());
    let mut ventana = Candado::en(c.p("connect.candado"));
    assert!(ventana.tomar());
    assert_eq!(v.una_vuelta(), Vuelta::Nada);
    ventana.soltar();
    assert!(matches!(v.una_vuelta(), Vuelta::Hecha(_)));
}

// --- Encender y apagar el servicio ---

#[test]
fn prender_y_apagar_el_servicio_del_usuario() {
    let c = Casa::nueva();
    assert!(viaje::prender(&c.sistema, true));
    assert!(viaje::prender(&c.sistema, false));
    assert_eq!(
        c.leer("reg-svc").lines().collect::<Vec<_>>(),
        [
            "--user enable --now vortex-connect-viaje.service",
            "--user disable --now vortex-connect-viaje.service"
        ]
    );
    // un systemctl que falla: lo dice
    guion(c.d.path(), "systemctl", "#!/bin/sh\nexit 1\n");
    assert!(!viaje::prender(&c.sistema, true));
}
