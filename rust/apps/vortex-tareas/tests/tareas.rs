//! Las pruebas de `vortex-tareas`: la batería de `pruebas/tareas.sh` (la parte de `bin/vortex-tareas`) con una
//! bóveda de Obsidian temporal, la hora fija (`VORTEX_TAREAS_AHORA`) y programas falsos en un PATH temporal.
//! HOME y XDG son temporales: nada toca tu Obsidian ni avisa en tu escritorio.
//!
//! La MISMA batería corre contra el nuevo (Rust) y contra el viejo (`bin/vortex-tareas` con
//! `VORTEX_TAREAS_PYTHON=1`, si hay python3), y al final se compara la salida de uno y otro.
//! Nunca abre quickshell ni ventanas.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Lanzador {
    Rust,
    Python,
}

fn raiz_repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn hay_python() -> bool {
    Command::new("python3")
        .arg("-c")
        .arg("pass")
        .status()
        .is_ok_and(|s| s.success())
}

fn lanzadores() -> Vec<Lanzador> {
    let mut v = vec![Lanzador::Rust];
    if hay_python() && raiz_repo().join("bin/vortex-tareas").exists() {
        v.push(Lanzador::Python);
    } else {
        eprintln!("(sin python3: solo se prueba el de Rust)");
    }
    v
}

struct Banco {
    t: tempfile::TempDir,
    l: Lanzador,
    env: BTreeMap<String, String>,
}

impl Banco {
    fn nuevo(l: Lanzador) -> Banco {
        let t = tempfile::Builder::new()
            .prefix("vortex-tareas-prueba.")
            .tempdir()
            .unwrap();
        let p = t.path();
        for d in [
            "home",
            "cfg",
            "estado",
            "bin",
            "serp/scripts",
            "b/Escuela",
            "b/.obsidian",
        ] {
            fs::create_dir_all(p.join(d)).unwrap();
        }
        let falsos = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/falsos");
        // Enlaces, no copias: un archivo recién escrito da «Text file busy» al ejecutarlo si otro hilo hace fork.
        symlink(falsos.join("notify-send"), p.join("bin/notify-send")).unwrap();
        symlink(falsos.join("xdg-open"), p.join("bin/xdg-open")).unwrap();
        symlink(falsos.join("qs_manager.sh"), p.join("serp/scripts/qs_manager.sh")).unwrap();
        let mut env = BTreeMap::new();
        let s = |x: &str| p.join(x).to_string_lossy().into_owned();
        env.insert("HOME".into(), s("home"));
        env.insert("XDG_CONFIG_HOME".into(), s("cfg"));
        env.insert("XDG_STATE_HOME".into(), s("estado"));
        env.insert("SERPANTINUM_DIR".into(), s("serp"));
        env.insert("AVISOS".into(), s("avisos"));
        env.insert("ABIERTOS".into(), s("abiertos"));
        env.insert("PANELES".into(), s("paneles"));
        env.insert("VORTEX_TAREAS_BOVEDA".into(), s("b"));
        env.insert("VORTEX_TAREAS_AHORA".into(), "2026-11-02T10:00".into());
        fs::write(p.join("avisos"), "").unwrap();
        Banco { t, l, env }
    }

    fn ruta(&self, r: &str) -> PathBuf {
        self.t.path().join(r)
    }

    fn texto(&self, r: &str) -> String {
        fs::read_to_string(self.ruta(r)).unwrap_or_default()
    }

