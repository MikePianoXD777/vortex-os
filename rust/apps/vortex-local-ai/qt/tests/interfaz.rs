//! Las pruebas de `apps/vortex-local-ai/pruebas/test_interfaz.py`, contra la ventana de Rust: sin pantalla
//! (offscreen) y con un Ollama falso por HTTP (`../../tests/falso`). Cada una corre un guion de JavaScript
//! dentro de la app (`VORTEX_LOCAL_AI_PRUEBA`, `cpp/prueba.h`) con `ia`, `chat`, `modelos`, `boveda` y
//! `ventana` a la mano, y el guion devuelve sus datos como «DATOS{json}».
//!
//! Cubre: la QML carga sin errores ni advertencias, cada página abre, un chat con trozos que llegan poco a
//! poco (con pensamiento, detener y regenerar), «Usar mi vault» con citas, bajar un modelo con progreso (y
//! cancelar), borrar un modelo, el vault (crear, editar en el editor, guardar solo, enlaces, retroenlaces,
//! [[ autocompletado, renombrar, borrar), «Guardar en el vault», el grafo con datos y que Ollama se arranque
//! solo con un aviso amable. Todo va a carpetas temporales: no toca tus chats, tu vault ni tu Ollama.

// El build.rs de cxx-qt enlaza sus objetos en todo lo del paquete (también esta prueba): sin estas dos
// líneas faltan sus símbolos de arranque al enlazar.
extern crate cxx_qt;
extern crate cxx_qt_lib;

#[path = "../../tests/falso/mod.rs"]
mod falso;

use falso::ServidorFalso;
use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn raiz_app() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../apps/vortex-local-ai"
    ))
}

fn script(dir: &Path, nombre: &str, cuerpo: &str) {
    let p = dir.join(nombre);
    fs::write(&p, format!("#!/bin/sh\n{cuerpo}\n")).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Lo de todos los guiones: `datos`, ayudas para las listas y el cierre.
const PREAMBULO: &str = r#"
var datos = {};
var avisos = [];
ia.aviso.connect(function (t, k) { avisos.push([t, k]); });
function filas(l) { var r = []; for (var i = 0; i < l.count; i++) r.push(l.obtener(i)); return r; }
function buscar(l, k, v) { for (var i = 0; i < l.count; i++) if (l.obtener(i)[k] === v) return i; return -1; }
function lista(a) { return Array.prototype.slice.call(a); }
function fin() { datos.avisos = avisos; prueba.fin(JSON.stringify(datos)); }
var paso = function (ms, f) { prueba.paso(ms, f); };
"#;

struct Corrida {
    d: Value,
    err: String,
}

struct Entorno<'a> {
    falso: &'a ServidorFalso,
    /// El Ollama de verdad (falso) lo arranca la app sola: un script `ollama` en el PATH.
    arranca: bool,
    /// El `vortex-ia` falso responde esto a `motor --pendiente`.
    bin: &'a Path,
}

