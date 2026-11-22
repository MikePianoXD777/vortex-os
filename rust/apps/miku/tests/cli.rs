//! El comando `vortex-miku` en Rust, corrido de verdad (`vortex-miku-rs`) con un HOME de prueba: estado, avatar,
//! internet, modelo, voz (con su descarga desde un servidor falso), ia, cámara, compañera, platicadora,
//! habla-sola, sentimientos, grabar, sentir, alternar (abrir y cerrar del todo una «app» falsa) y el paso a Python
//! de natural y elevenlabs (con un `python3` falso). Ninguna prueba toca la configuración, el llavero ni la
//! pantalla de nadie: PATH solo trae los programas falsos de cada prueba.
//!
//! Son las de `apps/miku/pruebas/comando.py`, `companera.py` y `ia.py`, escritas para el comando de Rust.

mod comun;

use serde_json::{Value, json};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use vortex_miku::sha256;

struct Caja {
    _d: tempfile::TempDir,
    base: PathBuf,
    home: PathBuf,
    bin: PathBuf,
    runtime: PathBuf,
    extra: Vec<(String, String)>,
}

fn raiz_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../apps/miku")
}

fn escribir_exe(ruta: &Path, texto: &str) {
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(ruta, texto).unwrap();
    std::fs::set_permissions(ruta, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Caja {
    fn nueva() -> Self {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().to_path_buf();
        let (home, bin, runtime) = (base.join("home"), base.join("bin"), base.join("run"));
        for c in [&home, &bin, &runtime] {
            std::fs::create_dir_all(c).unwrap();
        }
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        // Los programas de verdad que usan los falsos (y solo esos): el PATH de la prueba no trae nada más.
        for n in ["sleep", "cp", "curl"] {
            let real = ["/usr/bin", "/bin"]
                .iter()
                .map(|d| Path::new(d).join(n))
                .find(|p| p.exists())
                .unwrap();
            std::os::unix::fs::symlink(real, bin.join(n)).unwrap();
        }
        Self {
            _d: d,
            base,
            home,
            bin,
            runtime,
            extra: vec![],
        }
    }

    fn config(&self) -> PathBuf {
        self.home.join(".config/vortex/miku.json")
    }

    fn leer_config(&self) -> Value {
        serde_json::from_str(&std::fs::read_to_string(self.config()).unwrap()).unwrap()
    }

    fn escribir_config(&self, texto: &str) {
        std::fs::create_dir_all(self.config().parent().unwrap()).unwrap();
        std::fs::write(self.config(), texto).unwrap();
    }

    fn env(&mut self, k: &str, v: impl AsRef<str>) {
        self.extra.push((k.into(), v.as_ref().into()));
    }

    /// Un programa falso en el PATH de la prueba (un script de sh).
    fn falso(&self, nombre: &str, cuerpo: &str) {
        escribir_exe(&self.bin.join(nombre), &format!("#!/bin/sh\n{cuerpo}\n"));
    }

    fn orden(&self, args: &[&str]) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_vortex-miku-rs"));
        c.args(args)
            .env_clear()
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_DATA_HOME", self.home.join(".local/share"))
            .env("XDG_STATE_HOME", self.home.join(".local/state"))
            .env("XDG_RUNTIME_DIR", &self.runtime)
            .env("PATH", &self.bin)
            .env("VORTEX_MIKU_RAIZ", raiz_repo())
            .env("VORTEX_MIKU_CORAZON", self.home.join("Miku"))
            .env("VORTEX_MIKU_VIDEOS", self.base.join("videos"))
            .env("VORTEX_MIKU_MARCA_INTERNET", self.base.join("no-hay-marca"))
            .env("VORTEX_MIKU_MODELO_SISTEMA", self.base.join("no-hay-modelo.vrm"))
            .stdin(Stdio::null());
        for (k, v) in &self.extra {
            c.env(k, v);
        }
        c
    }

    /// (código, stdout, stderr)
    fn correr(&self, args: &[&str]) -> (i32, String, String) {
        let o = self.orden(args).output().unwrap();
        (
            o.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&o.stdout).into_owned(),
            String::from_utf8_lossy(&o.stderr).into_owned(),
        )
    }

    /// Una «app» de Miku falsa: un script llamado miku-rs (así la reconoce el comando) que anota con qué
    /// variables la abrieron y se queda dormida, con un hijo (como la voz) que también tiene que morir.
    fn app_falsa(&mut self) -> PathBuf {
        let app = self.base.join("falsa/miku-rs");
        let anota = self.base.join("abierta.log");
        escribir_exe(
            &app,
            &format!(
                "#!/bin/sh\necho \"visita=$VORTEX_MIKU_VISITA companera=$VORTEX_MIKU_COMPANERA voz=$VORTEX_MIKU_VOZ\" >> {}\nsleep 300 &\nwait\n",
                anota.display()
            ),
        );
        self.env("VORTEX_MIKU_APP", app.to_string_lossy());
        app
    }

    fn aperturas(&self) -> Vec<String> {
        std::fs::read_to_string(self.base.join("abierta.log"))
            .map(|t| t.lines().map(String::from).collect())
            .unwrap_or_default()
    }
}