    fn orden(&self) -> Command {
        let mut c = match self.l {
            Lanzador::Rust => Command::new(env!("CARGO_BIN_EXE_vortex-tareas-rs")),
            Lanzador::Python => {
                let mut c = Command::new(raiz_repo().join("bin/vortex-tareas"));
                c.env("VORTEX_TAREAS_PYTHON", "1").env("PYTHONUTF8", "1");
                c
            }
        };
        c.env_clear().env("LANG", "C.UTF-8");
        c.env(
            "PATH",
            format!(
                "{}:{}",
                self.ruta("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        for (k, v) in &self.env {
            c.env(k, v);
        }
        c
    }

    fn correr(&self, args: &[&str]) -> Output {
        self.orden().args(args).output().unwrap()
    }

    fn sale(&self, args: &[&str]) -> String {
        let o = self.correr(args);
        assert!(
            o.status.success(),
            "{:?} {args:?}: {}",
            self.l,
            String::from_utf8_lossy(&o.stderr)
        );
        String::from_utf8_lossy(&o.stdout).into_owned()
    }

    fn funciona(&self, args: &[&str]) -> bool {
        self.correr(args).status.success()
    }

    fn con(&mut self, k: &str, v: &str) {
        self.env.insert(k.into(), v.into());
    }

    fn sin(&mut self, k: &str) {
        self.env.remove(k);
    }

    fn hora(&mut self, h: &str) {
        self.con("VORTEX_TAREAS_AHORA", h);
    }

    /// «texto|fecha|dias|nota» de cada tarea de `listar` (con `None` si no hay días, como la prueba de bash).
    fn lista(&self) -> String {
        let d: serde_json::Value = serde_json::from_str(&self.sale(&["listar"])).unwrap();
        d["tareas"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| {
                let dias = t["dias"].as_i64().map_or("None".to_string(), |d| d.to_string());
                format!(
                    "{}|{}|{dias}|{}",
                    t["texto"].as_str().unwrap(),
                    t["fecha"].as_str().unwrap(),
                    t["nota"].as_str().unwrap()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn avisos(&self) -> Vec<String> {
        self.texto("avisos").lines().map(String::from).collect()
    }

    /// Espera (hasta 5 s) a que un programa que quedó en segundo plano deje su marca.
    fn esperar(&self, r: &str, que: impl Fn(&str) -> bool) -> bool {
        let fin = Instant::now() + Duration::from_secs(5);
        while Instant::now() < fin {
            if que(&self.texto(r)) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        false
    }
}

const MATE: &str = "# Mate
- [ ] Examen de fracciones 📅 2026-11-02
- [ ] Maqueta de ciencias (3/11)
- [ ] Problemas pág. 40 para el 6/11
- [ ] Tarea vieja [due:: 2026-10-30]
- [ ] Sin fecha en otra nota
- [x] Ya hecha 📅 2026-11-02
- [ ] Receta: 1/2 taza de azúcar
";

/// La bóveda de `pruebas/tareas.sh`: el lunes 2/11/2026 a las 10 am.
fn boveda_basica(b: &Banco) {
    fs::write(b.ruta("b/Escuela/Mate.md"), MATE).unwrap();
    fs::write(
        b.ruta("b/Tareas.md"),
        "---\ntags:\n  - tareas\n---\n- [ ] Comprar cartulina\n",
    )
    .unwrap();
    fs::create_dir_all(b.ruta("b/.trash")).unwrap();
    fs::write(b.ruta("b/.trash/Vieja.md"), "- [ ] En la basura 📅 2026-11-02\n").unwrap();
}

#[test]
fn leer_las_tareas() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        boveda_basica(&b);
        let lista = b.lista();
        assert_eq!(
            lista,
            "Tarea vieja|2026-10-30|-3|Mate
Examen de fracciones|2026-11-02|0|Mate
Maqueta de ciencias|2026-11-03|1|Mate
Problemas pág. 40|2026-11-06|4|Mate
Comprar cartulina||None|Tareas",
            "{l:?}: las de las notas con fecha, por fecha, y todas las de Tareas.md al final"
        );
        for malo in ["Ya hecha", "Sin fecha", "taza", "basura"] {
            assert!(!lista.contains(malo), "{l:?}: no debía salir «{malo}»");
        }
        let d: serde_json::Value = serde_json::from_str(&b.sale(&["listar"])).unwrap();
        assert_eq!(
            d["archivo"],
            b.ruta("b/Tareas.md").to_string_lossy().as_ref(),
            "{l:?}"
        );
        assert_eq!(d["boveda"], b.ruta("b").to_string_lossy().as_ref(), "{l:?}");
        // sin argumentos es lo mismo que «listar»
        assert_eq!(b.sale(&[]), b.sale(&["listar"]), "{l:?}");
    }
}

#[test]
fn agregar_y_marcar_hecha() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        boveda_basica(&b);
        b.sale(&["agregar", "  Leer   el cap. 3 ", "2026-11-04"]);
        assert_eq!(
            b.texto("b/Tareas.md").lines().last().unwrap(),
            "- [ ] Leer el cap. 3 📅 2026-11-04",
            "{l:?}: la nueva va a Tareas.md con su 📅"
        );
        assert!(!b.funciona(&["agregar", "Mal", "2026-13-40"]), "{l:?}");
        assert!(
            !b.texto("b/Tareas.md").contains("Mal"),
            "{l:?}: una fecha que no existe no se agrega"
        );
        assert!(!b.funciona(&["agregar", "   "]), "{l:?}: vacía no se agrega");

        let mate = b.ruta("b/Escuela/Mate.md");
        let mate = mate.to_string_lossy();
        b.sale(&["hecha", &mate, "2", "- [ ] Examen de fracciones 📅 2026-11-02"]);
        assert_eq!(
            b.texto("b/Escuela/Mate.md").lines().nth(1).unwrap(),
            "- [x] Examen de fracciones 📅 2026-11-02 ✅ 2026-11-02",
            "{l:?}: queda [x] con ✅ y la fecha, en su nota"
        );
        let antes = b.texto("b/Escuela/Mate.md");
        assert!(!b.funciona(&["hecha", &mate, "3", "- [ ] Otra cosa"]), "{l:?}");
        assert_eq!(
            b.texto("b/Escuela/Mate.md"),
            antes,
            "{l:?}: si la línea cambió, no toca nada"
        );
        assert!(
            !b.funciona(&["hecha", &mate, "6", "- [x] Ya hecha 📅 2026-11-02"]),
            "{l:?}: no es una tarea abierta"
        );
    }
}

#[test]
fn avisos() {
    for l in lanzadores() {
        let mut b = Banco::nuevo(l);
        boveda_basica(&b);
        b.sale(&["avisar"]);
        let a = b.avisos();
        assert!(
            a.iter().any(|x| x == "Para hoy: Tarea vieja") || a.iter().any(|x| x.starts_with("Tienes ")),
            "{l:?}: {a:?}"
        );
        assert!(
            a.iter().all(|x| !x.contains("Maqueta")),
            "{l:?}: a las 10 am no avisa de la de mañana"
        );
        let n = b.avisos().len();
        b.sale(&["avisar"]);
        assert_eq!(b.avisos().len(), n, "{l:?}: no repite el aviso");

        b.hora("2026-11-02T18:00");
        b.con("RESPUESTA", "ver");
        b.sale(&["avisar"]);
        assert_eq!(
            b.avisos().last().unwrap(),
            "Para mañana: Maqueta de ciencias",
            "{l:?}: a las 6 pm avisa de la de mañana"
        );
        assert!(
            b.esperar("paneles", |t| t.lines().any(|x| x == "toggle tareas")),
            "{l:?}: «Ver mis tareas» abre el panel"
        );

        b.sale(&["agregar", "Otra para hoy", "2026-11-02"]);
        fs::create_dir_all(b.ruta("estado/vortex")).unwrap();
        fs::write(b.ruta("estado/vortex/modo-juego"), "").unwrap();
        let n = b.avisos().len();
        b.sale(&["avisar"]);
        assert_eq!(b.avisos().len(), n, "{l:?}: en modo juego no avisa");
    }
}

#[test]
fn aviso_de_varias_y_registro() {
    for l in lanzadores() {
        let mut b = Banco::nuevo(l);
        boveda_basica(&b);
        b.hora("2026-11-02T18:00");
        b.sale(&["avisar"]);
        assert_eq!(b.avisos(), ["Tienes 3 tareas pendientes"], "{l:?}");
        assert_eq!(
            b.texto("estado/vortex/tareas-avisos.json"),
            "{\"dia\": \"2026-11-02\", \"avisadas\": [\"Tarea vieja|2026-10-30\", \"Examen de fracciones|2026-11-02\", \"Maqueta de ciencias|2026-11-03\"]}",
            "{l:?}: el registro de «ya avisé»"
        );
        // otro día: el registro se reinicia
        b.hora("2026-11-03T08:00");
        b.sale(&["avisar"]);
        assert!(
            b.texto("estado/vortex/tareas-avisos.json")
                .starts_with("{\"dia\": \"2026-11-03\""),
            "{l:?}"
        );
    }
}

#[test]
fn la_boveda_de_obsidian_y_abrir() {
    for l in lanzadores() {
        let mut b = Banco::nuevo(l);
        boveda_basica(&b);
        b.sin("VORTEX_TAREAS_BOVEDA");
        fs::create_dir_all(b.ruta("otra")).unwrap();
        fs::create_dir_all(b.ruta("cfg/obsidian")).unwrap();
        fs::write(
            b.ruta("cfg/obsidian/obsidian.json"),
            format!(
                "{{\"vaults\":{{\"a\":{{\"path\":\"{}\",\"ts\":1}},\"b\":{{\"path\":\"{}\",\"ts\":2,\"open\":true}},\"c\":{{\"path\":\"/no/existe\",\"ts\":3}}}}}}\n",
                b.ruta("b").display(),
                b.ruta("otra").display()
            ),
        )
        .unwrap();
        let d: serde_json::Value = serde_json::from_str(&b.sale(&["listar"])).unwrap();
        assert_eq!(
            d["boveda"],
            b.ruta("otra").to_string_lossy().as_ref(),
            "{l:?}: usa la bóveda que Obsidian tiene abierta"
        );
        b.sale(&["abrir"]);
        assert!(b.esperar("abiertos", |t| !t.is_empty()), "{l:?}");
        assert!(
            b.texto("otra/Tareas.md").lines().any(|x| x == "  - tareas"),
            "{l:?}: abrir crea Tareas.md (con tags)"
        );
        let esperado = format!(
            "obsidian://open?path={}",
            b.ruta("otra/Tareas.md").to_string_lossy().replace('/', "%2F")
        );
        assert_eq!(
            b.texto("abiertos").trim(),
            esperado,
            "{l:?}: y la abre en Obsidian"
        );

        // sin los de «abierta»: la de ts más alto que exista
        fs::write(
            b.ruta("cfg/obsidian/obsidian.json"),
            format!("{{\"vaults\":{{\"a\":{{\"path\":\"{}\",\"ts\":1}},\"c\":{{\"path\":\"/no/existe\",\"ts\":3}}}}}}", b.ruta("b").display()),
        )
        .unwrap();
        let d: serde_json::Value = serde_json::from_str(&b.sale(&["listar"])).unwrap();
        assert_eq!(d["boveda"], b.ruta("b").to_string_lossy().as_ref(), "{l:?}");

        fs::remove_dir_all(b.ruta("cfg/obsidian")).unwrap();
        let d: serde_json::Value = serde_json::from_str(&b.sale(&["listar"])).unwrap();
        assert_eq!(
            d["archivo"],
            b.ruta("home/Documentos/Tareas.md").to_string_lossy().as_ref(),
            "{l:?}: sin Obsidian, usa Documentos/Tareas.md"
        );
        assert_eq!(d["boveda"], "", "{l:?}");
    }
}

#[test]
fn orden_desconocida() {
    for l in lanzadores() {
        let b = Banco::nuevo(l);
        let o = b.correr(&["cosa"]);
        assert!(!o.status.success(), "{l:?}");
        assert!(
            String::from_utf8_lossy(&o.stderr).contains("vortex-tareas listar"),
            "{l:?}: dice cómo se usa"
        );
    }
}

// ====================================================================== el nuevo contra el viejo

/// Una bóveda rara a propósito: saltos de línea de Windows, emojis, fechas inválidas, enlaces, carpetas que se
/// saltan, un archivo que no es UTF-8, empates al ordenar…
fn boveda_dificil(b: &Banco) {
    let p = |x: &str| b.ruta(x);
    fs::create_dir_all(p("b/Escuela/Sub")).unwrap();
    fs::create_dir_all(p("b/node_modules")).unwrap();
    fs::create_dir_all(p("b/.oculta")).unwrap();
    fs::create_dir_all(p("b/Carpeta.md")).unwrap();
    let mate = "# Mate\r\n- [ ] Examen 📅 2026-11-02\r\n- [ ] Maqueta (3/11)\r\n- [ ] Para el 6/11 problemas\r\n\
- [ ] Año corto (5/11/26)\r\n- [ ] Año largo (5/11/2026)\r\n- [ ] Año nuevo (1/1)\r\n- [ ] Febrero malo (29/2/2027)\r\n\
- [ ] Día 31 de abril (31/4)\r\n- [ ] Fecha mala 📅 2026-02-30 due:: 2026-11-09\r\n\
- [ ] Con \"comillas\" y \\barra\\ 📅 2026-11-04\r\n- [ ] Con\ttab y \u{1}control 📅 2026-11-04\r\n\
- [ ] Prioridad ⏫ y ⏳ 2026-11-01 y ➕ 2026-10-01 📅 2026-11-05 🔁 2026-11-20\r\n\
  * [ ] Con sangría 📅 2026-11-04\r\n+ [ ] Con más 📅 2026-11-04\r\n- [ ] 📅 2026-11-04\r\n- [ ]  \r\n\
- [ ] Ñandú ENTREGA 8/11\r\n- [ ] Éxito Para El 7/11\r\n- [ ] ya sin año (3/9)\r\n\
- [ ] separador raro\u{2028}esta ya es otra línea 📅 2026-11-06\r- [ ] con cr 📅 2026-11-06\n- [ ] 1/2 taza (si) 2/3\n\
- [x] hecha 📅 2026-11-02\n- [ ] Mismo día A 📅 2026-11-04\n- [ ] mismo día a 📅 2026-11-04\n- [ ] [due:: 2026-11-03] con corchetes\n";
    fs::write(p("b/Escuela/Mate.md"), mate).unwrap();
    fs::write(
        p("b/Escuela/Sub/Otra.md"),
        "- [ ] Honda 📅 2026-11-03\n- [ ] Sin fecha\n",
    )
    .unwrap();
    fs::write(
        p("b/Rota.md"),
        b"- [ ] caf\xe9 \xff\xfe \xf0\x9f 2026-11-03 \xf0\x9f\x93\x85 2026-11-03\n",
    )
    .unwrap();
    fs::write(p("b/vacia.md"), "").unwrap();
    fs::write(p("b/sin-salto.md"), "- [ ] Sin salto final 📅 2026-11-02").unwrap();
    fs::write(p("b/.obsidian/x.md"), "- [ ] no 📅 2026-11-02\n").unwrap();
    fs::write(p("b/node_modules/x.md"), "- [ ] no 📅 2026-11-02\n").unwrap();
    fs::write(p("b/.oculta/x.md"), "- [ ] no 📅 2026-11-02\n").unwrap();
    fs::write(
        p("b/Carpeta.md/dentro.md"),
        "- [ ] Dentro de carpeta.md 📅 2026-11-02\n",
    )
    .unwrap();
    fs::write(p("b/nota.txt"), "- [ ] no es md 📅 2026-11-02\n").unwrap();
    fs::write(p("b/.oculto.md"), "- [ ] Oculto con punto 📅 2026-11-02\n").unwrap();
    fs::write(p("b/a.b.md"), "- [ ] Nombre con puntos 📅 2026-11-02\n").unwrap();
    fs::write(p("b/Tareas.md"), "---\ntags:\n  - tareas\n---\n# Tareas\r\n- [ ] Una sin fecha\r\n- [ ] Otra con fecha 📅 2026-11-03\r\n- [ ] Z sin fecha\n- [ ] a sin fecha\n").unwrap();
    symlink(p("b/Escuela"), p("b/enlace")).unwrap(); // una carpeta enlazada no se recorre
    symlink(p("b/Escuela/Sub/Otra.md"), p("b/enlace-a-nota.md")).unwrap(); // un enlace a nota sí cuenta
    symlink("/no/existe/nada", p("b/roto.md")).unwrap();
}

fn con_rutas_neutras(s: &str, b: &Banco) -> String {
    s.replace(b.t.path().to_string_lossy().as_ref(), "@")
}

#[test]
fn listar_igual_que_el_viejo() {
    let ls = lanzadores();
    if ls.len() < 2 {
        eprintln!("sin python3: no hay con qué comparar");
        return;
    }
    let mut rust = Banco::nuevo(Lanzador::Rust);
    let mut py = Banco::nuevo(Lanzador::Python);
    boveda_dificil(&rust);
    boveda_dificil(&py);
    // el viejo se lee con la misma bóveda y la misma hora: la del banco de Rust (las rutas salen iguales)
    let raiz = rust.t.path().to_string_lossy().into_owned();
    py.con("HOME", &format!("{raiz}/home"));
    py.con("XDG_CONFIG_HOME", &format!("{raiz}/cfg"));
    for bov in [format!("{raiz}/b"), format!("{raiz}/b/"), format!("{raiz}//b/./")] {
        for hora in [
            "2026-11-02T10:00",
            "2026-12-31T23:30",
            "2027-03-01T08:00",
            "2026-11-02",
            "2026-11-02 18:45:10",
        ] {
            rust.con("VORTEX_TAREAS_BOVEDA", &bov);
            rust.hora(hora);
            py.con("VORTEX_TAREAS_BOVEDA", &bov);
            py.hora(hora);
            let (a, v) = (rust.correr(&["listar"]), py.correr(&["listar"]));
            assert!(
                a.status.success() && v.status.success(),
                "{bov} {hora}: {}",
                String::from_utf8_lossy(&v.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&a.stdout),
                String::from_utf8_lossy(&v.stdout),
                "listar distinto con la bóveda {bov} a las {hora}"
            );
            assert!(
                a.stdout.windows(7).filter(|w| w == b"\"texto\"").count() > 20,
                "la bóveda difícil debe dar muchas tareas"
            );
        }
    }
    // sin bóveda (Documentos/Tareas.md)
    fs::create_dir_all(format!("{raiz}/home/Documentos")).unwrap();
    fs::write(
        format!("{raiz}/home/Documentos/Tareas.md"),
        "- [ ] Suelta\n- [ ] Con fecha (4/11)\n",
    )
    .unwrap();
    for b in [&mut rust, &mut py] {
        b.sin("VORTEX_TAREAS_BOVEDA");
    }
    assert_eq!(rust.sale(&["listar"]), py.sale(&["listar"]));
}

/// Las órdenes que escriben dan los mismos archivos (incluido pasar los saltos de Windows a Unix, como Python).
#[test]
fn escribir_igual_que_el_viejo() {
    let ls = lanzadores();
    if ls.len() < 2 {
        return;
    }
    let mut res = Vec::new();
    for l in ls {
        let b = Banco::nuevo(l);
        boveda_dificil(&b);
        let mate = b.ruta("b/Escuela/Mate.md").to_string_lossy().into_owned();
        b.sale(&["agregar", "Con\ttab   y ñ", "2026-11-09"]);
        b.sale(&["agregar", "Sin fecha"]);
        b.sale(&["hecha", &mate, "3", "- [ ] Maqueta (3/11)"]);
        let sin_salto = b.ruta("b/sin-salto.md").to_string_lossy().into_owned();
        b.sale(&["hecha", &sin_salto, "1", "- [ ] Sin salto final 📅 2026-11-02"]);
        assert!(!b.funciona(&["hecha", &mate, "0", "x"]));
        assert!(!b.funciona(&["hecha", &mate, "999", "x"]));
        assert!(!b.funciona(&["hecha", &mate, "uno", "x"]));
        res.push([
            fs::read(b.ruta("b/Tareas.md")).unwrap(),
            fs::read(b.ruta("b/Escuela/Mate.md")).unwrap(),
            fs::read(b.ruta("b/sin-salto.md")).unwrap(),
        ]);
    }
    assert_eq!(res[0], res[1], "los archivos quedan distintos");
}

/// Los avisos y el registro de «ya avisé» salen iguales.
#[test]
fn avisar_igual_que_el_viejo() {
    let ls = lanzadores();
    if ls.len() < 2 {
        return;
    }
    let mut res = Vec::new();
    for l in ls {
        let mut b = Banco::nuevo(l);
        boveda_dificil(&b);
        let mut horas = Vec::new();
        for hora in [
            "2026-11-02T06:00",
            "2026-11-02T07:00",
            "2026-11-02T17:30",
            "2026-11-03T20:00",
            "2026-11-03T20:10",
        ] {
            b.hora(hora);
            b.sale(&["avisar"]);
            horas.push((b.avisos(), b.texto("estado/vortex/tareas-avisos.json")));
        }
        res.push(horas);
    }
    assert_eq!(res[0], res[1]);
    assert!(!res[0].last().unwrap().0.is_empty());
}

#[test]
fn rutas_con_barras_repetidas_salen_normalizadas() {
    let mut b = Banco::nuevo(Lanzador::Rust);
    boveda_basica(&b);
    let raiz = b.t.path().to_string_lossy().into_owned();
    b.con("VORTEX_TAREAS_BOVEDA", &format!("{raiz}//b/"));
    let d: serde_json::Value = serde_json::from_str(&b.sale(&["listar"])).unwrap();
    assert_eq!(d["boveda"], format!("{raiz}/b").as_str());
    assert_eq!(
        con_rutas_neutras(d["archivo"].as_str().unwrap(), &b),
        "@/b/Tareas.md"
    );
    let _: &Path = b.t.path();
}