fn correr(t: &Path, e: &Entorno, cuerpo: &str) -> Corrida {
    let guion = t.join("guion.js");
    fs::write(
        &guion,
        format!(
            "{PREAMBULO}\nvar CASA = {:?};\n{cuerpo}",
            t.join("casa").to_string_lossy()
        ),
    )
    .unwrap();
    let mut c = Command::new(env!("CARGO_BIN_EXE_vortex-local-ai-rs"));
    c.env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", e.bin.display()))
        .env("HOME", t.join("home"))
        .env("QT_QPA_PLATFORM", "offscreen")
        .env("QT_QUICK_BACKEND", "software")
        // Sin terminal, Qt manda las advertencias al journal y no se verían aquí.
        .env("QT_FORCE_STDERR_LOGGING", "1")
        .env("VORTEX_LOCAL_AI_PRUEBA", &guion)
        .env("VORTEX_LOCAL_AI_HOME", t.join("casa"))
        .env("VORTEX_LOCAL_AI_COLORES", "/no/existe")
        .env("VORTEX_LOCAL_AI_OLLAMA", &e.falso.url)
        .env("VORTEX_LOCAL_AI_VRAM_MB", "10240")
        .env("VORTEX_LOCAL_AI_DIR", raiz_app())
        .env("TMPDIR", t)
        .env("XDG_RUNTIME_DIR", t)
        .env("XDG_CONFIG_HOME", t.join("config"))
        .env("XDG_STATE_HOME", t.join("estado"))
        .env("XDG_DATA_HOME", t.join("datos"))
        .env("XDG_CACHE_HOME", t.join("cache"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !e.arranca {
        c.env("VORTEX_LOCAL_AI_NO_ARRANCAR", "1");
    }
    let mut ventana = c.spawn().unwrap();
    // Límite de 120 s: si se cuelga, se corta y falla.
    let limite = Instant::now() + Duration::from_secs(120);
    while matches!(ventana.try_wait(), Ok(None)) && Instant::now() < limite {
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = ventana.kill();
    let salida = ventana.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&salida.stdout).to_string();
    let err = String::from_utf8_lossy(&salida.stderr).to_string();
    let Some(datos) = out.lines().find_map(|l| l.strip_prefix("DATOS")) else {
        panic!("sin DATOS\n{out}\n{}", &err[err.len().saturating_sub(4000)..]);
    };
    let d: Value = serde_json::from_str(datos).unwrap();
    eprintln!(
        "memoria: VmHWM {} · VmRSS {} · RssAnon {}",
        d["vmhwm"], d["vmrss"], d["rssanon"]
    );
    // La QML o Qt se quejaron: cada advertencia sale en stderr («file:///…/Main.qml:12: …», «qml: …»).
    assert_eq!(d["errores"], Value::Array(vec![]), "{d}");
    let quejas: Vec<&str> = err
        .lines()
        .filter(|l| l.contains(".qml") || l.starts_with("qml:"))
        .collect();
    assert!(quejas.is_empty(), "La QML se quejó:\n{}", quejas.join("\n"));
    Corrida { d, err }
}

/// Un directorio de pruebas con su `bin` (un `vortex-ia` que no pregunta nada: la ventana no lee el de la PC).
fn preparar() -> (tempfile::TempDir, PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let bin = t.path().join("bin");
    fs::create_dir(&bin).unwrap();
    fs::create_dir(t.path().join("home")).unwrap();
    script(
        &bin,
        "vortex-ia",
        "d=\"$(dirname \"$0\")\"\necho \"$@\" >> \"$d/args\"\ncase \"$1 $2\" in\n\"motor --pendiente\") echo '{\"preguntar\": false}';;\nesac\nexit 0",
    );
    (t, bin)
}

fn textos(v: &Value) -> Vec<String> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect()
}

fn avisos(d: &Value) -> Vec<String> {
    d["avisos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a[0].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn paginas_y_modelos() {
    let (t, bin) = preparar();
    let falso = ServidorFalso::para_ventana();
    let c = correr(
        t.path(),
        &Entorno {
            falso: &falso,
            arranca: false,
            bin: &bin,
        },
        r#"
        function inicio() {
            datos.estado = ia.estadoOllama;
            datos.gpu = modelos.gpu;
            datos.instalados = lista(modelos.nombresChat);
            var fs = filas(modelos.lista);
            datos.catalogo = fs.length;
            var cabe = {};
            fs.forEach(function (f) { cabe[f.cabe] = true; });
            datos.cabe = Object.keys(cabe).sort();
            datos.recomendado = fs.filter(function (f) { return f.recomendado; }).length;
            datos.modelo_chat = chat.modelo;
            prueba.capturar("chat-vacio");
        }
        function paginas() {
            datos.paginas = [];
            ["modelos", "vault", "ajustes", "chat"].forEach(function (p) {
                ventana.pagina = p;
                prueba.procesar();
                datos.paginas.push(ventana.pagina);
                prueba.capturar("pagina-" + p);
            });
            ventana.pagina = "modelos";
        }
        function filtrar() {
            modelos.filtro = "codigo";
            datos.filtrado = filas(modelos.lista).every(function (f) { return f.para.indexOf("codigo") >= 0; });
            datos.n_codigo = modelos.lista.count;
            modelos.filtro = "todos";
            var libres = filas(modelos.lista).filter(function (f) { return !f.instalado && f.cabe === "si"; });
            datos.a_bajar = libres[0].ident;
            datos.a_cancelar = libres[1].ident;
            modelos.bajar(datos.a_bajar);
            datos.progresos = [];
        }
        function mirar() {
            var f = modelos.lista.obtener(buscar(modelos.lista, "ident", datos.a_bajar));
            datos.progresos.push([f.bajando, Math.round(f.progreso * 100) / 100, f.estado]);
            prueba.capturar("bajando");
        }
        function bajado() {
            var f = modelos.lista.obtener(buscar(modelos.lista, "ident", datos.a_bajar));
            datos.bajado = [f.bajando, f.instalado];
            datos.instalados_despues = lista(modelos.nombresChat);
            modelos.bajar(datos.a_cancelar);
        }
        function cancelar() { modelos.cancelar(datos.a_cancelar); }
        function cancelado() {
            var f = modelos.lista.obtener(buscar(modelos.lista, "ident", datos.a_cancelar));
            datos.cancelado = [f.bajando, f.instalado];
            modelos.borrar("qwen3:8b");
        }
        function borrado() {
            datos.tras_borrar = lista(modelos.nombresChat);
            datos.modelo_chat_tras_borrar = chat.modelo;
            fin();
        }
        paso(600, inicio);
        paso(100, paginas);
        paso(200, filtrar);
        for (var i = 0; i < 4; i++) paso(150, mirar);
        paso(900, bajado);
        paso(150, cancelar);
        paso(400, cancelado);
        paso(400, borrado);
        "#,
    );
    let d = &c.d;
    assert_eq!(d["estado"], "corriendo");
    assert_eq!(d["gpu"], "Tu gráfica: Tarjeta de prueba · 10 GB");
    assert_eq!(d["instalados"], serde_json::json!(["qwen3:8b"]));
    assert!(d["catalogo"].as_i64().unwrap() > 5);
    let cabe = textos(&d["cabe"]);
    assert!(
        cabe.contains(&"si".into()) && cabe.contains(&"no".into()),
        "{cabe:?}"
    );
    assert_eq!(d["recomendado"], 1);
    assert_eq!(d["modelo_chat"], "qwen3:8b");
    assert_eq!(
        d["paginas"],
        serde_json::json!(["modelos", "vault", "ajustes", "chat"])
    );
    assert_eq!(d["filtrado"], true);
    assert!(d["n_codigo"].as_i64().unwrap() > 0);
    let en_curso: Vec<&Value> = d["progresos"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p[0] == true)
        .collect();
    assert!(!en_curso.is_empty(), "{}", d["progresos"]);
    assert!(
        en_curso
            .iter()
            .any(|p| p[1].as_f64().unwrap() > 0.0 && p[1].as_f64().unwrap() < 1.0),
        "{}",
        d["progresos"]
    );
    assert_eq!(d["bajado"], serde_json::json!([false, true]));
    assert!(textos(&d["instalados_despues"]).contains(&d["a_bajar"].as_str().unwrap().to_string()));
    assert_eq!(d["cancelado"], serde_json::json!([false, false]));
    assert!(!textos(&d["tras_borrar"]).contains(&"qwen3:8b".to_string()));
    assert_eq!(d["modelo_chat_tras_borrar"], d["a_bajar"]);
    let av = avisos(d);
    assert!(av.iter().any(|t| t.contains("ya está instalado")), "{av:?}");
    assert!(av.iter().any(|t| t.contains("Cancelé la descarga")), "{av:?}");
}

#[test]
fn chat() {
    let (t, bin) = preparar();
    let falso = ServidorFalso::para_ventana();
    let c = correr(
        t.path(),
        &Entorno {
            falso: &falso,
            arranca: false,
            bin: &bin,
        },
        r#"
        var m = chat.listaMensajes;
        function ultima() { return m.obtener(m.count - 1); }
        function mandar() { chat.enviar("Hola, ¿qué tal?"); datos.parciales = []; }
        function mirar() {
            var f = ultima();
            datos.parciales.push([f.escribiendo, f.contenido, f.pensamiento]);
            prueba.capturar("chat-escribiendo");
        }
        function termino() {
            var f = ultima();
            datos["final"] = [f.rol, f.contenido, f.pensamiento, f.escribiendo, f.datos];
            datos.generando = chat.generando;
            datos.lista = chat.lista.count;
            datos.titulo = chat.titulo;
            datos.vista = boveda.vista(f.contenido);
            prueba.capturar("chat-respuesta");
            chat.enviar("Otra pregunta");
        }
        function detener() { chat.detener(); }
        function detenido() {
            var f = ultima();
            datos.detenido = [f.escribiendo, f.datos, f.contenido.length < 40];
            chat.regenerar();
        }
        function regenerado() {
            datos.tras_regenerar = filas(m).map(function (x) { return x.rol; });
            datos.ultimo = ultima().contenido;
            var conv = JSON.parse(prueba.leer(CASA + "/datos/chats/" + chat.actual + ".json"));
            datos.guardados = conv.mensajes.map(function (x) { return x.role; });
            // Usar mi vault: la nota se cita como chip
            boveda.guardarEn("Ideas.md", "Quiero aprender piano y hacer juegos.\n");
            boveda.recargar();
            chat.nuevo();
            chat.usarVault = true;
            chat.enviar("¿Qué quiero aprender de piano?");
        }
        function con_vault() {
            datos.citas = ultima().citas;
            chat.guardarEnVault();
        }
        function guardado() {
            datos.chats_en_vault = lista(prueba.listar(boveda.ruta + "/Chats")).filter(function (n) { return n.endsWith(".md"); });
            var primero = chat.lista.obtener(chat.lista.count - 1).clave;
            chat.abrir(primero);
            datos.abierto = filas(m).map(function (x) { return x.rol; });
            prueba.capturar("chat-abierto");
            fin();
        }
        paso(500, mandar);
        for (var i = 0; i < 3; i++) paso(180, mirar);
        paso(900, termino);
        paso(250, detener);
        paso(400, detenido);
        paso(1500, regenerado);
        paso(1500, con_vault);
        paso(300, guardado);
        "#,
    );
    let d = &c.d;
    let parciales = d["parciales"].as_array().unwrap();
    let en_curso: Vec<&Value> = parciales.iter().filter(|p| p[0] == true).collect();
    assert!(!en_curso.is_empty(), "{}", d["parciales"]);
    assert!(
        en_curso.iter().any(|p| {
            let n = p[1].as_str().unwrap().chars().count();
            n > 0 && n < 60
        }),
        "los trozos no llegaron poco a poco: {}",
        d["parciales"]
    );
    let fin = &d["final"];
    assert_eq!(fin[0], "assistant");
    assert!(
        fin[1].as_str().unwrap().starts_with("Hola, esto es **Markdown**"),
        "{fin}"
    );
    assert_eq!(fin[2], "Veamos, lo pienso.");
    assert_eq!(fin[3], false);
    assert!(fin[4].as_str().unwrap().contains("palabras/s"), "{fin}");
    assert_eq!(d["generando"], false);
    assert_eq!(d["lista"], 1);
    assert!(!d["titulo"].as_str().unwrap().is_empty());
    assert!(
        d["vista"].as_str().unwrap().contains("[Ideas](nota:Ideas)"),
        "{}",
        d["vista"]
    );
    assert_eq!(d["detenido"][0], false);
    assert!(
        d["detenido"][1].as_str().unwrap().starts_with("Detenido"),
        "{}",
        d["detenido"]
    );
    assert_eq!(d["detenido"][2], true);
    let cuatro = serde_json::json!(["user", "assistant", "user", "assistant"]);
    assert_eq!(d["tras_regenerar"], cuatro);
    assert!(
        d["ultimo"].as_str().unwrap().ends_with("[[Ideas]]."),
        "{}",
        d["ultimo"]
    );
    assert_eq!(d["guardados"], cuatro);
    let citas: Vec<&str> = d["citas"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["ruta"].as_str().unwrap())
        .collect();
    assert!(citas.contains(&"Ideas.md"), "{citas:?}");
    assert_eq!(d["chats_en_vault"].as_array().unwrap().len(), 1);
    assert_eq!(d["abierto"], cuatro);
    // Lo que se le mandó al modelo: el historial al regenerar, y la nota en el «system» al usar el vault.
    let chats = falso.estado.chats.lock().unwrap();
    let roles = |i: usize| -> Vec<String> {
        chats[i]["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(roles(2), ["user", "assistant", "user"], "al regenerar");
    assert_eq!(roles(chats.len() - 1)[0], "system");
}

#[test]
fn vault_y_grafo() {
    let (t, bin) = preparar();
    let falso = ServidorFalso::para_ventana();
    let c = correr(
        t.path(),
        &Entorno {
            falso: &falso,
            arranca: false,
            bin: &bin,
        },
        r#"
        var b = boveda;
        function crear() {
            ventana.pagina = "vault";
            datos.ruta1 = b.nuevaNota("Ideas", "");
            b.nuevaCarpeta("Proyectos", "");
            datos.ruta2 = b.nuevaNota("Juegos", "Proyectos");
            b.abrir(datos.ruta1);
            prueba.hijo("editor").text = "Me gustan los [[Juegos]] y el piano. #musica\n\n[[Nota que falta]]";
        }
        function guardado() {
            datos.en_disco = prueba.leer(b.ruta + "/Ideas.md");
            datos.enlaces = lista(b.enlaces).map(function (e) { return [e.nombre, e.existe]; });
            datos.etiquetas = lista(b.etiquetasNota);
            datos.sugerir = lista(b.sugerir("jue"));
            prueba.capturar("vault-editor");
            prueba.hijo("paginaVault").modo = "lado";
            b.abrir(datos.ruta2);
        }
        function retro() {
            datos.retro = lista(b.retroenlaces).map(function (r) { return r.titulo; });
            datos.arbol = filas(b.lista).map(function (f) { return [f.tipo, f.nombre, f.nivel]; });
            prueba.capturar("vault-lado-a-lado");
            b.busqueda = "piano";
            datos.busqueda = lista(b.resultados).map(function (r) { return r.titulo; });
            b.busqueda = "";
            b.abrirEnlace("Nota que falta");
        }
        function creada() { datos.creada = b.nota; b.pedirGrafo(); }
        function grafo() {
            var g = b.grafo;
            datos.nodos = lista(g.nodos).map(function (n) { return n.titulo; }).sort();
            datos.aristas = g.aristas.length;
            prueba.hijo("paginaVault").verGrafo = true;
        }
        function grafo_mueve() {
            var gr = prueba.hijo("grafo");
            datos.grafo_cuantos = gr.cuantos;
            datos.energia = gr.energia;
            prueba.capturar("vault-grafo");
            prueba.hijo("paginaVault").verGrafo = false;
            b.renombrar(datos.ruta2, "Videojuegos");
        }
        function renombrado() {
            datos.tras_renombrar = prueba.leer(b.ruta + "/Ideas.md");
            b.borrar("Nota que falta.md");
        }
        function borrado() {
            datos.papelera = lista(prueba.listar(b.ruta + "/.trash"));
            datos.nota_tras_borrar = b.nota;
            b.actualizarMemoria();
        }
        function memoria() { datos.memoria = [b.memoriaProgreso, b.memoriaTexto]; fin(); }
        paso(500, crear);
        paso(1300, guardado);
        paso(300, retro);
        paso(300, creada);
        paso(500, grafo);
        paso(700, grafo_mueve);
        paso(300, renombrado);
        paso(300, borrado);
        paso(1200, memoria);
        "#,
    );
    let d = &c.d;
    assert_eq!(d["ruta1"], "Ideas.md");
    assert_eq!(d["ruta2"], "Proyectos/Juegos.md");
    assert!(
        d["en_disco"].as_str().unwrap().contains("[[Juegos]]"),
        "el editor no guardó solo"
    );
    let enlaces = d["enlaces"].as_array().unwrap();
    assert!(
        enlaces.contains(&serde_json::json!(["Juegos", true])),
        "{enlaces:?}"
    );
    assert!(
        enlaces.contains(&serde_json::json!(["Nota que falta", false])),
        "{enlaces:?}"
    );
    assert!(textos(&d["etiquetas"]).contains(&"musica".to_string()));
    assert_eq!(d["sugerir"], serde_json::json!(["Juegos"]));
    assert_eq!(d["retro"], serde_json::json!(["Ideas"]));
    let arbol = d["arbol"].as_array().unwrap();
    assert!(
        arbol.contains(&serde_json::json!(["carpeta", "Proyectos", 0])),
        "{arbol:?}"
    );
    assert!(
        arbol.contains(&serde_json::json!(["nota", "Juegos", 1])),
        "{arbol:?}"
    );
    assert_eq!(d["busqueda"], serde_json::json!(["Ideas"]));
    assert_eq!(d["creada"], "Nota que falta.md");
    assert_eq!(
        d["nodos"],
        serde_json::json!(["Ideas", "Juegos", "Nota que falta"])
    );
    assert_eq!(d["aristas"], 2);
    assert_eq!(d["grafo_cuantos"], 3);
    assert!(d["energia"].as_f64().unwrap() >= 0.0);
    assert!(
        d["tras_renombrar"].as_str().unwrap().contains("[[Videojuegos]]"),
        "{}",
        d["tras_renombrar"]
    );
    assert!(
        textos(&d["papelera"]).contains(&"Nota que falta.md".to_string()),
        "{}",
        d["papelera"]
    );
    assert_eq!(d["nota_tras_borrar"], "");
    assert!(d["memoria"][0].as_f64().unwrap() < 0.0);
    assert!(!d["memoria"][1].as_str().unwrap().is_empty(), "{}", d["memoria"]);
}

#[test]
fn arranca_ollama_solo() {
    let (t, bin) = preparar();
    // Nadie escucha en esa URL: la app tiene que arrancar «ollama» (el falso, que está en el PATH).
    let puerto = falso::puerto_cerrado();
    let mut falso = ServidorFalso::para_ventana();
    falso.url = format!("http://127.0.0.1:{puerto}");
    script(
        &bin,
        "ollama",
        r#"exec python3 -c '
import os, http.server
host, port = os.environ["OLLAMA_HOST"].rsplit(":", 1)
class H(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == "/api/tags":
            b = b"{\"models\": [{\"name\": \"qwen3:8b\", \"size\": 5200000000, \"details\": {\"family\": \"qwen3\"}}]}"
        else:
            b = b"{\"version\": \"0.0.1\"}"
        self.send_response(200)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
    def log_message(self, *a): pass
http.server.HTTPServer((host, int(port)), H).serve_forever()
'"#,
    );
    let c = correr(
        t.path(),
        &Entorno {
            falso: &falso,
            arranca: true,
            bin: &bin,
        },
        r#"
        function mirar() {
            datos.estado = ia.estadoOllama;
            datos.instalados = lista(modelos.nombresChat);
            fin();
        }
        paso(2500, mirar);
        "#,
    );
    let d = &c.d;
    assert_eq!(d["estado"], "corriendo");
    assert_eq!(d["instalados"], serde_json::json!(["qwen3:8b"]));
    assert!(
        d["avisos"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a[0].as_str().unwrap().contains("Arranqué Ollama") && a[1] == "exito"),
        "{}",
        d["avisos"]
    );
}

#[test]
fn pregunta_del_motor_al_abrir() {
    let (t, bin) = preparar();
    let falso = ServidorFalso::para_ventana();
    let c = correr(
        t.path(),
        &Entorno {
            falso: &falso,
            arranca: false,
            bin: &bin,
        },
        r#"
        var dialogo = prueba.hijo("dialogoMotor");
        function preguntar() {
            datos.cerrado_al_inicio = !dialogo.visible;
            ia.motorPregunta("¿Lo cambio?");
            prueba.procesar();
            datos.abierto = dialogo.visible;
            datos.mensaje = dialogo.mensaje;
            datos.boton_no = dialogo.textoCancelar;
            prueba.capturar("pregunta-motor");
            dialogo._rechazar();
        }
        function aceptar() {
            ia.motorPregunta("¿Lo cambio?");
            prueba.procesar();
            dialogo._aceptar();
        }
        function mirar() { datos.args = prueba.leer(CASA + "/../bin/args"); fin(); }
        paso(800, preguntar);
        paso(800, aceptar);
        paso(800, mirar);
        "#,
    );
    let d = &c.d;
    assert_eq!(d["cerrado_al_inicio"], true);
    assert_eq!(d["abierto"], true);
    assert_eq!(d["mensaje"], "¿Lo cambio?");
    assert_eq!(d["boton_no"], "No, gracias");
    // Las respuestas llegan a `vortex-ia`: primero «no», luego «sí».
    let args = d["args"].as_str().unwrap();
    let motor: Vec<&str> = args.lines().filter(|l| *l != "motor --pendiente").collect();
    assert_eq!(motor, ["motor --no", "motor --si"], "{args}");
    assert!(
        avisos(d).iter().any(|a| a.contains("Listo: Ollama usará CUDA")),
        "{:?}",
        avisos(d)
    );
    let _ = c.err;
}