fn esperar(limite: f64, mut cond: impl FnMut() -> bool) -> bool {
    let fin = Instant::now() + Duration::from_secs_f64(limite);
    while Instant::now() < fin {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

fn vivo(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .map(|t| !t.rsplit(')').next().unwrap_or("").trim_start().starts_with('Z'))
        .unwrap_or(false)
}

// ---- estado, avatar, internet ---------------------------------------------------------------------

#[test]
fn estado_de_fabrica_y_json_roto() {
    let c = Caja::nueva();
    let (cod, sal, _) = c.correr(&["estado"]);
    assert_eq!(
        (cod, sal.as_str()),
        (0, "avatar=on\nabierta=no\nplaticadora=on\n")
    );
    c.escribir_config("{esto no es json");
    let (cod, sal, _) = c.correr(&["estado"]);
    assert_eq!(
        (cod, sal.as_str()),
        (0, "avatar=on\nabierta=no\nplaticadora=on\n"),
        "un JSON inválido no truena"
    );
    let (cod, sal, _) = c.correr(&["avatar", "off"]);
    assert_eq!((cod, sal.as_str()), (0, "avatar=off\n"));
    assert_eq!(
        c.leer_config(),
        json!({"avatar": false}),
        "y se reescribe bien al guardar"
    );
}

#[test]
fn avatar_conserva_las_otras_claves_y_valida() {
    let c = Caja::nueva();
    c.escribir_config(r#"{"voz": "supertonic-f2", "otra": 1, "avatar": false}"#);
    assert_eq!(c.correr(&["avatar", "on"]).0, 0);
    assert_eq!(
        c.leer_config(),
        json!({"voz": "supertonic-f2", "otra": 1, "avatar": true})
    );
    let (cod, _, err) = c.correr(&["avatar", "quizas"]);
    assert_eq!(cod, 2);
    assert_eq!(err, "vortex-miku: uso: vortex-miku avatar on|off\n");
    for (dicho, valor) in [("sí", true), ("NO", false), ("1", true), ("false", false)] {
        c.correr(&["avatar", dicho]);
        assert_eq!(c.leer_config()["avatar"], json!(valor), "{dicho}");
    }
}

#[test]
fn internet_estado_y_marca_del_instalador() {
    let mut c = Caja::nueva();
    assert_eq!(c.correr(&["internet"]).1, "internet=off\n");
    let marca = c.base.join("marca");
    std::fs::write(&marca, "").unwrap();
    c.env("VORTEX_MIKU_MARCA_INTERNET", marca.to_string_lossy());
    assert_eq!(
        c.correr(&["internet", "estado"]).1,
        "internet=on\n",
        "manda la casilla del instalador"
    );
    let (cod, sal, _) = c.correr(&["internet", "off"]);
    assert_eq!((cod, sal.as_str()), (0, "internet=off\n"));
    assert_eq!(
        c.correr(&["internet"]).1,
        "internet=off\n",
        "y lo dicho en miku.json manda sobre la marca"
    );
    let (_, sal, _) = c.correr(&["internet", "on"]);
    assert_eq!(
        sal,
        "internet=on\nMiku podrá buscar en la web y leer páginas; cada vez te pedirá permiso en su ventana.\n"
    );
    assert_eq!(c.correr(&["internet", "a", "b"]).0, 2);
}

// ---- modelo ---------------------------------------------------------------------------------------

#[test]
fn modelo_valida_copia_y_cambia_el_credito() {
    let c = Caja::nueva();
    let destino = c.home.join(".local/share/vortex/miku/modelo.vrm");
    let malo = c.base.join("malo.vrm");
    std::fs::write(&malo, b"no soy un modelo".repeat(10)).unwrap();
    let (cod, _, err) = c.correr(&["modelo", malo.to_str().unwrap()]);
    assert_eq!(cod, 1);
    assert!(
        err.contains("no parece un modelo VRM") && err.contains("«malo.vrm»"),
        "{err}"
    );
    assert!(!destino.exists(), "y no deja nada copiado");
    let (cod, _, err) = c.correr(&["modelo", c.base.join("no-existe.vrm").to_str().unwrap()]);
    assert_eq!(cod, 1);
    assert!(
        err.contains("no puedo leer") && err.contains("No such file or directory"),
        "{err}"
    );
    assert_eq!(c.correr(&["modelo"]).0, 2);

    let bueno = c.base.join("con espacios/otro modelo.vrm");
    std::fs::create_dir_all(bueno.parent().unwrap()).unwrap();
    let mut datos = b"glTF\x02\0\0\0".to_vec();
    datos.extend([0u8; 100]);
    std::fs::write(&bueno, &datos).unwrap();
    std::fs::write(
        bueno.parent().unwrap().join("CREDITO.txt"),
        "Modelo de prueba por nadie\n",
    )
    .unwrap();
    c.escribir_config(r#"{"modelo": "/otro/viejo.vrm"}"#);
    let (cod, sal, _) = c.correr(&["modelo", bueno.to_str().unwrap()]);
    assert_eq!(cod, 0);
    assert_eq!(sal, format!("Modelo listo: {}\n", destino.display()));
    assert_eq!(std::fs::read(&destino).unwrap(), datos);
    assert_eq!(
        std::fs::read_to_string(destino.parent().unwrap().join("CREDITO.txt")).unwrap(),
        "Modelo de prueba por nadie\n"
    );
    assert_eq!(
        c.leer_config()["modelo"],
        "",
        "el «modelo» de miku.json no tapa al nuevo"
    );

    // Otro modelo sin crédito: borra el del anterior y avisa.
    let sin = c.base.join("sin/m.vrm");
    std::fs::create_dir_all(sin.parent().unwrap()).unwrap();
    std::fs::write(&sin, &datos).unwrap();
    let (cod, sal, _) = c.correr(&["modelo", sin.to_str().unwrap()]);
    assert_eq!(cod, 0);
    assert!(
        sal.contains("Aviso: no hay un CREDITO.txt junto al modelo"),
        "{sal}"
    );
    assert!(!destino.parent().unwrap().join("CREDITO.txt").exists());
    // El mismo archivo ya instalado: no se copia sobre sí mismo.
    let (cod, sal, _) = c.correr(&["modelo", destino.to_str().unwrap()]);
    assert_eq!(cod, 0);
    assert!(!sal.contains("Aviso"), "{sal}");
    assert_eq!(std::fs::read(&destino).unwrap(), datos);
}

// ---- voz ------------------------------------------------------------------------------------------

#[test]
fn voz_lista_elige_y_baja_la_opcional_con_su_sha256() {
    let mut c = Caja::nueva();
    let (cod, sal, _) = c.correr(&["voz"]);
    assert_eq!(cod, 0);
    assert!(sal.starts_with("Voces de Miku (la elegida lleva *):\n"), "{sal}");
    assert!(
        sal.contains("* supertonic-f1  Miku (F1)  [se baja con el modelo la primera vez que abras a Miku]"),
        "{sal}"
    );
    assert!(
        sal.contains("  miku-natural   ") && sal.contains("[instálala con: vortex-miku natural instalar]"),
        "{sal}"
    );
    assert!(
        sal.contains("ElevenLabs") && sal.contains("[ponla con: vortex-miku elevenlabs VOICE_ID]"),
        "{sal}"
    );
    assert!(
        sal.contains("[se baja al elegirla (~290 KB)]") && sal.contains("OpenRAIL-M"),
        "{sal}"
    );
    // Sin modelo ni F1 ya bajados.
    let (cod, sal, _) = c.correr(&["voz", "supertonic-f1"]);
    assert_eq!(cod, 0);
    assert!(
        sal.starts_with("Voz elegida: Miku (F1)\nAún no está bajado el modelo de voz"),
        "{sal}"
    );
    assert_eq!(c.leer_config()["voz"], "supertonic-f1");
    let (cod, _, err) = c.correr(&["voz", "no-existe"]);
    assert_eq!(cod, 1);
    assert_eq!(
        err,
        "vortex-miku: no existe la voz «no-existe». Mira las que hay con: vortex-miku voz\n"
    );
    let (cod, _, err) = c.correr(&["voz", "miku-natural"]);
    assert_eq!(cod, 1);
    assert!(err.contains("la voz natural no está instalada"), "{err}");
    let (cod, _, err) = c.correr(&["voz", "elevenlabs"]);
    assert_eq!(cod, 1);
    assert!(err.contains("primero pon tu voz y tu clave"), "{err}");
    assert_eq!(c.correr(&["voz", "a", "b"]).0, 2);

    // Una voz opcional baja su archivito (de un servidor falso) y lo comprueba.
    let falso = b"{\"style_ttl\": {}, \"style_dp\": {}}".repeat(20);
    let cuerpo = falso.clone();
    let srv = comun::servidor(move |_| (200, "application/json", cuerpo.clone()));
    let mut cat: Value =
        serde_json::from_str(&std::fs::read_to_string(raiz_repo().join("voces.json")).unwrap()).unwrap();
    for v in cat["voces"].as_array_mut().unwrap() {
        if v["id"] == "supertonic-f2" {
            v["archivo"]["url"] = json!(format!("{}/F2.json", srv.url));
            v["archivo"]["bytes"] = json!(falso.len());
            v["archivo"]["sha256"] = json!(sha256::de_bytes(&falso));
        }
    }
    let catalogo = c.base.join("voces.json");
    std::fs::write(&catalogo, cat.to_string()).unwrap();
    c.env("VORTEX_MIKU_CATALOGO", catalogo.to_string_lossy());
    let f2 = c.home.join(".local/share/vortex/miku/voz/voice_styles/F2.json");
    let (cod, sal, err) = c.correr(&["voz", "supertonic-f2"]);
    assert_eq!(cod, 0, "{err}");
    assert!(sal.starts_with("Voz elegida: Voz F2\n"), "{sal}");
    assert_eq!(std::fs::read(&f2).unwrap(), falso);
    assert_eq!(c.leer_config()["voz"], "supertonic-f2");

    // Si llega distinta (sha256), no se instala ni se elige.
    std::fs::remove_file(&f2).unwrap();
    c.correr(&["voz", "supertonic-f1"]);
    for v in cat["voces"].as_array_mut().unwrap() {
        if v["id"] == "supertonic-f2" {
            v["archivo"]["sha256"] = json!("0".repeat(64));
        }
    }
    std::fs::write(&catalogo, cat.to_string()).unwrap();
    let (cod, _, err) = c.correr(&["voz", "supertonic-f2"]);
    assert_eq!(cod, 1);
    assert!(err.contains("sha256") && !f2.exists(), "{err}");
    assert_eq!(c.leer_config()["voz"], "supertonic-f1");
}

/// Un manifiesto de pruebas con un archivo chico por paquete (oído y voz).
fn manifiesto(c: &mut Caja, url: &str, contenido: &[u8]) {
    let archivo = |nombre: &str| {
        json!({"nombre": nombre, "archivos": [{"ruta": "a.bin", "url": format!("{url}/a.bin"),
            "bytes": contenido.len(), "sha256": sha256::de_bytes(contenido)}]})
    };
    let m = json!({"paquetes": {"oido": archivo("el oído"), "voz": archivo("la voz")}});
    let ruta = c.base.join("manifiesto.json");
    std::fs::write(&ruta, m.to_string()).unwrap();
    c.env("VORTEX_MIKU_MANIFIESTO", ruta.to_string_lossy());
}

#[test]
fn preparar_baja_y_luego_dice_que_ya_esta() {
    let mut c = Caja::nueva();
    let contenido = b"modelo de mentira".to_vec();
    let cuerpo = contenido.clone();
    let srv = comun::servidor(move |_| (200, "application/octet-stream", cuerpo.clone()));
    manifiesto(&mut c, &srv.url, &contenido);
    let (cod, sal, err) = c.correr(&["preparar"]);
    assert_eq!(cod, 0, "{err}");
    assert!(
        sal.starts_with("Preparando la voz de Miku (una sola vez, ~560 MB)…\n"),
        "{sal}"
    );
    assert!(sal.contains("100%"), "{sal}");
    assert!(
        sal.contains("\nListo: la voz y el oído de Miku están en "),
        "{sal}"
    );
    let datos = c.home.join(".local/share/vortex/miku");
    assert_eq!(std::fs::read(datos.join("voz/a.bin")).unwrap(), contenido);
    assert_eq!(std::fs::read(datos.join("oido/a.bin")).unwrap(), contenido);
    let (cod, sal, _) = c.correr(&["preparar"]);
    assert_eq!(
        (cod, sal.as_str()),
        (0, "La voz y el oído de Miku ya están listos.\n")
    );
}

#[test]
fn preparar_con_un_archivo_distinto_falla_sin_instalarlo() {
    let mut c = Caja::nueva();
    let srv = comun::servidor(|_| (200, "application/octet-stream", b"otra cosa distinta".to_vec()));
    manifiesto(&mut c, &srv.url, b"lo que se esperaba");
    let (cod, _, err) = c.correr(&["preparar"]);
    assert_eq!(cod, 1);
    assert!(err.starts_with("vortex-miku: "), "{err}");
    assert!(!c.home.join(".local/share/vortex/miku/voz/a.bin").exists());
}

#[test]
fn licencias_ayuda_y_orden_desconocida() {
    let c = Caja::nueva();
    let (cod, sal, _) = c.correr(&["licencias"]);
    assert_eq!(cod, 0);
    let esperado = std::fs::read_to_string(raiz_repo().join("licencias/CREDITOS.txt")).unwrap();
    assert_eq!(sal, format!("{esperado}\n"));
    assert_eq!(c.correr(&["créditos"]).1, sal);
    let (cod, sal, _) = c.correr(&["ayuda"]);
    assert_eq!(cod, 0);
    assert!(
        sal.starts_with(
            "vortex-miku: Miku, la asistente de voz de Vortex OS (SUPER + O).\n\n  vortex-miku  "
        ),
        "{sal}"
    );
    assert!(sal.contains("vortex-miku companera on|off|estado|tono"), "{sal}");
    assert_eq!(c.correr(&["--help"]).1, sal);
    let (cod, _, err) = c.correr(&["bailar"]);
    assert_eq!(cod, 2);
    assert_eq!(err, "vortex-miku: no conozco «bailar». Mira: vortex-miku ayuda\n");
}

#[test]
fn decir_a_un_wav_con_la_voz_falsa_y_sus_errores() {
    let mut c = Caja::nueva();
    // Sin el modelo de voz bajado.
    let (cod, _, err) = c.correr(&["decir", "hola"]);
    assert_eq!(cod, 1);
    assert!(
        err.contains("el modelo de voz aún no está bajado. Bájalo con: vortex-miku preparar"),
        "{err}"
    );
    assert_eq!(c.correr(&["decir"]).0, 2);
    assert_eq!(c.correr(&["decir", "hola", "-o"]).0, 2);
    // Con el modelo (ya bajado) y un python3 falso que escribe el WAV pedido y anota lo que le pasaron.
    let contenido = b"x".to_vec();
    let srv = comun::servidor(|_| (200, "x", vec![]));
    manifiesto(&mut c, &srv.url, &contenido);
    let datos = c.home.join(".local/share/vortex/miku");
    for p in ["voz", "oido"] {
        std::fs::create_dir_all(datos.join(p)).unwrap();
        std::fs::write(datos.join(p).join("a.bin"), &contenido).unwrap();
    }
    let anota = c.base.join("python.log");
    c.falso(
        "python3",
        &format!(
            "echo \"$@\" >> {}\nwhile [ $# -gt 0 ]; do [ \"$1\" = --salida ] && s=\"$2\"; shift; done\necho wav > \"$s\"",
            anota.display()
        ),
    );
    let wav = c.base.join("hola.wav");
    // Con la voz de Python (VORTEX_MIKU_VOZ_PYTHON) y con la de Rust (miku-voz, otro falso que hace lo mismo).
    c.env("VORTEX_MIKU_VOZ_PYTHON", "1");
    let (cod, sal, err) = c.correr(&["decir", "hola", "mundo", "-o", wav.to_str().unwrap()]);
    assert_eq!(cod, 0, "{err}");
    assert_eq!(sal, format!("Listo: {}\n", wav.display()));
    assert!(wav.is_file());
    let llamada = std::fs::read_to_string(&anota).unwrap();
    assert!(
        llamada.contains("voz_worker.py --voz supertonic-f1 --estilo miku --salida")
            && llamada.trim_end().ends_with("hola mundo"),
        "{llamada}"
    );
    c.extra.retain(|(k, _)| k != "VORTEX_MIKU_VOZ_PYTHON");
    std::fs::remove_file(&anota).unwrap();
    std::fs::remove_file(&wav).unwrap();
    let rust = c.bin.join("miku-voz");
    std::fs::copy(c.bin.join("python3"), &rust).unwrap();
    c.env("VORTEX_MIKU_VOZ", rust.to_str().unwrap());
    let (cod, _, err) = c.correr(&["decir", "hola", "mundo", "-o", wav.to_str().unwrap()]);
    assert_eq!(cod, 0, "{err}");
    assert!(wav.is_file());
    let llamada = std::fs::read_to_string(&anota).unwrap();
    assert!(
        llamada.starts_with("--voz supertonic-f1 --estilo miku --salida")
            && llamada.trim_end().ends_with("hola mundo"),
        "{llamada}"
    );
    // «--voz ID»: otra voz solo esta vez (la compañera usa la rápida) y no se dice en voz alta.
    std::fs::remove_file(&anota).unwrap();
    let (cod, _, err) = c.correr(&["decir", "--voz", "supertonic-f3", "hola", "-o", wav.to_str().unwrap()]);
    assert_eq!(cod, 0, "{err}");
    let llamada = std::fs::read_to_string(&anota).unwrap();
    assert!(
        llamada.starts_with("--voz supertonic-f3 ") && llamada.trim_end().ends_with(&format!("--salida {} hola", wav.display())),
        "{llamada}"
    );
    let (cod, _, err) = c.correr(&["decir", "--voz", "nadie", "hola"]);
    assert_eq!(cod, 2);
    assert!(err.contains("no conozco la voz «nadie»"), "{err}");
    assert_eq!(c.correr(&["decir", "hola", "--voz"]).0, 2);
    // Sin dónde reproducir (PATH sin pw-play/paplay/aplay).
    let (cod, _, err) = c.correr(&["decir", "hola"]);
    assert_eq!(cod, 1);
    assert!(
        err.contains("no encuentro con qué reproducir sonido (pw-play)."),
        "{err}"
    );
    // Con reproductor falso: suena y el WAV temporal se borra.
    let toco = c.base.join("toco");
    c.falso("pw-play", &format!("cp \"$1\" {}", toco.display()));
    assert_eq!(c.correr(&["decir", "hola"]).0, 0);
    assert!(toco.is_file());
    assert!(
        std::fs::read_dir(c.runtime.join("vortex-miku"))
            .map(|d| d.count() == 0)
            .unwrap_or(true)
    );
}

#[test]
fn natural_y_elevenlabs_se_pasan_a_python_con_su_codigo_de_salida() {
    let c = Caja::nueva();
    let anota = c.base.join("py.log");
    c.falso("python3", &format!("echo \"$@\" >> {}\nexit 7", anota.display()));
    let script = c.base.join("vortex-miku-python");
    std::fs::write(&script, "").unwrap();
    let mut c = c;
    c.env("VORTEX_MIKU_CLI_PY", script.to_string_lossy());
    let (cod, _, _) = c.correr(&["natural", "instalar", "--cpu"]);
    assert_eq!(cod, 7, "el código de Python llega tal cual");
    let (cod, _, _) = c.correr(&["elevenlabs", "quitar"]);
    assert_eq!(cod, 7);
    let llamadas = std::fs::read_to_string(&anota).unwrap();
    assert_eq!(
        llamadas,
        format!(
            "-I {0} natural instalar --cpu\n-I {0} elevenlabs quitar\n",
            script.display()
        )
    );
    // Sin la parte de Python instalada, lo dice.
    let mut sin = Caja::nueva();
    let raiz = sin.base.to_string_lossy().into_owned();
    sin.env("VORTEX_MIKU_RAIZ", raiz);
    let no_hay = sin.base.join("no-hay-vortex-miku-python");
    sin.env("VORTEX_MIKU_CLI_PY", no_hay.to_string_lossy());
    let (cod, _, err) = sin.correr(&["natural"]);
    assert_eq!(cod, 1);
    assert!(err.contains("necesita la parte de Python de Miku"), "{err}");
}

// ---- ia -------------------------------------------------------------------------------------------

#[test]
fn ia_estado_cambia_y_valida() {
    let mut c = Caja::nueva();
    c.env("VORTEX_MIKU_OLLAMA", "http://127.0.0.1:9");
    let (cod, sal, _) = c.correr(&["ia"]);
    assert_eq!(cod, 0);
    assert_eq!(
        sal,
        "ia=claude\nCámbiala con: vortex-miku ia claude|local|gemini|openai [MODELO]  ·  vortex-miku ia otra URL MODELO\n"
    );
    // ChatGPT con modelo y sin clave: la guarda como dice y avisa que falta (sin terminal no la pide).
    let (cod, sal, _) = c.correr(&["ia", "openai", "gpt-prueba"]);
    assert_eq!(cod, 0);
    assert_eq!(
        sal,
        "ia=openai: Miku piensa con ChatGPT («gpt-prueba»).\nFalta tu clave: ponla con «vortex-miku ia clave».\n"
    );
    let cfg = c.leer_config();
    assert_eq!(
        (cfg["ia"].as_str(), cfg["ia_modelo"].as_str()),
        (Some("openai"), Some("gpt-prueba"))
    );
    let (_, sal, _) = c.correr(&["ia", "estado"]);
    assert!(
        sal.starts_with("ia=openai\n  ChatGPT: modelo «gpt-prueba» en https://api.openai.com/v1\n  clave: (sin poner: vortex-miku ia clave)\n"),
        "{sal}"
    );
    // La clave, dada en la orden: queda en un archivo (sin llavero en el PATH de la prueba).
    let (cod, sal, err) = c.correr(&["ia", "clave", "sk-prueba"]);
    assert_eq!(cod, 0, "{err}");
    let archivo = c.home.join(".config/vortex/ia-openai.clave");
    assert_eq!(
        sal,
        format!("Listo: la clave de ChatGPT quedó en {}.\n", archivo.display())
    );
    assert_eq!(std::fs::read_to_string(&archivo).unwrap().trim(), "sk-prueba");
    assert!(c.correr(&["ia"]).1.contains("  clave: guardada\n"));
    // «otra» pide URL y modelo.
    let (cod, _, err) = c.correr(&["ia", "otra", "no-es-url", "m"]);
    assert_eq!(cod, 2);
    assert!(err.contains("uso: vortex-miku ia otra URL MODELO"), "{err}");
    let (cod, sal, _) = c.correr(&["ia", "otra", "https://openrouter.ai/api/v1/", "un/modelo"]);
    assert_eq!(cod, 0);
    assert!(
        sal.starts_with("ia=otra: Miku piensa con tu IA («un/modelo»).\n"),
        "{sal}"
    );
    let (_, sal, _) = c.correr(&["ia"]);
    assert!(
        sal.contains("  tu IA: modelo «un/modelo» en https://openrouter.ai/api/v1\n"),
        "{sal}"
    );
    // Claude no usa clave ni modelo; una IA que no existe, falla.
    let (cod, sal, _) = c.correr(&["ia", "claude", "ignorado"]);
    assert_eq!((cod, sal.as_str()), (0, "ia=claude: Miku piensa con Claude.\n"));
    let (cod, _, err) = c.correr(&["ia", "clave"]);
    assert_eq!((cod, err.as_str()), (1, "vortex-miku: Claude no usa clave.\n"));
    let (cod, _, err) = c.correr(&["ia", "pepe"]);
    assert_eq!(cod, 2);
    assert!(
        err.contains("uso: vortex-miku ia claude|local|gemini|openai"),
        "{err}"
    );
    // La local sin Ollama instalado.
    let (cod, sal, _) = c.correr(&["ia", "local"]);
    assert_eq!(cod, 0);
    assert!(
        sal.contains("Falta Ollama: actualiza Vortexine's con «vortex actualizar»"),
        "{sal}"
    );
    let (_, sal, _) = c.correr(&["ia"]);
    assert!(
        sal.contains("  falta Ollama: actualiza con «vortex actualizar»\n"),
        "{sal}"
    );
}

#[test]
fn ia_local_con_ollama_falso_baja_el_modelo() {
    let mut c = Caja::nueva();
    let srv = comun::servidor(|p| match p.ruta.as_str() {
        "/api/version" => comun::json(json!({"version": "0"})),
        "/api/tags" => comun::json(json!({"models": []})),
        "/api/pull" => (
            200,
            "application/x-ndjson",
            b"{\"status\":\"pulling\",\"total\":100,\"completed\":50}\n{\"status\":\"success\"}\n".to_vec(),
        ),
        _ => (404, "text/plain", vec![]),
    });
    c.env("VORTEX_MIKU_OLLAMA", &srv.url);
    c.falso("ollama", "exit 0");
    let (cod, sal, err) = c.correr(&["ia", "local"]);
    assert_eq!(cod, 0, "{err}");
    assert!(
        sal.starts_with("ia=local: Miku piensa con IA local («qwen3.5:4b»).\n"),
        "{sal}"
    );
    assert!(
        sal.contains("Bajando la IA local «qwen3.5:4b» (una sola vez, ~3,3 GB)…"),
        "{sal}"
    );
    assert!(
        sal.contains("Lista: Miku ya puede pensar en tu PC, sin internet."),
        "{sal}"
    );
    let pedidos = srv.pedidos.lock().unwrap();
    let pull = pedidos
        .iter()
        .find(|p| p.ruta == "/api/pull")
        .expect("pidió bajar el modelo");
    assert_eq!(pull.json()["model"], "qwen3.5:4b");
}

// ---- cámara, platicadora, habla-sola, sentimientos ----------------------------------------------------

#[test]
fn camara_platicadora_habla_sola_y_sentimientos() {
    let c = Caja::nueva();
    assert_eq!(c.correr(&["camara"]).1, "camara=off\n");
    let (cod, sal, _) = c.correr(&["cámara", "on"]);
    assert_eq!(cod, 0);
    assert!(sal.starts_with("camara=on\nAntes de usar la cámara"), "{sal}");
    assert_eq!(c.correr(&["camara", "estado"]).1, "camara=on\n");
    assert_eq!(c.correr(&["camara", "off"]).1, "camara=off\n");
    assert_eq!(c.correr(&["camara", "quizas"]).0, 2);

    assert_eq!(c.correr(&["platicadora"]).1, "platicadora=on\n");
    assert_eq!(c.correr(&["platicadora", "off"]).1, "platicadora=off\n");
    assert_eq!(
        c.correr(&["estado"]).1,
        "avatar=on\nabierta=no\nplaticadora=off\n"
    );
    assert_eq!(c.correr(&["platicadora", "tal"]).0, 2);

    assert_eq!(c.correr(&["habla-sola"]).1, "habla-sola=on\n");
    assert_eq!(c.correr(&["habla-sola", "off"]).1, "habla-sola=off\n");
    assert_eq!(c.leer_config()["habla_sola"], json!(false));
    assert_eq!(c.correr(&["habla_sola", "x"]).0, 2);

    let (cod, sal, _) = c.correr(&["sentimientos"]);
    assert_eq!(cod, 0);
    assert!(sal.starts_with("sentimientos=on\nánimo="), "{sal}");
    assert!(
        sal.contains(&format!("notas: {}", c.home.join("Miku").display())),
        "{sal}"
    );
    let (_, sal, _) = c.correr(&["corazón", "off"]);
    assert_eq!(
        sal,
        format!(
            "sentimientos=off\nSus notas se quedan en {}: si los vuelves a encender, se acuerda de todo.\n",
            c.home.join("Miku").display()
        )
    );
    assert_eq!(c.correr(&["sentimientos", "on"]).1, "sentimientos=on\n");
}

#[test]
fn lo_que_corre_la_ia_no_toca_la_camara_ni_la_companera() {
    let mut c = Caja::nueva();
    c.env("VORTEX_MIKU_DESDE_IA", "1");
    let razon = "vortex-miku: eso solo lo cambia la persona (en la ventana de Miku o en Ajustes), no un comando que corre la IA.\n";
    for args in [
        vec!["camara", "on"],
        vec!["companera", "on"],
        vec!["companera", "off"],
        vec!["companera", "tono", "suave"],
    ] {
        let (cod, _, err) = c.correr(&args);
        assert_eq!((cod, err.as_str()), (1, razon), "{args:?}");
    }
    assert!(!c.config().exists(), "y no cambió nada");
    // Mirar sí puede: el estado y la bitácora.
    assert_eq!(c.correr(&["camara", "estado"]).1, "camara=off\n");
    assert_eq!(c.correr(&["companera", "estado"]).0, 0);
    assert_eq!(
        c.correr(&["companera", "bitacora"]).1,
        "La bitácora está vacía.\n"
    );
}

// ---- modo compañera -------------------------------------------------------------------------------

#[test]
fn companera_estado_tono_bitacora_y_apagar() {
    let mut c = Caja::nueva();
    c.env("VORTEX_MIKU_AHORA", "2026-11-19T15:00");
    assert_eq!(
        c.correr(&["companera"]).1,
        "companera=off\ntono=carinosa\nsin_cuenta=off\n"
    );
    assert_eq!(c.correr(&["companera", "tono", "suave"]).1, "tono=suave\n");
    assert_eq!(c.correr(&["compañera", "tono", "cariñosa"]).1, "tono=carinosa\n");
    let (cod, _, err) = c.correr(&["companera", "tono", "celosa"]);
    assert_eq!(
        (cod, err.as_str()),
        (2, "vortex-miku: uso: vortex-miku companera tono carinosa|suave\n")
    );
    let (cod, _, err) = c.correr(&["companera", "bailar"]);
    assert_eq!(cod, 2);
    assert!(
        err.contains("uso: vortex-miku companera on|off|estado|tono|sin-cuenta|bitacora|borrar-bitacora"),
        "{err}"
    );
    // Con el permiso ya dado en la ventana.
    c.escribir_config(r#"{"companera": true, "companera_consentimiento": true, "camara": true}"#);
    let (_, sal, _) = c.correr(&["companera", "estado"]);
    assert_eq!(
        sal,
        "companera=on\ntono=carinosa\nsin_cuenta=off\nasomadas_hoy=0/6\nahora=puede-asomarse\n"
    );
    // Apagarlo retira el permiso y lo anota en la bitácora.
    let (cod, sal, _) = c.correr(&["companera", "off"]);
    assert_eq!((cod, sal.as_str()), (0, "companera=off\n"));
    let cfg = c.leer_config();
    assert_eq!(
        (
            cfg["companera"].clone(),
            cfg["companera_consentimiento"].clone(),
            cfg["camara"].clone()
        ),
        (json!(false), json!(false), json!(true))
    );
    let (_, sal, _) = c.correr(&["companera", "bitacora"]);
    assert!(
        sal.trim_end()
            .ends_with("2026-11-19 15:00 · Modo compañera apagado"),
        "{sal}"
    );
    assert_eq!(
        c.correr(&["companera", "borrar-bitacora"]).1,
        "Bitácora borrada.\n"
    );
    assert_eq!(
        c.correr(&["companera", "bitacora"]).1,
        "La bitácora está vacía.\n"
    );
}

#[test]
fn companera_on_sin_permiso_solo_abre_la_bienvenida_y_con_permiso_se_enciende() {
    let mut c = Caja::nueva();
    c.app_falsa();
    let (cod, sal, _) = c.correr(&["companera", "on"]);
    assert_eq!(cod, 0);
    assert!(sal.starts_with("Modo compañera: de vez en cuando"), "{sal}");
    assert!(
        sal.contains("\nSe acepta en la ventana de Miku, con el botón «Sí, activar el modo compañera».\n"),
        "{sal}"
    );
    assert!(sal.ends_with("companera=off\n"), "{sal}");
    assert!(esperar(5.0, || !c.aperturas().is_empty()));
    assert_eq!(
        c.aperturas(),
        vec!["visita= companera=consentimiento voz=".to_string()],
        "abre la ventana para aceptar"
    );
    assert!(!c.config().exists(), "ningún comando da el permiso");
    // Ya abierta: avisa y no la abre otra vez.
    let (_, sal, _) = c.correr(&["companera", "acepto"]);
    assert!(
        sal.contains("Miku ya está abierta: ciérrala y vuelve a correr «vortex-miku companera on»."),
        "{sal}"
    );
    assert_eq!(c.aperturas().len(), 1);
    assert_eq!(c.correr(&["alternar"]).0, 0, "se cierra");
    // Con el permiso dado en la ventana, «on» prende el modo y la cámara.
    c.escribir_config(r#"{"companera_consentimiento": true}"#);
    let (cod, sal, _) = c.correr(&["companera", "on"]);
    assert_eq!((cod, sal.as_str()), (0, "companera=on\n"));
    let cfg = c.leer_config();
    assert_eq!(
        (&cfg["companera"], &cfg["camara"], &cfg["camara_aviso"]),
        (&json!(true), &json!(true), &json!(true))
    );
    // «sin-cuenta» pide la mayoría de edad en la ventana: el comando solo la abre.
    let (cod, sal, _) = c.correr(&["companera", "sin-cuenta", "on"]);
    assert_eq!(cod, 0);
    assert!(
        sal.contains("\nSe marca en la ventana de Miku, con el botón «Soy mayor de 18 años».\n")
            && sal.ends_with("sin_cuenta=off\n"),
        "{sal}"
    );
    assert!(esperar(5.0, || c.aperturas().len() == 2));
    assert_eq!(c.aperturas()[1], "visita= companera=mayoria voz=");
    c.correr(&["alternar"]);
    let (cod, _, err) = c.correr(&["companera", "sin-cuenta", "quizas"]);
    assert_eq!(cod, 2);
    assert!(err.contains("companera sin-cuenta on|off"), "{err}");
    let (_, sal, _) = c.correr(&["companera", "sin-cuenta", "off"]);
    assert_eq!(sal, "sin_cuenta=off\n");
    // Y sin el modo encendido no se puede.
    c.correr(&["companera", "off"]);
    let (cod, _, err) = c.correr(&["companera", "sin-cuenta", "on"]);
    assert_eq!(
        (cod, err.as_str()),
        (
            1,
            "vortex-miku: primero activa el modo compañera (vortex-miku companera on).\n"
        )
    );
}

// ---- abrir y cerrar -------------------------------------------------------------------------------

#[test]
fn alternar_abre_y_cierra_del_todo() {
    let mut c = Caja::nueva();
    c.app_falsa();
    let (cod, _, err) = c.correr(&["alternar"]);
    assert_eq!(cod, 0, "{err}");
    let pid: u32 = std::fs::read_to_string(c.runtime.join("vortex-miku.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(esperar(5.0, || c.correr(&["estado"]).1.contains("abierta=si\n")));
    // Corre en su propia sesión (para poder cerrarla entera) y con un hijo (como la voz).
    assert_eq!(
        std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .unwrap()
            .rsplit(')')
            .next()
            .unwrap()
            .split_whitespace()
            .nth(3)
            .unwrap(),
        pid.to_string()
    );
    assert!(esperar(5.0, || c.aperturas().len() == 1));
    let hijos = || -> Vec<u32> {
        std::fs::read_dir("/proc")
            .unwrap()
            .flatten()
            .filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok())
            .filter(|p| {
                std::fs::read_to_string(format!("/proc/{p}/stat"))
                    .ok()
                    .is_some_and(|t| {
                        t.rsplit(')').next().unwrap().split_whitespace().nth(1) == Some(&pid.to_string())
                    })
            })
            .collect()
    };
    assert!(esperar(5.0, || !hijos().is_empty()));
    let nietos = hijos();
    let log = std::fs::read_to_string(c.runtime.join("vortex-miku.log")).unwrap();
    assert_eq!(log, "", "lo que escriba la app va al registro");
    // «mostrar» con la app abierta no hace nada.
    assert_eq!(c.correr(&["mostrar"]).0, 0);
    assert_eq!(c.aperturas().len(), 1);
    // Otra vez la cierra: no queda ni ella, ni sus hijos, ni el PID, ni los audios temporales.
    std::fs::create_dir_all(c.runtime.join("vortex-miku")).unwrap();
    std::fs::write(c.runtime.join("vortex-miku/audio.wav"), "x").unwrap();
    assert_eq!(c.correr(&["alternar"]).0, 0);
    assert!(c.correr(&["estado"]).1.contains("abierta=no\n"));
    assert!(esperar(5.0, || !vivo(pid) && nietos.iter().all(|p| !vivo(*p))));
    assert!(!c.runtime.join("vortex-miku.pid").exists() && !c.runtime.join("vortex-miku").exists());
    // «mostrar» la abre si estaba cerrada.
    assert_eq!(c.correr(&["mostrar"]).0, 0);
    assert!(esperar(5.0, || c.aperturas().len() == 2));
    c.correr(&["alternar"]);
}

#[test]
fn un_pid_viejo_o_ajeno_cuenta_como_cerrada_y_un_cierre_brusco_se_recupera() {
    let mut c = Caja::nueva();
    c.app_falsa();
    std::fs::write(c.runtime.join("vortex-miku.pid"), "999999").unwrap();
    assert!(
        c.correr(&["estado"]).1.contains("abierta=no\n"),
        "un PID que ya no existe"
    );
    std::fs::write(c.runtime.join("vortex-miku.pid"), "1").unwrap();
    assert!(
        c.correr(&["estado"]).1.contains("abierta=no\n"),
        "un PID vivo que no es Miku"
    );
    assert_eq!(c.correr(&[]).0, 0, "sin argumentos también alterna");
    assert!(esperar(5.0, || c.correr(&["estado"]).1.contains("abierta=si\n")));
    let pid: u32 = std::fs::read_to_string(c.runtime.join("vortex-miku.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    matar_9(pid);
    assert!(esperar(5.0, || c.correr(&["estado"]).1.contains("abierta=no\n")));
    assert_eq!(c.correr(&["alternar"]).0, 0, "se puede abrir de nuevo");
    assert!(esperar(5.0, || c.correr(&["estado"]).1.contains("abierta=si\n")));
    c.correr(&["alternar"]);
}

fn matar_9(pid: u32) {
    let _ = Command::new("kill")
        .args(["-9", &pid.to_string()])
        .env("PATH", "/usr/bin:/bin")
        .status();
}

#[test]
fn avatar_con_la_app_abierta_la_reabre() {
    let mut c = Caja::nueva();
    c.app_falsa();
    c.correr(&["alternar"]);
    assert!(esperar(5.0, || c.aperturas().len() == 1));
    let antes: u32 = std::fs::read_to_string(c.runtime.join("vortex-miku.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let (cod, sal, _) = c.correr(&["avatar", "off"]);
    assert_eq!((cod, sal.as_str()), (0, "avatar=off\n"));
    assert!(esperar(5.0, || c.aperturas().len() == 2), "la abrió de nuevo");
    let despues: u32 = std::fs::read_to_string(c.runtime.join("vortex-miku.pid"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_ne!(antes, despues);
    assert!(esperar(5.0, || !vivo(antes)));
    c.correr(&["alternar"]);
}

#[test]
fn si_la_app_se_cierra_al_abrir_lo_dice_con_su_registro() {
    let mut c = Caja::nueva();
    let app = c.base.join("mala/miku-rs");
    escribir_exe(&app, "#!/bin/sh\necho 'falta algo importante' >&2\nexit 3\n");
    c.env("VORTEX_MIKU_APP", app.to_string_lossy());
    let (cod, _, err) = c.correr(&["alternar"]);
    assert_eq!(cod, 1);
    assert_eq!(
        err,
        "vortex-miku: Miku se cerró al abrir. falta algo importante\n"
    );
    assert!(!c.runtime.join("vortex-miku.pid").exists());
}

// ---- grabar y momentos ----------------------------------------------------------------------------

#[test]
fn grabar_y_momentos_sin_nada_que_hacer() {
    let c = Caja::nueva();
    assert_eq!(c.correr(&["grabar", "estado"]).1, "grabando=no\n");
    let (cod, _, err) = c.correr(&["grabar", "parar"]);
    assert_eq!((cod, err.as_str()), (1, "vortex-miku: No estoy grabando nada.\n"));
    assert_eq!(c.correr(&["grabar", "pausa"]).0, 2);
    assert_eq!(c.correr(&["grabar", "empezar", "x"]).0, 2);
    let (cod, _, err) = c.correr(&["momentos"]);
    assert_eq!(cod, 1);
    assert_eq!(
        err,
        format!(
            "vortex-miku: no hay grabaciones en {}. Dime cuál: vortex-miku momentos VIDEO\n",
            c.base.join("videos").display()
        )
    );
    assert_eq!(c.correr(&["momentos", "a", "b"]).0, 2);
}

// ---- sentir ---------------------------------------------------------------------------------------

#[test]
fn sentir_revisar_asoma_a_miku_con_su_aviso_y_respeta_los_limites() {
    let mut c = Caja::nueva();
    c.app_falsa();
    let avisos = c.base.join("avisos.log");
    c.falso("notify-send", &format!("echo \"$@\" >> {}", avisos.display()));
    c.env("VORTEX_MIKU_AHORA", "2026-11-19T15:00");
    c.escribir_config(
        r#"{"companera": true, "companera_consentimiento": true, "camara": true, "habla_sola": false}"#,
    );
    let (cod, _, err) = c.correr(&["sentir", "revisar"]);
    assert_eq!(cod, 0, "{err}");
    assert_eq!(c.aperturas(), vec!["visita= companera=asomarse voz=".to_string()]);
    let aviso = std::fs::read_to_string(&avisos).unwrap();
    assert!(
        aviso.contains("Miku se asoma (modo compañera)") && aviso.contains("Pulsa Esc para que no."),
        "{aviso}"
    );
    let (_, sal, _) = c.correr(&["companera", "estado"]);
    assert!(sal.contains("asomadas_hoy=1/6\nahora=muy-pronto\n"), "{sal}");
    // Ya hay una abierta, y además pasó hace muy poco: no se asoma otra vez.
    c.correr(&["alternar"]);
    c.correr(&["sentir", "revisar"]);
    assert_eq!(c.aperturas().len(), 1);
    // De noche no, aunque haya pasado el tiempo.
    c.env("VORTEX_MIKU_AHORA", "2026-11-20T23:30");
    c.correr(&["sentir", "revisar"]);
    assert_eq!(c.aperturas().len(), 1);
    // Sin sentimientos ni compañera no pasa nada.
    c.escribir_config(r#"{"sentimientos": false}"#);
    assert_eq!(c.correr(&["sentir", "revisar"]).0, 0);
    assert_eq!(c.aperturas().len(), 1);
}

#[test]
fn sentir_aviso_abre_a_miku_solo_si_pulsan_el_boton() {
    let mut c = Caja::nueva();
    c.app_falsa();
    c.falso("notify-send", "echo hablar");
    assert_eq!(c.correr(&["sentir", "aviso", "Hola"]).0, 0);
    assert_eq!(c.aperturas(), vec!["visita= companera= voz=".to_string()]);
    c.correr(&["alternar"]);
    c.falso("notify-send", "echo ");
    assert_eq!(c.correr(&["sentir", "aviso", "Hola"]).0, 0);
    assert_eq!(
        c.aperturas().len(),
        1,
        "cerró la notificación sin pulsar: no se abre"
    );
}

#[test]
fn sentir_decir_lanza_el_aviso_aparte_y_respeta_habla_sola_y_si_juegas() {
    let c = Caja::nueva();
    let avisos = c.base.join("avisos.log");
    c.falso("notify-send", &format!("echo \"$@\" >> {}", avisos.display()));
    c.escribir_config(r#"{"habla_sola": true}"#);
    assert_eq!(c.correr(&["sentir", "decir", "Te quiero contar algo"]).0, 0);
    assert!(esperar(5.0, || std::fs::read_to_string(&avisos).is_ok_and(|t| t
        .contains("Te quiero contar algo")
        && t.contains("-A hablar=Hablar con Miku"))));
    // Si juegas (la marca de modo juego), no habla.
    std::fs::create_dir_all(c.home.join(".local/state/vortex")).unwrap();
    std::fs::write(c.home.join(".local/state/vortex/modo-juego"), "").unwrap();
    let antes = std::fs::read_to_string(&avisos).unwrap();
    assert_eq!(c.correr(&["sentir", "decir", "Otra cosa"]).0, 0);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(std::fs::read_to_string(&avisos).unwrap(), antes);
    // Con habla-sola apagado, tampoco.
    std::fs::remove_file(c.home.join(".local/state/vortex/modo-juego")).unwrap();
    c.correr(&["habla-sola", "off"]);
    c.correr(&["sentir", "decir", "Nada"]);
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(std::fs::read_to_string(&avisos).unwrap(), antes);
    // Una orden de sentir que no existe.
    let (cod, _, err) = c.correr(&["sentir", "bailar"]);
    assert_eq!(cod, 2);
    assert!(
        err.contains("uso: vortex-miku sentir partida JUEGO MINUTOS CPU GPU | sentir revisar"),
        "{err}"
    );
}

#[test]
fn sentir_partida_y_jugando_con_sus_condiciones() {
    let mut c = Caja::nueva();
    c.app_falsa();
    let avisos = c.base.join("avisos.log");
    c.falso("notify-send", &format!("echo \"$@\" >> {}", avisos.display()));
    // Sin sentimientos no queda nada anotado ni se avisa.
    c.escribir_config(r#"{"sentimientos": false}"#);
    assert_eq!(c.correr(&["sentir", "partida", "Roblox", "45", "30", "60"]).0, 0);
    assert!(!c.home.join("Miku").exists());
    // Con sentimientos: la partida se anota en el diario del corazón; con habla-sola apagado no avisa.
    c.escribir_config(r#"{"habla_sola": false}"#);
    assert_eq!(c.correr(&["sentir", "partida", "Roblox", "45", "30", "60"]).0, 0);
    assert!(c.home.join("Miku").is_dir(), "su corazón guardó la partida");
    let estado = c.correr(&["sentimientos"]).1;
    assert!(estado.contains("días=") || estado.contains("afecto="), "{estado}");
    // «jugando» solo se asoma con la platicadora encendida; apagada no pasa nada.
    c.escribir_config(r#"{"platicadora": false}"#);
    assert_eq!(c.correr(&["sentir", "jugando", "Roblox", "50", "20", "40"]).0, 0);
    assert!(c.aperturas().is_empty());
    // «preguntar» sin la platicadora tampoco abre.
    assert_eq!(c.correr(&["sentir", "preguntar", "¿Cómo te fue?", "0"]).0, 0);
    assert!(c.aperturas().is_empty());
    // Encendida, pregunta abriendo a Miku con la pregunta (si no juegas ni estás en una llamada).
    c.escribir_config(r#"{"platicadora": true}"#);
    assert_eq!(c.correr(&["sentir", "preguntar", "¿Cómo te fue?", "0"]).0, 0);
    assert!(esperar(5.0, || c.aperturas().len() == 1));
    c.correr(&["alternar"]);
}
